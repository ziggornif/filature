use crate::{credentials::CredentialCipher, persistence::Db};
use async_trait::async_trait;
use domain::printers::{
    FeedMode, LoadableSpool, LoadedSpool, MachineLink, Module, NewPrinter, Printer, PrinterBrand,
    PrinterName, PrinterRepository, RepositoryError, Slot, derive_slots,
};
use domain::shared::{PrinterId, SpoolId};
use sqlx::Row;
use std::collections::HashMap;
use ulid::Ulid;

pub struct SqlxPrinterRepository {
    pool: Db,
    cipher: Option<CredentialCipher>,
}
impl SqlxPrinterRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool, cipher: None }
    }
    pub fn with_cipher(pool: Db, cipher: Option<CredentialCipher>) -> Self {
        Self { pool, cipher }
    }

    async fn read_link(&self, id: &str) -> Result<Option<MachineLink>, RepositoryError> {
        let row =
            sqlx::query("SELECT kind,endpoint,credential FROM machine_links WHERE printer_id=$1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        row.map(|r| match r.get::<String, _>("kind").as_str() {
            "prusalink" => Ok(MachineLink::PrusaLink {
                host: r.get("endpoint"),
                api_key: String::new(),
            }),
            "moonraker" => Ok(MachineLink::Moonraker {
                url: r.get("endpoint"),
            }),
            _ => Err(RepositoryError::Backend("unknown machine link kind".into())),
        })
        .transpose()
    }

    async fn write_link(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: &str,
        link: Option<&MachineLink>,
    ) -> Result<(), RepositoryError> {
        let preserve = matches!(link, Some(MachineLink::PrusaLink { api_key, .. }) if api_key == "__configured__");
        if !preserve {
            sqlx::query("DELETE FROM machine_links WHERE printer_id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(backend)?;
        }
        if let Some(link) = link {
            match link {
                MachineLink::PrusaLink { host, api_key } if api_key == "__configured__" => {
                    let result=sqlx::query("UPDATE machine_links SET endpoint=$2 WHERE printer_id=$1 AND kind='prusalink'").bind(id).bind(host).execute(&mut **tx).await.map_err(backend)?;
                    if result.rows_affected() == 0 {
                        return Err(RepositoryError::Backend(
                            "configured PrusaLink credential is missing".into(),
                        ));
                    }
                }
                MachineLink::PrusaLink { host, api_key } => {
                    let cipher = self.cipher.as_ref().ok_or_else(|| {
                        RepositoryError::Backend(
                            "FILATURE_CREDENTIALS_KEY is required to save a PrusaLink API key"
                                .into(),
                        )
                    })?;
                    let encrypted = cipher.encrypt(api_key).map_err(RepositoryError::Backend)?;
                    sqlx::query("INSERT INTO machine_links(printer_id,kind,endpoint,credential) VALUES($1,'prusalink',$2,$3)").bind(id).bind(host).bind(encrypted).execute(&mut **tx).await.map_err(backend)?;
                }
                MachineLink::Moonraker { url } => {
                    sqlx::query("INSERT INTO machine_links(printer_id,kind,endpoint,credential) VALUES($1,'moonraker',$2,NULL)").bind(id).bind(url).execute(&mut **tx).await.map_err(backend)?;
                }
            }
        }
        Ok(())
    }
}

async fn topology(
    pool: &Db,
    printer_id: &str,
    heads: u8,
) -> Result<(u8, Vec<FeedMode>), RepositoryError> {
    let ams_units =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM printer_ams_units WHERE printer_id=$1")
            .bind(printer_id)
            .fetch_one(pool)
            .await
            .map_err(backend)?;
    let modes = sqlx::query_scalar::<_, String>(
        "SELECT feed_mode FROM printer_head_feed_modes WHERE printer_id=$1 ORDER BY head_index",
    )
    .bind(printer_id)
    .fetch_all(pool)
    .await
    .map_err(backend)?
    .into_iter()
    .map(|mode| FeedMode::parse(&mode).map_err(RepositoryError::from))
    .collect::<Result<Vec<_>, _>>()?;
    // A printer with no stored feed-mode rows predates the AMS-topology model
    // (or was inserted before its heads were backfilled); default every head to
    // Direct, mirroring migration 0012 and the export adapter's tolerant read. A
    // non-empty-but-wrong-length set is genuine corruption and still rejected.
    let modes = if modes.is_empty() {
        vec![FeedMode::Direct; usize::from(heads)]
    } else if modes.len() != usize::from(heads) {
        return Err(RepositoryError::Domain(
            domain::shared::DomainError::InvalidPrinterConfiguration(
                "stored feed modes do not match heads".into(),
            ),
        ));
    } else {
        modes
    };
    Ok((
        u8::try_from(ams_units).map_err(|_| {
            RepositoryError::Domain(domain::shared::DomainError::InvalidPrinterConfiguration(
                "invalid AMS unit count".into(),
            ))
        })?,
        modes,
    ))
}
fn backend(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

fn slot_write_error(
    e: sqlx::Error,
    printer_id: &PrinterId,
    spool_id: Option<&SpoolId>,
) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && let Some(constraint) = db.constraint()
    {
        if constraint == "printer_slots_spool_id_fkey" {
            return RepositoryError::UnknownSpool(
                spool_id.cloned().unwrap_or_else(|| SpoolId::new("")),
            );
        }
        if constraint == "printer_slots_printer_id_fkey" {
            return RepositoryError::NotFound(printer_id.clone());
        }
        if constraint == "printer_slots_spool_id_unique" {
            return RepositoryError::AlreadyLoaded(
                spool_id.cloned().unwrap_or_else(|| SpoolId::new("")),
            );
        }
    }
    backend(e)
}

#[async_trait]
impl PrinterRepository for SqlxPrinterRepository {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id,name,brand,model,heads,module_kind,module_count FROM printers ORDER BY name,id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let brand = PrinterBrand::parse(row.get("brand"))?;
            let model: String = row.get("model");
            let heads = u8::try_from(row.get::<i32, _>("heads")).map_err(|_| {
                RepositoryError::Domain(domain::shared::DomainError::InvalidPrinterConfiguration(
                    "invalid stored head count".into(),
                ))
            })?;
            let module = Module::from_storage(row.get("module_kind"), row.get("module_count"))?;
            Module::validate(brand, &model, heads, module.clone())?;
            let (ams_units, feed_modes) = topology(&self.pool, &id, heads).await?;
            let slot_rows = sqlx::query(
                r#"SELECT ps.group_label,ps.slot_key,ps.position,ps.spool_id,
                          s.colour_hex,s.colour_name,s.remaining_weight,s.net_weight,s.status,
                          m.name AS material_name,mf.name AS manufacturer_name
                   FROM printer_slots ps
                   LEFT JOIN spools s ON s.id=ps.spool_id
                   LEFT JOIN materials m ON m.id=s.material_id
                   LEFT JOIN manufacturers mf ON mf.id=s.manufacturer_id
                   WHERE ps.printer_id=$1"#,
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
                        loaded_spool: r.get::<Option<String>, _>("spool_id").map(|spool_id| {
                            LoadedSpool {
                                id: SpoolId::new(spool_id),
                                manufacturer_name: r.get("manufacturer_name"),
                                colour_hex: r.get("colour_hex"),
                                colour_name: r.get("colour_name"),
                                material_name: r.get("material_name"),
                                remaining_weight: r.get("remaining_weight"),
                                net_weight: r.get("net_weight"),
                                status: r.get("status"),
                            }
                        }),
                    };
                    (slot.key.clone(), slot)
                })
                .collect();
            let slots = derive_slots(brand, &model, heads, &module, ams_units, &feed_modes)?
                .into_iter()
                .filter_map(|expected| persisted.get(&expected.key).cloned())
                .collect();
            let machine_link = self.read_link(&id).await?;
            out.push(Printer {
                id: PrinterId::new(id),
                name: PrinterName::new(row.get::<String, _>("name"))?,
                brand,
                model,
                heads,
                module,
                ams_units,
                feed_modes,
                machine_link,
                slots,
            });
        }
        Ok(out)
    }

    async fn insert(&self, p: NewPrinter) -> Result<Printer, RepositoryError> {
        let slots = derive_slots(
            p.brand,
            &p.model,
            p.heads,
            &p.module,
            p.ams_units,
            &p.feed_modes,
        )?;
        let id = PrinterId::new(Ulid::new().to_string());
        let mut tx = self.pool.begin().await.map_err(backend)?;
        sqlx::query("INSERT INTO printers(id,name,brand,model,heads,module_kind,module_count) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(id.as_str()).bind(p.name.as_str()).bind(p.brand.as_str()).bind(&p.model).bind(i32::from(p.heads)).bind(p.module.kind()).bind(p.module.count().map(i32::from))
            .execute(&mut *tx).await.map_err(backend)?;
        for unit in 0..p.ams_units {
            sqlx::query("INSERT INTO printer_ams_units(printer_id,unit_index) VALUES($1,$2)")
                .bind(id.as_str())
                .bind(i32::from(unit))
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
        }
        for (head, mode) in p.feed_modes.iter().enumerate() {
            sqlx::query("INSERT INTO printer_head_feed_modes(printer_id,head_index,feed_mode) VALUES($1,$2,$3)").bind(id.as_str()).bind(i32::try_from(head).unwrap_or(i32::MAX)).bind(mode.as_str()).execute(&mut *tx).await.map_err(backend)?;
        }
        for s in &slots {
            sqlx::query("INSERT INTO printer_slots(id,printer_id,group_label,slot_key,position,spool_id) VALUES($1,$2,$3,$4,$5,NULL)")
                .bind(Ulid::new().to_string()).bind(id.as_str()).bind(&s.group_label).bind(&s.key).bind(i32::from(s.position))
                .execute(&mut *tx).await.map_err(backend)?;
        }
        self.write_link(&mut tx, id.as_str(), p.machine_link.as_ref())
            .await?;
        tx.commit().await.map_err(backend)?;
        Ok(Printer {
            id,
            name: p.name,
            brand: p.brand,
            model: p.model,
            heads: p.heads,
            module: p.module,
            ams_units: p.ams_units,
            feed_modes: p.feed_modes,
            machine_link: p.machine_link,
            slots,
        })
    }

    async fn update(&self, mut p: Printer) -> Result<Printer, RepositoryError> {
        let new_slots = derive_slots(
            p.brand,
            &p.model,
            p.heads,
            &p.module,
            p.ams_units,
            &p.feed_modes,
        )?;
        let mut tx = self.pool.begin().await.map_err(backend)?;
        let result = sqlx::query("UPDATE printers SET name=$2,brand=$3,model=$4,heads=$5,module_kind=$6,module_count=$7 WHERE id=$1")
            .bind(p.id.as_str()).bind(p.name.as_str()).bind(p.brand.as_str()).bind(&p.model).bind(i32::from(p.heads)).bind(p.module.kind()).bind(p.module.count().map(i32::from))
            .execute(&mut *tx).await.map_err(backend)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(p.id));
        }
        sqlx::query("DELETE FROM printer_ams_units WHERE printer_id=$1")
            .bind(p.id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        sqlx::query("DELETE FROM printer_head_feed_modes WHERE printer_id=$1")
            .bind(p.id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        for unit in 0..p.ams_units {
            sqlx::query("INSERT INTO printer_ams_units(printer_id,unit_index) VALUES($1,$2)")
                .bind(p.id.as_str())
                .bind(i32::from(unit))
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
        }
        for (head, mode) in p.feed_modes.iter().enumerate() {
            sqlx::query("INSERT INTO printer_head_feed_modes(printer_id,head_index,feed_mode) VALUES($1,$2,$3)").bind(p.id.as_str()).bind(i32::try_from(head).unwrap_or(i32::MAX)).bind(mode.as_str()).execute(&mut *tx).await.map_err(backend)?;
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
        self.write_link(&mut tx, p.id.as_str(), p.machine_link.as_ref())
            .await?;
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

    async fn spool_is_loadable(&self, id: &SpoolId) -> Result<Option<bool>, RepositoryError> {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM spools WHERE id=$1")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(backend)?;
        Ok(status.map(|s| matches!(s.as_str(), "Sealed" | "Open")))
    }

    async fn set_slot_spool(
        &self,
        printer_id: &PrinterId,
        slot_key: &str,
        spool_id: Option<&SpoolId>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(backend)?;
        if let Some(id) = spool_id {
            let status: Option<String> =
                sqlx::query_scalar("SELECT status FROM spools WHERE id=$1 FOR UPDATE")
                    .bind(id.as_str())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(backend)?;
            match status.as_deref() {
                None => return Err(RepositoryError::UnknownSpool(id.clone())),
                Some("Sealed" | "Open") => {}
                Some(_) => {
                    return Err(RepositoryError::Domain(
                        domain::shared::DomainError::SpoolNotLoadable,
                    ));
                }
            }
            sqlx::query("UPDATE printer_slots SET spool_id=NULL WHERE spool_id=$1")
                .bind(id.as_str())
                .execute(&mut *tx)
                .await
                .map_err(|e| slot_write_error(e, printer_id, spool_id))?;
        }
        let result =
            sqlx::query("UPDATE printer_slots SET spool_id=$3 WHERE printer_id=$1 AND slot_key=$2")
                .bind(printer_id.as_str())
                .bind(slot_key)
                .bind(spool_id.map(SpoolId::as_str))
                .execute(&mut *tx)
                .await
                .map_err(|e| slot_write_error(e, printer_id, spool_id))?;
        if result.rows_affected() == 0 {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM printers WHERE id=$1)")
                    .bind(printer_id.as_str())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(backend)?;
            return if exists {
                Err(RepositoryError::SlotNotFound {
                    printer_id: printer_id.clone(),
                    slot_key: slot_key.to_string(),
                })
            } else {
                Err(RepositoryError::NotFound(printer_id.clone()))
            };
        }
        tx.commit().await.map_err(backend)
    }

    async fn clear_spool(&self, spool_id: &SpoolId) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE printer_slots SET spool_id=NULL WHERE spool_id=$1")
            .bind(spool_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn loadable_spools(
        &self,
        current: Option<&SpoolId>,
    ) -> Result<Vec<LoadableSpool>, RepositoryError> {
        let rows = sqlx::query(
            r#"SELECT s.id,mf.name AS manufacturer_name,s.colour_hex,s.colour_name,
                      m.name AS material_name
               FROM spools s
               JOIN materials m ON m.id=s.material_id
               LEFT JOIN manufacturers mf ON mf.id=s.manufacturer_id
               LEFT JOIN printer_slots ps ON ps.spool_id=s.id
               WHERE s.status IN ('Sealed','Open')
                 AND (ps.spool_id IS NULL OR s.id=$1)
               ORDER BY mf.name NULLS LAST,s.colour_name NULLS LAST,m.name,s.id"#,
        )
        .bind(current.map(SpoolId::as_str))
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        Ok(rows
            .into_iter()
            .map(|r| LoadableSpool {
                id: SpoolId::new(r.get::<String, _>("id")),
                manufacturer_name: r.get("manufacturer_name"),
                colour_hex: r.get("colour_hex"),
                colour_name: r.get("colour_name"),
                material_name: r.get("material_name"),
            })
            .collect())
    }
}
