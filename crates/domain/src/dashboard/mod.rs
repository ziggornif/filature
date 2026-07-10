pub mod model;
pub mod ports;
pub mod read_models;
pub mod usecases;

#[cfg(feature = "stubs")]
pub mod stubs;

pub use model::{LOW_STOCK_RATIO, SpoolStockRow, StockStatus};
pub use ports::api::DashboardUseCases;
pub use ports::spi::{DashboardRepository, RepositoryError};
pub use read_models::{DashboardOverview, MaterialStockRow, SoonEmptyItem};
pub use usecases::DashboardService;
