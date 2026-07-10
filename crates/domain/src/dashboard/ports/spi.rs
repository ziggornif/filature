use crate::dashboard::model::SpoolStockRow;
use crate::shared::DomainError;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

/// Thin data-supply port: the adapter joins spools/materials/locations and
/// returns one row per non-archived spool. All aggregation (KPIs, the
/// low-stock rule, material grouping, sorting) happens in the domain, over
/// the rows this port returns — not in the adapter.
#[async_trait]
pub trait DashboardRepository: Send + Sync {
    async fn stock_rows(&self) -> Result<Vec<SpoolStockRow>, RepositoryError>;
}
