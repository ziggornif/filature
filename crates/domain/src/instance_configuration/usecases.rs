use crate::instance_configuration::InstanceConfiguration;
use crate::instance_configuration::ports::api::InstanceConfigurationUseCases;
use crate::instance_configuration::ports::spi::{InstanceConfigurationRepository, RepositoryError};
use crate::shared::LowStockThreshold;
use async_trait::async_trait;
use std::sync::Arc;

pub struct InstanceConfigurationService {
    repo: Arc<dyn InstanceConfigurationRepository>,
}

impl InstanceConfigurationService {
    pub fn new(repo: Arc<dyn InstanceConfigurationRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl InstanceConfigurationUseCases for InstanceConfigurationService {
    async fn get(&self) -> Result<InstanceConfiguration, RepositoryError> {
        Ok(self.repo.load().await?.unwrap_or_default())
    }

    async fn update_low_stock_threshold(
        &self,
        threshold: LowStockThreshold,
    ) -> Result<InstanceConfiguration, RepositoryError> {
        self.repo
            .save(InstanceConfiguration {
                low_stock_threshold: threshold,
            })
            .await
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::instance_configuration::stubs::StubInstanceConfigurationRepository;

    #[tokio::test]
    async fn absent_configuration_uses_default_threshold() {
        let service =
            InstanceConfigurationService::new(Arc::new(StubInstanceConfigurationRepository::new()));
        assert_eq!(
            service.get().await.unwrap().low_stock_threshold,
            LowStockThreshold::default()
        );
    }

    #[tokio::test]
    async fn updated_threshold_is_persisted_and_read_back() {
        let service =
            InstanceConfigurationService::new(Arc::new(StubInstanceConfigurationRepository::new()));
        let threshold = LowStockThreshold::new(27).unwrap();
        let updated = service.update_low_stock_threshold(threshold).await.unwrap();
        assert_eq!(updated.low_stock_threshold, threshold);
        assert_eq!(service.get().await.unwrap().low_stock_threshold, threshold);
    }
}
