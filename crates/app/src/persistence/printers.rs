use crate::persistence::Db;
use async_trait::async_trait;
use domain::printers::{
    Module, NewPrinter, Printer, PrinterBrand, PrinterName, PrinterRepository, RepositoryError,
    Slot, derive_slots,
};
use domain::shared::PrinterId;
use sqlx::Row;
use std::collections::HashMap;
use ulid::Ulid;

pub struct SqlxPrinterRepository {
    pool: Db,
}
impl SqlxPrinterRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}
fn backend(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

#[async_trait]
impl PrinterRepository for SqlxPrinterRepository {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id,name,brand,model,module_kind,module_count FROM printers ORDER BY name,id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let brand = PrinterBrand::parse(row.get("brand"))?;
            let model: String = row.get("model");
            let module = Module::from_storage(row.get("module_kind"), row.get("module_count"))?;
            Module::validate(brand, &model, module.clone())?;
            let slot_rows = sqlx::query(
                "SELECT group_label,slot_key,position FROM printer_slots WHERE printer_id=$1",
            )
            .bind(&id)
            .fetch_all(&self.pool)
            .await
            .map_err(backend)?;
            let persisted: HashMap<String, Slot> = slot_rows
                .into_iter()
                .map(|r| {
                    let slot = Slot {
                        group_label: r.get("group_label"),
                        key: r.get("slot_key"),
                        position: r.get::<i32, _>("position") as u16,
                    };
                    (slot.key.clone(), slot)
                })
                .collect();
            let slots = derive_slots(brand, &model, &module)?
                .into_iter()
                .filter_map(|expected| persisted.get(&expected.key).cloned())
                .collect();
            out.push(Printer {
                id: PrinterId::new(id),
                name: PrinterName::new(row.get::<String, _>("name"))?,
                brand,
                model,
                module,
                slots,
            });
        }
        Ok(out)
    }

    async fn insert(&self, p: NewPrinter) -> Result<Printer, RepositoryError> {
        let slots = derive_slots(p.brand, &p.model, &p.module)?;
        let id = PrinterId::new(Ulid::new().to_string());
        let mut tx = self.pool.begin().await.map_err(backend)?;
        sqlx::query("INSERT INTO printers(id,name,brand,model,module_kind,module_count) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(id.as_str()).bind(p.name.as_str()).bind(p.brand.as_str()).bind(&p.model).bind(p.module.kind()).bind(p.module.count().map(i32::from))
            .execute(&mut *tx).await.map_err(backend)?;
        for s in &slots {
            sqlx::query("INSERT INTO printer_slots(id,printer_id,group_label,slot_key,position,spool_id) VALUES($1,$2,$3,$4,$5,NULL)")
                .bind(Ulid::new().to_string()).bind(id.as_str()).bind(&s.group_label).bind(&s.key).bind(i32::from(s.position))
                .execute(&mut *tx).await.map_err(backend)?;
        }
        tx.commit().await.map_err(backend)?;
        Ok(Printer {
            id,
            name: p.name,
            brand: p.brand,
            model: p.model,
            module: p.module,
            slots,
        })
    }

    async fn update(&self, mut p: Printer) -> Result<Printer, RepositoryError> {
        let new_slots = derive_slots(p.brand, &p.model, &p.module)?;
        let mut tx = self.pool.begin().await.map_err(backend)?;
        let result = sqlx::query("UPDATE printers SET name=$2,brand=$3,model=$4,module_kind=$5,module_count=$6 WHERE id=$1")
            .bind(p.id.as_str()).bind(p.name.as_str()).bind(p.brand.as_str()).bind(&p.model).bind(p.module.kind()).bind(p.module.count().map(i32::from))
            .execute(&mut *tx).await.map_err(backend)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(p.id));
        }
        let keys: Vec<&str> = new_slots.iter().map(|s| s.key.as_str()).collect();
        sqlx::query("DELETE FROM printer_slots WHERE printer_id=$1 AND NOT(slot_key = ANY($2))")
            .bind(p.id.as_str())
            .bind(&keys)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        for s in &new_slots {
            sqlx::query("INSERT INTO printer_slots(id,printer_id,group_label,slot_key,position,spool_id) VALUES($1,$2,$3,$4,$5,NULL) ON CONFLICT(printer_id,slot_key) DO UPDATE SET group_label=EXCLUDED.group_label,position=EXCLUDED.position")
                .bind(Ulid::new().to_string()).bind(p.id.as_str()).bind(&s.group_label).bind(&s.key).bind(i32::from(s.position))
                .execute(&mut *tx).await.map_err(backend)?;
        }
        tx.commit().await.map_err(backend)?;
        p.slots = new_slots;
        Ok(p)
    }

    async fn delete(&self, id: &PrinterId) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM printers WHERE id=$1")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id.clone()));
        }
        Ok(())
    }
}
