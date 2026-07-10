use crate::dashboard::model::SpoolStockRow;
use crate::dashboard::ports::spi::{DashboardRepository, RepositoryError};
use async_trait::async_trait;

pub struct StubDashboardRepository {
    rows: Vec<SpoolStockRow>,
}

impl StubDashboardRepository {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn with(rows: Vec<SpoolStockRow>) -> Self {
        Self { rows }
    }
}

impl Default for StubDashboardRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DashboardRepository for StubDashboardRepository {
    async fn stock_rows(&self) -> Result<Vec<SpoolStockRow>, RepositoryError> {
        Ok(self.rows.clone())
    }
}
