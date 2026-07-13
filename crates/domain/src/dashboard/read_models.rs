use crate::dashboard::model::{SpoolStockRow, StockStatus};
use crate::shared::{Grams, LowStockThreshold, Money};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// One row of the per-material stock breakdown: how much of a given
/// material is currently in stock, plus a proportional mini-bar fraction
/// (this material's remaining weight ÷ the largest material's remaining
/// weight among the rows fed in) for the UI to render a width from.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialStockRow {
    pub material_name: String,
    pub spool_count: usize,
    pub remaining_weight: Grams,
    pub bar_fraction: f64,
}

/// A single low-stock ("soon-empty") spool, as shown in the dashboard's
/// soon-empty list.
#[derive(Debug, Clone, PartialEq)]
pub struct SoonEmptyItem {
    pub spool_id: String,
    pub manufacturer_name: Option<String>,
    pub material_name: String,
    pub colour_hex: String,
    pub colour_name: Option<String>,
    pub location_name: Option<String>,
    pub remaining_weight: Grams,
    pub remaining_ratio: f64,
}

/// The stock-at-a-glance dashboard: KPIs, per-material breakdown and the
/// soon-empty list, all computed over the non-archived spool stock rows
/// supplied by `DashboardRepository`.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardOverview {
    /// Σ (remaining/net × price_paid) over the supplied rows — same
    /// semantics as `SpoolRepository::stock_value` for the all-stock scope.
    pub stock_value: Money,
    /// Σ remaining weight over the supplied rows.
    pub total_remaining: Grams,
    pub total_count: usize,
    pub active_count: usize,
    pub empty_count: usize,
    /// Count of low-stock rows (ratio ≤ configured threshold and remaining > 0).
    pub alert_count: usize,
    /// One row per material with ≥1 supplied row, ordered by remaining
    /// weight descending then material name (deterministic).
    pub material_breakdown: Vec<MaterialStockRow>,
    /// Low-stock rows, sorted by remaining ratio ascending (closest to
    /// empty first).
    pub soon_empty: Vec<SoonEmptyItem>,
}

/// Per-material running totals used while folding rows, before the
/// mini-bar fraction (which needs the max across all materials) can be
/// computed.
struct MaterialAgg {
    material_name: String,
    spool_count: usize,
    remaining_weight: f64,
}

impl DashboardOverview {
    /// Pure computation of the dashboard overview from the raw, non-archived
    /// stock rows supplied by the SPI. No I/O, no database — everything
    /// (stock value, counts, the low-stock rule, material grouping, the
    /// mini-bar fraction and the soon-empty sort) is a fold over `rows`.
    /// Guards every division: `Grams::ratio_of` handles zero-net rows, and
    /// the mini-bar fraction guards a zero max weight (including the
    /// empty-input case) — both yield `0.0` rather than panicking.
    pub fn from_rows(rows: Vec<SpoolStockRow>, threshold: LowStockThreshold) -> Self {
        let total_count = rows.len();
        let mut active_count = 0usize;
        let mut empty_count = 0usize;
        let mut total_remaining = 0.0f64;
        let mut stock_value = Decimal::ZERO;
        let mut soon_empty: Vec<SoonEmptyItem> = Vec::new();
        let mut material_order: Vec<String> = Vec::new();
        let mut material_aggs: HashMap<String, MaterialAgg> = HashMap::new();

        for row in &rows {
            match row.status {
                StockStatus::Sealed | StockStatus::Open => active_count += 1,
                StockStatus::Empty => empty_count += 1,
            }

            total_remaining += row.remaining_weight.value();

            let ratio = row.remaining_ratio();
            let ratio_decimal = Decimal::try_from(ratio).unwrap_or(Decimal::ZERO);
            stock_value += ratio_decimal * row.price_paid.value();

            if row.is_low_stock(threshold) {
                soon_empty.push(SoonEmptyItem {
                    spool_id: row.spool_id.clone(),
                    manufacturer_name: row.manufacturer_name.clone(),
                    material_name: row.material_name.clone(),
                    colour_hex: row.colour_hex.clone(),
                    colour_name: row.colour_name.clone(),
                    location_name: row.location_name.clone(),
                    remaining_weight: row.remaining_weight,
                    remaining_ratio: ratio,
                });
            }

            let key = row.material_id.as_str().to_string();
            material_aggs
                .entry(key.clone())
                .and_modify(|agg| {
                    agg.spool_count += 1;
                    agg.remaining_weight += row.remaining_weight.value();
                })
                .or_insert_with(|| {
                    material_order.push(key);
                    MaterialAgg {
                        material_name: row.material_name.clone(),
                        spool_count: 1,
                        remaining_weight: row.remaining_weight.value(),
                    }
                });
        }

        soon_empty.sort_by(|a, b| a.remaining_ratio.partial_cmp(&b.remaining_ratio).unwrap());

        let max_weight = material_aggs
            .values()
            .map(|agg| agg.remaining_weight)
            .fold(0.0f64, f64::max);

        let mut material_breakdown: Vec<MaterialStockRow> = material_order
            .into_iter()
            .filter_map(|key| material_aggs.remove(&key))
            .map(|agg| {
                let bar_fraction = if max_weight <= 0.0 {
                    0.0
                } else {
                    agg.remaining_weight / max_weight
                };
                MaterialStockRow {
                    material_name: agg.material_name,
                    spool_count: agg.spool_count,
                    remaining_weight: Grams::new(agg.remaining_weight)
                        .unwrap_or(Grams::new(0.0).unwrap()),
                    bar_fraction,
                }
            })
            .collect();

        material_breakdown.sort_by(|a, b| {
            b.remaining_weight
                .value()
                .partial_cmp(&a.remaining_weight.value())
                .unwrap()
                .then_with(|| a.material_name.cmp(&b.material_name))
        });

        let alert_count = soon_empty.len();

        DashboardOverview {
            stock_value: Money::from_decimal(stock_value).unwrap_or(Money::new(0, 0).unwrap()),
            total_remaining: Grams::new(total_remaining).unwrap_or(Grams::new(0.0).unwrap()),
            total_count,
            active_count,
            empty_count,
            alert_count,
            material_breakdown,
            soon_empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::MaterialId;

    fn default_threshold() -> LowStockThreshold {
        LowStockThreshold::default()
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        spool_id: &str,
        material_id: &str,
        material_name: &str,
        status: StockStatus,
        remaining: f64,
        net: f64,
        price: (i64, u32),
        location_name: Option<&str>,
    ) -> SpoolStockRow {
        SpoolStockRow {
            spool_id: spool_id.to_string(),
            material_id: MaterialId::new(material_id),
            material_name: material_name.to_string(),
            manufacturer_name: Some("Prusament".to_string()),
            colour_hex: "#1A9E4B".to_string(),
            colour_name: None,
            status,
            remaining_weight: Grams::new(remaining).unwrap(),
            net_weight: Grams::new(net).unwrap(),
            price_paid: Money::new(price.0, price.1).unwrap(),
            location_name: location_name.map(|s| s.to_string()),
        }
    }

    #[test]
    fn stock_value_sums_remaining_ratio_times_price() {
        // Full spool: 1000g net, price 25.00 -> contributes 25.00.
        let full = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Sealed,
            1000.0,
            1000.0,
            (2500, 2),
            None,
        );
        // Half-consumed: 500/1000 * 12.00 -> contributes 6.00.
        let half = row(
            "s2",
            "m1",
            "PLA",
            StockStatus::Open,
            500.0,
            1000.0,
            (1200, 2),
            None,
        );
        let overview = DashboardOverview::from_rows(vec![full, half], default_threshold());
        assert_eq!(overview.stock_value, Money::new(3100, 2).unwrap());
    }

    #[test]
    fn remaining_sum_and_counts_over_supplied_rows() {
        let rows = vec![
            row(
                "s1",
                "m1",
                "PLA",
                StockStatus::Sealed,
                1000.0,
                1000.0,
                (1000, 2),
                None,
            ),
            row(
                "s2",
                "m1",
                "PLA",
                StockStatus::Open,
                400.0,
                1000.0,
                (1000, 2),
                None,
            ),
            row(
                "s3",
                "m2",
                "PETG",
                StockStatus::Empty,
                0.0,
                1000.0,
                (1000, 2),
                None,
            ),
        ];
        let overview = DashboardOverview::from_rows(rows, default_threshold());
        assert_eq!(overview.total_remaining.value(), 1400.0);
        assert_eq!(overview.total_count, 3);
        assert_eq!(overview.active_count, 2);
        assert_eq!(overview.empty_count, 1);
    }

    #[test]
    fn alert_boundary_ratio_exactly_threshold_is_included() {
        // 150 / 1000 == 0.15 exactly.
        let r = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Open,
            150.0,
            1000.0,
            (1000, 2),
            None,
        );
        let overview = DashboardOverview::from_rows(vec![r], default_threshold());
        assert_eq!(overview.alert_count, 1);
        assert_eq!(overview.soon_empty.len(), 1);
    }

    #[test]
    fn alert_boundary_ratio_just_above_threshold_is_excluded() {
        // 151 / 1000 == 0.151, just above 0.15.
        let r = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Open,
            151.0,
            1000.0,
            (1000, 2),
            None,
        );
        let overview = DashboardOverview::from_rows(vec![r], default_threshold());
        assert_eq!(overview.alert_count, 0);
        assert!(overview.soon_empty.is_empty());
    }

    #[test]
    fn alert_excludes_zero_remaining_even_with_large_net() {
        let r = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Empty,
            0.0,
            5000.0,
            (1000, 2),
            None,
        );
        let overview = DashboardOverview::from_rows(vec![r], default_threshold());
        assert_eq!(overview.alert_count, 0);
        assert!(overview.soon_empty.is_empty());
    }

    #[test]
    fn alert_includes_just_above_zero_at_low_ratio() {
        // 1 / 1000 == 0.001, well within the low-stock band and > 0.
        let r = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Open,
            1.0,
            1000.0,
            (1000, 2),
            None,
        );
        let overview = DashboardOverview::from_rows(vec![r], default_threshold());
        assert_eq!(overview.alert_count, 1);
        assert_eq!(overview.soon_empty.len(), 1);
    }

    #[test]
    fn material_breakdown_groups_by_material_with_count_weight_and_bar_fraction() {
        let rows = vec![
            row(
                "s1",
                "m1",
                "PLA",
                StockStatus::Sealed,
                800.0,
                1000.0,
                (1000, 2),
                None,
            ),
            row(
                "s2",
                "m1",
                "PLA",
                StockStatus::Open,
                200.0,
                1000.0,
                (1000, 2),
                None,
            ),
            row(
                "s3",
                "m2",
                "PETG",
                StockStatus::Sealed,
                500.0,
                1000.0,
                (1000, 2),
                None,
            ),
        ];
        let overview = DashboardOverview::from_rows(rows, default_threshold());
        assert_eq!(overview.material_breakdown.len(), 2);

        let pla = overview
            .material_breakdown
            .iter()
            .find(|m| m.material_name == "PLA")
            .unwrap();
        assert_eq!(pla.spool_count, 2);
        assert_eq!(pla.remaining_weight.value(), 1000.0);
        assert_eq!(pla.bar_fraction, 1.0); // max material.

        let petg = overview
            .material_breakdown
            .iter()
            .find(|m| m.material_name == "PETG")
            .unwrap();
        assert_eq!(petg.spool_count, 1);
        assert_eq!(petg.remaining_weight.value(), 500.0);
        assert_eq!(petg.bar_fraction, 0.5); // 500 / 1000.
    }

    #[test]
    fn material_breakdown_single_material_gets_full_bar() {
        let rows = vec![row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Sealed,
            300.0,
            1000.0,
            (1000, 2),
            None,
        )];
        let overview = DashboardOverview::from_rows(rows, default_threshold());
        assert_eq!(overview.material_breakdown.len(), 1);
        assert_eq!(overview.material_breakdown[0].bar_fraction, 1.0);
    }

    #[test]
    fn material_breakdown_omits_materials_with_no_rows() {
        // Only rows fed in appear; a material with zero supplied rows simply
        // never shows up (the SPI only supplies rows for materials that
        // actually have stock).
        let rows = vec![row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Sealed,
            300.0,
            1000.0,
            (1000, 2),
            None,
        )];
        let overview = DashboardOverview::from_rows(rows, default_threshold());
        assert_eq!(overview.material_breakdown.len(), 1);
        assert_eq!(overview.material_breakdown[0].material_name, "PLA");
    }

    #[test]
    fn soon_empty_contains_exactly_low_stock_rows_sorted_ascending_with_correct_fields() {
        let rows = vec![
            row(
                "s1",
                "m1",
                "PLA",
                StockStatus::Open,
                100.0,
                1000.0,
                (1000, 2),
                Some("Shelf A"),
            ), // ratio 0.10
            row(
                "s2",
                "m1",
                "PLA",
                StockStatus::Open,
                50.0,
                1000.0,
                (1000, 2),
                None,
            ), // ratio 0.05
            row(
                "s3",
                "m1",
                "PLA",
                StockStatus::Sealed,
                900.0,
                1000.0,
                (1000, 2),
                None,
            ), // not low-stock
        ];
        let overview = DashboardOverview::from_rows(rows, default_threshold());
        assert_eq!(overview.soon_empty.len(), 2);
        assert_eq!(overview.soon_empty[0].spool_id, "s2");
        assert!((overview.soon_empty[0].remaining_ratio - 0.05).abs() < 1e-9);
        assert_eq!(overview.soon_empty[1].spool_id, "s1");
        assert!((overview.soon_empty[1].remaining_ratio - 0.10).abs() < 1e-9);
        assert_eq!(
            overview.soon_empty[1].location_name,
            Some("Shelf A".to_string())
        );
        assert_eq!(overview.soon_empty[0].location_name, None);
        assert_eq!(overview.soon_empty[0].material_name, "PLA");
        assert_eq!(
            overview.soon_empty[0].manufacturer_name,
            Some("Prusament".to_string())
        );
    }

    #[test]
    fn empty_input_yields_all_zeros_no_panic() {
        let overview = DashboardOverview::from_rows(Vec::new(), default_threshold());
        assert_eq!(overview.stock_value, Money::new(0, 0).unwrap());
        assert_eq!(overview.total_remaining.value(), 0.0);
        assert_eq!(overview.total_count, 0);
        assert_eq!(overview.active_count, 0);
        assert_eq!(overview.empty_count, 0);
        assert_eq!(overview.alert_count, 0);
        assert!(overview.material_breakdown.is_empty());
        assert!(overview.soon_empty.is_empty());
    }

    #[test]
    fn configured_threshold_changes_low_stock_result() {
        let r = row(
            "s1",
            "m1",
            "PLA",
            StockStatus::Open,
            200.0,
            1000.0,
            (1000, 2),
            None,
        );
        let default = DashboardOverview::from_rows(vec![r.clone()], default_threshold());
        let raised = DashboardOverview::from_rows(vec![r], LowStockThreshold::new(20).unwrap());
        assert_eq!(default.alert_count, 0);
        assert_eq!(raised.alert_count, 1);
    }
}
