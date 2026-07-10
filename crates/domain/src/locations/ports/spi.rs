use crate::locations::model::{Location, NewLocation};
use crate::shared::{DomainError, LocationId};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("no location with id '{}'", .0.as_str())]
    NotFound(LocationId),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

#[async_trait]
pub trait LocationRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError>;
    async fn insert(&self, l: NewLocation) -> Result<Location, RepositoryError>;
    async fn update(&self, l: Location) -> Result<Location, RepositoryError>;
    async fn delete(&self, id: &LocationId) -> Result<(), RepositoryError>;
    async fn count_spools(&self, id: &LocationId) -> Result<u64, RepositoryError>;
}
