use crate::shared::{Grams, LocationId, Money};
use crate::spools::model::{EditSpool, NewSpool, Spool, SpoolId};
use crate::spools::ports::spi::{ReconcilableSpool, RepositoryError, SpoolFilter, SpoolSort};
use crate::spools::read_models::{SpoolDetail, SpoolListItem};
use async_trait::async_trait;

#[async_trait]
pub trait SpoolsUseCases: Send + Sync {
    async fn reconcilable(&self) -> Result<Vec<ReconcilableSpool>, RepositoryError>;
    async fn memorize_ams_tag(
        &self,
        id: SpoolId,
        tag_uid: String,
    ) -> Result<Spool, RepositoryError>;
    async fn add(&self, s: NewSpool) -> Result<Spool, RepositoryError>;
    async fn edit(&self, s: EditSpool) -> Result<Spool, RepositoryError>;
    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError>;
    async fn view(&self, id: SpoolId) -> Result<SpoolDetail, RepositoryError>;
    async fn set_remaining(&self, id: SpoolId, remaining: Grams) -> Result<Spool, RepositoryError>;
    async fn consume(&self, id: SpoolId, amount: Grams) -> Result<Spool, RepositoryError>;
    async fn archive(&self, id: SpoolId) -> Result<Spool, RepositoryError>;
    async fn restore(&self, id: SpoolId) -> Result<Spool, RepositoryError>;
    async fn assign_location(
        &self,
        id: SpoolId,
        location: Option<LocationId>,
    ) -> Result<Spool, RepositoryError>;
    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError>;
    async fn count(&self, filter: SpoolFilter) -> Result<u64, RepositoryError>;
}
