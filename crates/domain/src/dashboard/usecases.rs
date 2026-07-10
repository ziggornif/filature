use crate::dashboard::ports::api::DashboardUseCases;
use crate::dashboard::ports::spi::{DashboardRepository, RepositoryError};
use crate::dashboard::read_models::DashboardOverview;
use async_trait::async_trait;
use std::sync::Arc;

pub struct DashboardService {
    repo: Arc<dyn DashboardRepository>,
}

impl DashboardService {
    pub fn new(repo: Arc<dyn DashboardRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl DashboardUseCases for DashboardService {
    async fn overview(&self) -> Result<DashboardOverview, RepositoryError> {
        let rows = self.repo.stock_rows().await?;
        Ok(DashboardOverview::from_rows(rows))
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::dashboard::model::{SpoolStockRow, StockStatus};
    use crate::dashboard::stubs::StubDashboardRepository;
    use crate::shared::{Grams, MaterialId, Money};

    fn sample_row() -> SpoolStockRow {
        SpoolStockRow {
            spool_id: "s1".to_string(),
            material_id: MaterialId::new("m1"),
            material_name: "PLA".to_string(),
            colour_hex: "#1A9E4B".to_string(),
            colour_name: None,
            status: StockStatus::Sealed,
            remaining_weight: Grams::new(1000.0).unwrap(),
            net_weight: Grams::new(1000.0).unwrap(),
            price_paid: Money::new(2500, 2).unwrap(),
            location_name: None,
        }
    }

    #[tokio::test]
    async fn overview_wires_spi_rows_through_the_pure_computation() {
        let repo = StubDashboardRepository::with(vec![sample_row()]);
        let service = DashboardService::new(Arc::new(repo));
        let overview = service.overview().await.unwrap();
        assert_eq!(overview.total_count, 1);
        assert_eq!(overview.stock_value, Money::new(2500, 2).unwrap());
    }

    #[tokio::test]
    async fn overview_on_empty_repository_is_all_zeros() {
        let repo = StubDashboardRepository::new();
        let service = DashboardService::new(Arc::new(repo));
        let overview = service.overview().await.unwrap();
        assert_eq!(overview.total_count, 0);
        assert_eq!(overview.stock_value, Money::new(0, 0).unwrap());
    }
}
