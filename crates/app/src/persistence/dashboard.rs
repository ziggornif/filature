//! SQLx adapter for the dashboard slice SPI (`DashboardRepository`). A thin
//! data-supply port: one SELECT joining `spools` to `materials` (name) and
//! `manufacturers`/`locations` (names), excluding archived spools — no aggregation here, all
//! KPI/grouping/sorting computation happens in the domain over the rows this
//! adapter returns (ADR-0003, PostgreSQL).

use crate::persistence::Db;
use async_trait::async_trait;
use domain::dashboard::{DashboardRepository, RepositoryError, SpoolStockRow, StockStatus};
use domain::shared::{Grams, MaterialId, Money};
use rust_decimal::Decimal;

#[derive(sqlx::FromRow)]
struct StockRow {
    id: String,
    colour_hex: Option<String>,
    colour_name: Option<String>,
    net_weight: f64,
    remaining_weight: f64,
    price_paid: Decimal,
    status: String,
    material_id: String,
    material_name: String,
    manufacturer_name: Option<String>,
    location_name: Option<String>,
}

pub struct SqlxDashboardRepository {
    pool: Db,
}

impl SqlxDashboardRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

fn backend(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

fn build_status(s: &str) -> Result<StockStatus, RepositoryError> {
    match s {
        "Sealed" => Ok(StockStatus::Sealed),
        "Open" => Ok(StockStatus::Open),
        "Empty" => Ok(StockStatus::Empty),
        other => Err(RepositoryError::Backend(format!(
            "unexpected non-archived spool status: {other}"
        ))),
    }
}

fn build_grams(v: f64) -> Result<Grams, RepositoryError> {
    Grams::new(v).map_err(|e| RepositoryError::Backend(e.to_string()))
}

#[async_trait]
impl DashboardRepository for SqlxDashboardRepository {
    async fn stock_rows(&self) -> Result<Vec<SpoolStockRow>, RepositoryError> {
        let rows = sqlx::query_as::<_, StockRow>(
            r#"SELECT s.id, s.colour_hex, s.colour_name, s.net_weight,
                      s.remaining_weight, s.price_paid, s.status,
                      m.id AS material_id, m.name AS material_name,
                      mf.name AS manufacturer_name,
                      l.name AS location_name
               FROM spools s
               JOIN materials m ON m.id = s.material_id
               LEFT JOIN manufacturers mf ON mf.id = s.manufacturer_id
               LEFT JOIN locations l ON l.id = s.location_id
               WHERE s.status <> 'Archived'"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        rows.into_iter()
            .map(|r| {
                Ok(SpoolStockRow {
                    spool_id: r.id,
                    material_id: MaterialId::new(r.material_id),
                    material_name: r.material_name,
                    manufacturer_name: r.manufacturer_name,
                    colour_hex: r.colour_hex.unwrap_or_default(),
                    colour_name: r.colour_name,
                    status: build_status(&r.status)?,
                    remaining_weight: build_grams(r.remaining_weight)?,
                    net_weight: build_grams(r.net_weight)?,
                    price_paid: Money::from_decimal(r.price_paid)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    location_name: r.location_name,
                })
            })
            .collect()
    }
}
