use crate::manufacturers::model::{Manufacturer, NewManufacturer};
use crate::shared::{DomainError, ManufacturerId};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("no manufacturer with id '{}'", .0.as_str())]
    NotFound(ManufacturerId),
    #[error("a manufacturer named '{0}' already exists")]
    Duplicate(String),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

#[async_trait]
pub trait ManufacturerRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Manufacturer>, RepositoryError>;
    async fn insert(&self, m: NewManufacturer) -> Result<Manufacturer, RepositoryError>;
    async fn delete(&self, id: &ManufacturerId) -> Result<(), RepositoryError>;
    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError>;
    async fn count_spools(&self, id: &ManufacturerId) -> Result<u64, RepositoryError>;
}
