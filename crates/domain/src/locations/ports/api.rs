use crate::locations::model::{Location, NewLocation};
use crate::locations::ports::spi::RepositoryError;
use crate::shared::LocationId;
use async_trait::async_trait;

#[async_trait]
pub trait LocationsUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError>;
    /// Read model for the locations list page: each `Location` paired with
    /// its current assigned-spool count (`LocationRepository::count_spools`),
    /// so the web adapter can render a "spool count" column without the
    /// domain exposing a bespoke view type. Implemented in terms of the
    /// existing SPI primitives (`list` + `count_spools`) — no SPI change
    /// needed.
    async fn list_with_spool_counts(&self) -> Result<Vec<(Location, u64)>, RepositoryError>;
    async fn add(&self, l: NewLocation) -> Result<Location, RepositoryError>;
    async fn edit(&self, l: Location) -> Result<Location, RepositoryError>;
    async fn delete(&self, id: LocationId) -> Result<(), RepositoryError>;
}
