use crate::dashboard::ports::spi::RepositoryError;
use crate::dashboard::read_models::DashboardOverview;
use async_trait::async_trait;

#[async_trait]
pub trait DashboardUseCases: Send + Sync {
    /// Computes the stock-at-a-glance overview over all non-archived spools.
    async fn overview(&self) -> Result<DashboardOverview, RepositoryError>;
}
