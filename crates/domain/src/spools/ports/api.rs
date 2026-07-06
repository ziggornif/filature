use crate::spools::model::{NewSpool, Spool, SpoolId};
use crate::spools::ports::spi::{RepositoryError, SpoolFilter, SpoolSort};
use crate::spools::read_models::{SpoolDetail, SpoolListItem};
use async_trait::async_trait;

#[async_trait]
pub trait SpoolsUseCases: Send + Sync {
    async fn add(&self, s: NewSpool) -> Result<Spool, RepositoryError>;
    async fn edit(&self, s: Spool) -> Result<Spool, RepositoryError>;
    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError>;
    async fn view(&self, id: SpoolId) -> Result<SpoolDetail, RepositoryError>;
}
