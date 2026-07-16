//! PostgreSQL adapter for complete instance export and atomic replacement.

use crate::persistence::Db;
use async_trait::async_trait;
use domain::instance_transfer::{
    InstanceSnapshot, InstanceTransferRepository, SnapshotConfiguration, SnapshotDiameter,
    SnapshotLocation, SnapshotManufacturer, SnapshotMaterial, SnapshotPrinter, SnapshotPrinterSlot,
    SnapshotSensitivity, SnapshotSpool, SnapshotSpoolStatus, SnapshotSpoolType, TransferError,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};

pub struct SqlxInstanceTransferRepository {
    pool: Db,
}

impl SqlxInstanceTransferRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

fn backend(error: impl std::fmt::Display) -> TransferError {
    TransferError::Backend(error.to_string())
}

fn invalid_value(field: &str, value: impl std::fmt::Display) -> TransferError {
    TransferError::Backend(format!("invalid {field} stored in database: {value}"))
}

#[derive(sqlx::FromRow)]
struct MaterialRow {
    id: String,
    name: String,
    density: f64,
    drying_temp_c: i32,
    drying_time_h: i32,
    sensitivity: String,
    nozzle_c: i32,
    bed_c: i32,
}

#[derive(sqlx::FromRow)]
struct ManufacturerRow {
    id: String,
    name: String,
    country: Option<String>,
}

#[derive(sqlx::FromRow)]
struct LocationRow {
    id: String,
    name: String,
    note: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SpoolRow {
    id: String,
    material_id: String,
    spool_type: String,
    colour_hex: Option<String>,
    colour_name: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    price_paid: Decimal,
    status: String,
    location_id: Option<String>,
    manufacturer_id: Option<String>,
    notes: Option<String>,
    purchased_at: Option<Date>,
    opened_at: Option<Date>,
    created_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct PrinterRow {
    id: String,
    name: String,
    brand: String,
    model: String,
    heads: i32,
    module_kind: String,
    module_count: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct PrinterSlotRow {
    printer_id: String,
    slot_key: String,
    group_label: String,
    position: i32,
    spool_id: Option<String>,
}

fn u16_field(field: &str, value: i32) -> Result<u16, TransferError> {
    u16::try_from(value).map_err(|_| invalid_value(field, value))
}

fn sensitivity(value: &str) -> Result<SnapshotSensitivity, TransferError> {
    match value {
        "Low" => Ok(SnapshotSensitivity::Low),
        "Medium" => Ok(SnapshotSensitivity::Medium),
        "High" => Ok(SnapshotSensitivity::High),
        other => Err(invalid_value("sensitivity", other)),
    }
}

fn spool_type(value: &str) -> Result<SnapshotSpoolType, TransferError> {
    match value {
        "Complete" => Ok(SnapshotSpoolType::Complete),
        "Recharge" => Ok(SnapshotSpoolType::Recharge),
        other => Err(invalid_value("spool_type", other)),
    }
}

fn diameter(value: &str) -> Result<SnapshotDiameter, TransferError> {
    match value {
        "1.75" => Ok(SnapshotDiameter::Mm1_75),
        "2.85" => Ok(SnapshotDiameter::Mm2_85),
        other => Err(invalid_value("diameter", other)),
    }
}

fn status(value: &str) -> Result<SnapshotSpoolStatus, TransferError> {
    match value {
        "Sealed" => Ok(SnapshotSpoolStatus::Sealed),
        "Open" => Ok(SnapshotSpoolStatus::Open),
        "Empty" => Ok(SnapshotSpoolStatus::Empty),
        "Archived" => Ok(SnapshotSpoolStatus::Archived),
        other => Err(invalid_value("status", other)),
    }
}

#[async_trait]
impl InstanceTransferRepository for SqlxInstanceTransferRepository {
    async fn export_snapshot(&self) -> Result<InstanceSnapshot, TransferError> {
        let mut transaction = self.pool.begin().await.map_err(backend)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        let materials = sqlx::query_as::<_, MaterialRow>(
            "SELECT id, name, density, drying_temp_c, drying_time_h, sensitivity, nozzle_c, bed_c FROM materials ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(backend)?
        .into_iter()
        .map(|row| {
            Ok(SnapshotMaterial {
                id: row.id,
                name: row.name,
                density: row.density,
                drying_temp_c: u16_field("drying_temp_c", row.drying_temp_c)?,
                drying_time_h: u16_field("drying_time_h", row.drying_time_h)?,
                sensitivity: sensitivity(&row.sensitivity)?,
                nozzle_c: u16_field("nozzle_c", row.nozzle_c)?,
                bed_c: u16_field("bed_c", row.bed_c)?,
            })
        })
        .collect::<Result<Vec<_>, TransferError>>()?;

        let manufacturers = sqlx::query_as::<_, ManufacturerRow>(
            "SELECT id, name, country FROM manufacturers ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(backend)?
        .into_iter()
        .map(|row| SnapshotManufacturer {
            id: row.id,
            name: row.name,
            country: row.country,
        })
        .collect();

        let locations =
            sqlx::query_as::<_, LocationRow>("SELECT id, name, note FROM locations ORDER BY id")
                .fetch_all(&mut *transaction)
                .await
                .map_err(backend)?
                .into_iter()
                .map(|row| SnapshotLocation {
                    id: row.id,
                    name: row.name,
                    note: row.note,
                })
                .collect();

        let spools = sqlx::query_as::<_, SpoolRow>(
            r#"SELECT id, material_id, spool_type, colour_hex, colour_name, diameter,
                      net_weight, remaining_weight, price_paid, status, location_id,
                      manufacturer_id, notes, purchased_at, opened_at, created_at
               FROM spools ORDER BY id"#,
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(backend)?
        .into_iter()
        .map(|row| {
            Ok(SnapshotSpool {
                id: row.id,
                material_id: row.material_id,
                spool_type: spool_type(&row.spool_type)?,
                colour_hex: row.colour_hex,
                colour_name: row.colour_name,
                diameter: diameter(&row.diameter)?,
                net_weight: row.net_weight,
                remaining_weight: row.remaining_weight,
                price_paid: row.price_paid,
                status: status(&row.status)?,
                location_id: row.location_id,
                manufacturer_id: row.manufacturer_id,
                notes: row.notes,
                purchased_at: row.purchased_at,
                opened_at: row.opened_at,
                created_at: row.created_at.format(&Rfc3339).map_err(backend)?,
            })
        })
        .collect::<Result<Vec<_>, TransferError>>()?;

        let mut printers = sqlx::query_as::<_, PrinterRow>(
            "SELECT id, name, brand, model, heads, module_kind, module_count FROM printers ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(backend)?
        .into_iter()
        .map(|row| {
            Ok(SnapshotPrinter {
                id: row.id,
                name: row.name,
                brand: row.brand,
                model: row.model,
                heads: u8::try_from(row.heads).map_err(|_| invalid_value("heads", row.heads))?,
                module_kind: row.module_kind,
                module_count: row
                    .module_count
                    .map(|count| u16_field("module_count", count))
                    .transpose()?,
                slots: vec![],
            })
        })
        .collect::<Result<Vec<_>, TransferError>>()?;
        let printer_indices: HashMap<String, usize> = printers
            .iter()
            .enumerate()
            .map(|(index, printer)| (printer.id.clone(), index))
            .collect();
        for row in sqlx::query_as::<_, PrinterSlotRow>(
            r#"SELECT printer_id, slot_key, group_label, position, spool_id
               FROM printer_slots ORDER BY printer_id, position, slot_key"#,
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(backend)?
        {
            let index = printer_indices
                .get(&row.printer_id)
                .ok_or_else(|| invalid_value("printer_id", &row.printer_id))?;
            printers[*index].slots.push(SnapshotPrinterSlot {
                slot_key: row.slot_key,
                group_label: row.group_label,
                position: u16_field("position", row.position)?,
                spool_id: row.spool_id,
            });
        }

        let threshold = sqlx::query_scalar::<_, i16>(
            "SELECT low_stock_threshold FROM instance_configuration WHERE singleton = TRUE",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(backend)?
        .unwrap_or(15);
        let low_stock_threshold =
            u8::try_from(threshold).map_err(|_| invalid_value("low_stock_threshold", threshold))?;
        transaction.commit().await.map_err(backend)?;

        Ok(InstanceSnapshot {
            materials,
            manufacturers,
            locations,
            spools,
            printers,
            configuration: SnapshotConfiguration {
                low_stock_threshold,
            },
        })
    }

    async fn replace_snapshot(&self, snapshot: InstanceSnapshot) -> Result<(), TransferError> {
        // Parse every timestamp before opening the transaction. A malformed
        // snapshot therefore cannot even start the destructive phase.
        let created_at = snapshot
            .spools
            .iter()
            .map(|spool| OffsetDateTime::parse(&spool.created_at, &Rfc3339).map_err(backend))
            .collect::<Result<Vec<_>, _>>()?;
        let mut transaction = self.pool.begin().await.map_err(backend)?;

        // Dependants first, then referentials. Nothing is committed until all
        // inserts and constraints have succeeded.
        sqlx::query("DELETE FROM printer_slots")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM printers")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM spools")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM materials")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM locations")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM manufacturers")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM instance_configuration")
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;

        for material in snapshot.materials {
            sqlx::query(
                r#"INSERT INTO materials
                   (id, name, density, drying_temp_c, drying_time_h, sensitivity, nozzle_c, bed_c)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(material.id)
            .bind(material.name)
            .bind(material.density)
            .bind(i32::from(material.drying_temp_c))
            .bind(i32::from(material.drying_time_h))
            .bind(material.sensitivity.as_str())
            .bind(i32::from(material.nozzle_c))
            .bind(i32::from(material.bed_c))
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        }

        for manufacturer in snapshot.manufacturers {
            sqlx::query("INSERT INTO manufacturers (id, name, country) VALUES ($1, $2, $3)")
                .bind(manufacturer.id)
                .bind(manufacturer.name)
                .bind(manufacturer.country)
                .execute(&mut *transaction)
                .await
                .map_err(backend)?;
        }

        for location in snapshot.locations {
            sqlx::query("INSERT INTO locations (id, name, note) VALUES ($1, $2, $3)")
                .bind(location.id)
                .bind(location.name)
                .bind(location.note)
                .execute(&mut *transaction)
                .await
                .map_err(backend)?;
        }

        for (spool, created_at) in snapshot.spools.into_iter().zip(created_at) {
            sqlx::query(
                r#"INSERT INTO spools
                   (id, material_id, spool_type, colour_hex, colour_name, diameter, net_weight,
                    remaining_weight, price_paid, status, location_id, manufacturer_id, notes,
                    purchased_at, opened_at, created_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
            )
            .bind(spool.id)
            .bind(spool.material_id)
            .bind(spool.spool_type.as_str())
            .bind(spool.colour_hex)
            .bind(spool.colour_name)
            .bind(spool.diameter.as_str())
            .bind(spool.net_weight)
            .bind(spool.remaining_weight)
            .bind(spool.price_paid)
            .bind(spool.status.as_str())
            .bind(spool.location_id)
            .bind(spool.manufacturer_id)
            .bind(spool.notes)
            .bind(spool.purchased_at)
            .bind(spool.opened_at)
            .bind(created_at)
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;
        }

        for printer in snapshot.printers {
            sqlx::query(
                r#"INSERT INTO printers (id, name, brand, model, heads, module_kind, module_count)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(&printer.id)
            .bind(printer.name)
            .bind(printer.brand)
            .bind(printer.model)
            .bind(i32::from(printer.heads))
            .bind(printer.module_kind)
            .bind(printer.module_count.map(i32::from))
            .execute(&mut *transaction)
            .await
            .map_err(backend)?;

            for slot in printer.slots {
                sqlx::query(
                    r#"INSERT INTO printer_slots
                       (id, printer_id, group_label, slot_key, position, spool_id)
                       VALUES ($1, $2, $3, $4, $5, $6)"#,
                )
                .bind(ulid::Ulid::new().to_string())
                .bind(&printer.id)
                .bind(slot.group_label)
                .bind(slot.slot_key)
                .bind(i32::from(slot.position))
                .bind(slot.spool_id)
                .execute(&mut *transaction)
                .await
                .map_err(backend)?;
            }
        }

        sqlx::query(
            "INSERT INTO instance_configuration (singleton, low_stock_threshold) VALUES (TRUE, $1)",
        )
        .bind(i16::from(snapshot.configuration.low_stock_threshold))
        .execute(&mut *transaction)
        .await
        .map_err(backend)?;

        transaction.commit().await.map_err(backend)
    }
}
