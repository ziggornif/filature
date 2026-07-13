//! SQLx adapter for the instance-configuration persistence port.

use crate::persistence::Db;
use async_trait::async_trait;
use domain::instance_configuration::{
    InstanceConfiguration, InstanceConfigurationRepository, RepositoryError,
};
use domain::shared::LowStockThreshold;
use sqlx::Postgres;

pub struct SqlxInstanceConfigurationRepository {
    pool: Db,
}

impl SqlxInstanceConfigurationRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

fn backend(e: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

fn configuration_from(percent: i16) -> Result<InstanceConfiguration, RepositoryError> {
    let low_stock_threshold = LowStockThreshold::new(i64::from(percent)).map_err(backend)?;
    Ok(InstanceConfiguration {
        low_stock_threshold,
    })
}

#[async_trait]
impl InstanceConfigurationRepository for SqlxInstanceConfigurationRepository {
    async fn load(&self) -> Result<Option<InstanceConfiguration>, RepositoryError> {
        let percent = sqlx::query_scalar::<Postgres, i16>(
            "SELECT low_stock_threshold FROM instance_configuration WHERE singleton = TRUE",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        percent.map(configuration_from).transpose()
    }

    async fn save(
        &self,
        configuration: InstanceConfiguration,
    ) -> Result<InstanceConfiguration, RepositoryError> {
        let percent = i16::from(configuration.low_stock_threshold.percent());
        let stored = sqlx::query_scalar::<Postgres, i16>(
            r#"INSERT INTO instance_configuration (singleton, low_stock_threshold)
               VALUES (TRUE, $1)
               ON CONFLICT (singleton) DO UPDATE
               SET low_stock_threshold = EXCLUDED.low_stock_threshold
               RETURNING low_stock_threshold"#,
        )
        .bind(percent)
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;

        configuration_from(stored)
    }
}
