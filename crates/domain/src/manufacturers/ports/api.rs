use crate::manufacturers::model::{Manufacturer, NewManufacturer};
use crate::manufacturers::ports::spi::RepositoryError;
use crate::shared::ManufacturerId;
use async_trait::async_trait;

#[async_trait]
pub trait ManufacturersUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Manufacturer>, RepositoryError>;
    /// Read model for the manufacturers list page: each `Manufacturer`
    /// paired with its current referencing-spool count, so the web adapter
    /// can render a "spool count" column without a bespoke view type.
    async fn list_with_spool_counts(&self) -> Result<Vec<(Manufacturer, u64)>, RepositoryError>;
    async fn add(&self, m: NewManufacturer) -> Result<Manufacturer, RepositoryError>;
    async fn delete(&self, id: ManufacturerId) -> Result<(), RepositoryError>;
    /// Idempotently insert the built-in manufacturer referential.
    async fn seed_defaults(&self) -> Result<(), RepositoryError>;
}
