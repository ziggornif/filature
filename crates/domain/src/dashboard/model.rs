use crate::shared::{Grams, LowStockThreshold, MaterialId, Money};

/// Lifecycle status of a spool, as relevant to the dashboard overview. A
/// deliberately smaller/local mirror of `spools::model::SpoolStatus` (which
/// also has `Archived`) — the SPI only ever supplies non-archived rows, so
/// `Archived` has no representation here. Kept as its own type (rather than
/// importing `spools::model::SpoolStatus`) so the `dashboard` slice does not
/// depend on the `spools` slice's internals; the persistence adapter maps
/// the persisted status into this type when building a `SpoolStockRow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockStatus {
    Sealed,
    Open,
    Empty,
}

/// Raw, non-archived spool stock data as supplied by `DashboardRepository`.
/// One row per non-archived spool. Cross-slice fields (`material_name`, the
/// spool's colour) are carried as plain primitives rather than by importing
/// `spools::model::Colour` — that type lives in the `spools` slice (not the
/// shared kernel), so importing it here would make `dashboard` depend on
/// `spools`'s internals and break slice isolation. `material_id` reuses
/// `shared::MaterialId`, which — unlike `Colour` — *is* part of the shared
/// kernel precisely so other slices can reference a material by id.
#[derive(Debug, Clone, PartialEq)]
pub struct SpoolStockRow {
    pub spool_id: String,
    pub material_id: MaterialId,
    pub material_name: String,
    pub manufacturer_name: Option<String>,
    pub colour_hex: String,
    pub colour_name: Option<String>,
    pub status: StockStatus,
    pub remaining_weight: Grams,
    pub net_weight: Grams,
    pub price_paid: Money,
    pub location_name: Option<String>,
}

impl SpoolStockRow {
    /// Remaining weight as a fraction of net weight (0.0..=1.0+), guarding
    /// the zero-net case via `Grams::ratio_of`.
    pub fn remaining_ratio(&self) -> f64 {
        self.remaining_weight.ratio_of(self.net_weight)
    }

    /// A row is low-stock ("soon-empty") when its ratio is at or below
    /// the configured threshold and it still has weight remaining.
    pub fn is_low_stock(&self, threshold: LowStockThreshold) -> bool {
        self.remaining_weight.value() > 0.0 && self.remaining_ratio() <= threshold.ratio()
    }
}
