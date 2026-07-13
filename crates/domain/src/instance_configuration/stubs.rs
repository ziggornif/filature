use crate::instance_configuration::{
    InstanceConfiguration, InstanceConfigurationRepository, RepositoryError,
};
use async_trait::async_trait;
use std::sync::Mutex;

pub struct StubInstanceConfigurationRepository {
    configuration: Mutex<Option<InstanceConfiguration>>,
}

impl StubInstanceConfigurationRepository {
    pub fn new() -> Self {
        Self {
            configuration: Mutex::new(None),
        }
    }

    pub fn with(configuration: InstanceConfiguration) -> Self {
        Self {
            configuration: Mutex::new(Some(configuration)),
        }
    }
}

impl Default for StubInstanceConfigurationRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InstanceConfigurationRepository for StubInstanceConfigurationRepository {
    async fn load(&self) -> Result<Option<InstanceConfiguration>, RepositoryError> {
        Ok(*self.configuration.lock().unwrap())
    }

    async fn save(
        &self,
        configuration: InstanceConfiguration,
    ) -> Result<InstanceConfiguration, RepositoryError> {
        *self.configuration.lock().unwrap() = Some(configuration);
        Ok(configuration)
    }
}
