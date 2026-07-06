use crate::materials::model::{Material, NewMaterial};
use crate::materials::ports::spi::RepositoryError;
use async_trait::async_trait;

#[async_trait]
pub trait MaterialsUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Material>, RepositoryError>;
    async fn add(&self, m: NewMaterial) -> Result<Material, RepositoryError>;
    async fn edit(&self, m: Material) -> Result<Material, RepositoryError>;
    async fn seed_defaults(&self) -> Result<(), RepositoryError>;
}
