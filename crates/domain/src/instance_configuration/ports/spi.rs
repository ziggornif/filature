use crate::instance_configuration::InstanceConfiguration;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
}

#[async_trait]
pub trait InstanceConfigurationRepository: Send + Sync {
    async fn load(&self) -> Result<Option<InstanceConfiguration>, RepositoryError>;
    async fn save(
        &self,
        configuration: InstanceConfiguration,
    ) -> Result<InstanceConfiguration, RepositoryError>;
}
