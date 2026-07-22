use crate::{credentials::CredentialCipher, persistence::Db};
use async_trait::async_trait;
use domain::{
    machine_connectivity::{MachineError, MachineLinkRepository},
    printers::MachineLink,
    shared::PrinterId,
};
use sqlx::Row;

pub struct SqlxMachineLinkRepository {
    pool: Db,
    cipher: Option<CredentialCipher>,
}
impl SqlxMachineLinkRepository {
    pub fn new(pool: Db, cipher: Option<CredentialCipher>) -> Self {
        Self { pool, cipher }
    }
}

#[async_trait]
impl MachineLinkRepository for SqlxMachineLinkRepository {
    async fn find_link(&self, printer_id: &PrinterId) -> Result<Option<MachineLink>, MachineError> {
        let row =
            sqlx::query("SELECT kind,endpoint,credential FROM machine_links WHERE printer_id=$1")
                .bind(printer_id.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| MachineError::Repository(e.to_string()))?;
        row.map(|r| match r.get::<String, _>("kind").as_str() {
            "prusalink" => {
                let encrypted: String = r.get("credential");
                let cipher = self.cipher.as_ref().ok_or_else(|| {
                    MachineError::Repository("FILATURE_CREDENTIALS_KEY is required".into())
                })?;
                Ok(MachineLink::PrusaLink {
                    host: r.get("endpoint"),
                    api_key: cipher
                        .decrypt(&encrypted)
                        .map_err(MachineError::Repository)?,
                })
            }
            "moonraker" => Ok(MachineLink::Moonraker {
                url: r.get("endpoint"),
            }),
            _ => Err(MachineError::Repository(
                "unknown stored machine link kind".into(),
            )),
        })
        .transpose()
    }
}

pub async fn validate_credentials_at_boot(
    pool: &Db,
    cipher: Option<&CredentialCipher>,
) -> Result<(), String> {
    let rows = sqlx::query("SELECT credential FROM machine_links")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Ok(());
    }
    let cipher = cipher.ok_or_else(|| {
        "FILATURE_CREDENTIALS_KEY is required because Machine Links exist".to_string()
    })?;
    for row in rows {
        if let Some(credential) = row
            .try_get::<Option<String>, _>("credential")
            .map_err(|e| e.to_string())?
        {
            cipher.decrypt(&credential)?;
        }
    }
    Ok(())
}
