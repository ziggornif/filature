use crate::instance_configuration::{InstanceConfiguration, RepositoryError};
use crate::shared::LowStockThreshold;
use async_trait::async_trait;

#[async_trait]
pub trait InstanceConfigurationUseCases: Send + Sync {
    async fn get(&self) -> Result<InstanceConfiguration, RepositoryError>;
    async fn update_low_stock_threshold(
        &self,
        threshold: LowStockThreshold,
    ) -> Result<InstanceConfiguration, RepositoryError>;
}
