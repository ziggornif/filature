use crate::locations::model::{Location, NewLocation};
use crate::locations::ports::spi::RepositoryError;
use crate::shared::LocationId;
use async_trait::async_trait;

#[async_trait]
pub trait LocationsUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError>;
    async fn add(&self, l: NewLocation) -> Result<Location, RepositoryError>;
    async fn edit(&self, l: Location) -> Result<Location, RepositoryError>;
    async fn delete(&self, id: LocationId) -> Result<(), RepositoryError>;
}
