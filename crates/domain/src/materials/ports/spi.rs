use crate::materials::model::{Material, MaterialId, NewMaterial};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("a material named '{0}' already exists")]
    Duplicate(String),
    #[error("no material with id '{}'", .0.as_str())]
    NotFound(MaterialId),
    #[error("persistence backend error: {0}")]
    Backend(String),
}

#[async_trait]
pub trait MaterialRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Material>, RepositoryError>;
    async fn insert(&self, m: NewMaterial) -> Result<Material, RepositoryError>;
    async fn update(&self, m: Material) -> Result<Material, RepositoryError>;
    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError>;
}
