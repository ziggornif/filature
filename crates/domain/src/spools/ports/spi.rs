use crate::shared::{DomainError, LocationId, ManufacturerId, MaterialId, Money};
use crate::spools::model::{NewSpool, Spool, SpoolId, SpoolStatus};
use crate::spools::read_models::{SpoolDetail, SpoolListItem};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("a spool named '{0}' already exists")]
    Duplicate(String),
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("no spool with id '{}'", .0.as_str())]
    NotFound(SpoolId),
    #[error("no material with id '{}'", .0.as_str())]
    UnknownMaterial(MaterialId),
    #[error("no location with id '{}'", .0.as_str())]
    UnknownLocation(LocationId),
    #[error("no manufacturer with id '{}'", .0.as_str())]
    UnknownManufacturer(ManufacturerId),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

/// Filter applied when listing spools.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpoolFilter {
    pub material_id: Option<MaterialId>,
    pub status: Option<SpoolStatus>,
}

/// Sort order applied when listing spools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpoolSort {
    #[default]
    CreatedDesc,
    RemainingRatioAsc,
    RemainingRatioDesc,
}

#[async_trait]
pub trait SpoolRepository: Send + Sync {
    async fn insert(&self, s: NewSpool) -> Result<Spool, RepositoryError>;
    async fn update(&self, s: Spool) -> Result<Spool, RepositoryError>;
    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError>;
    async fn get(&self, id: &SpoolId) -> Result<Option<SpoolDetail>, RepositoryError>;
    /// Loads the aggregate (not a read model) for load -> mutate -> save
    /// use cases.
    async fn find(&self, id: &SpoolId) -> Result<Option<Spool>, RepositoryError>;
    /// Sum of `(remaining/net) * price_paid` over non-archived spools
    /// matching `filter`.
    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError>;
}
