//! The driving (Axum) adapter for the dashboard slice: a single read-only,
//! static server render (`GET /`) showing stock at a glance. No htmx
//! interactivity, no fragment routes — mirrors `web::materials`/`web::spools`
//! for locale/theme resolution and Tera rendering, but has exactly one
//! handler since the whole page is one aggregate read
//! (`DashboardUseCases::overview`).

use crate::web::router::{internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use crate::web::templates::Renderer;
use axum::{
    Router,
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use domain::dashboard::{DashboardOverview, MaterialStockRow, SoonEmptyItem};
use domain::spools::Colour;
use serde::Serialize;
use tera::Context;

/// Template-shaped view of a `MaterialStockRow`: plain strings/numbers plus
/// the derived `bar_pct` (the domain exposes a `bar_fraction` in `0.0..=1.0`;
/// the template wants a `width: N%` — same "derive the display value in Rust,
/// not in Tera" convention as `SpoolView::remaining_pct`).
#[derive(Serialize)]
pub struct MaterialBreakdownView {
    pub material_name: String,
    pub spool_count: usize,
    pub remaining_kg: f64,
    pub bar_pct: u8,
}

impl From<MaterialStockRow> for MaterialBreakdownView {
    fn from(row: MaterialStockRow) -> Self {
        Self {
            material_name: row.material_name,
            spool_count: row.spool_count,
            remaining_kg: to_kg(row.remaining_weight.value()),
            bar_pct: pct(row.bar_fraction),
        }
    }
}

/// Template-shaped view of a `SoonEmptyItem`.
#[derive(Serialize)]
pub struct SoonEmptyView {
    pub spool_id: String,
    pub manufacturer_name: Option<String>,
    pub material_name: String,
    pub colour_hex: String,
    /// The localized human name derived from the colour value.
    pub colour_label: String,
    pub location_name: Option<String>,
    pub remaining_g: f64,
    pub remaining_pct: u8,
}

impl SoonEmptyView {
    fn localized(item: SoonEmptyItem, renderer: &Renderer, locale: &str) -> Self {
        let colour_label = Colour::from_hex(item.colour_hex.clone())
            .ok()
            .and_then(|colour| colour.name().map(str::to_owned))
            .map(|key| renderer.t(locale, &format!("spools.colour.preset.{key}")))
            .unwrap_or_else(|| "—".into());
        Self {
            spool_id: item.spool_id,
            manufacturer_name: item.manufacturer_name,
            material_name: item.material_name,
            colour_label,
            colour_hex: item.colour_hex,
            location_name: item.location_name,
            remaining_g: round1(item.remaining_weight.value()),
            remaining_pct: pct(item.remaining_ratio),
        }
    }
}

/// Template-shaped view of the whole `DashboardOverview`.
#[derive(Serialize)]
pub struct DashboardView {
    /// Formatted exactly like the spools list's Stock Value stat
    /// (`Money`'s `Display`, no currency symbol appended — matches
    /// `web::spools::list_page`'s `stock_value.to_string()`).
    pub stock_value: String,
    pub remaining_kg: f64,
    pub total_count: usize,
    pub active_count: usize,
    pub empty_count: usize,
    pub alert_count: usize,
    pub material_breakdown: Vec<MaterialBreakdownView>,
    pub soon_empty: Vec<SoonEmptyView>,
}

impl DashboardView {
    fn localized(o: DashboardOverview, renderer: &Renderer, locale: &str) -> Self {
        Self {
            stock_value: o.stock_value.to_string(),
            remaining_kg: to_kg(o.total_remaining.value()),
            total_count: o.total_count,
            active_count: o.active_count,
            empty_count: o.empty_count,
            alert_count: o.alert_count,
            material_breakdown: o.material_breakdown.into_iter().map(Into::into).collect(),
            soon_empty: o
                .soon_empty
                .into_iter()
                .map(|item| SoonEmptyView::localized(item, renderer, locale))
                .collect(),
        }
    }
}

/// Grams -> kg, rounded to 2 decimals — kg figures are small enough (single-
/// to double-digit) that 1-decimal rounding (the g/m convention elsewhere)
/// would lose too much precision.
fn to_kg(grams: f64) -> f64 {
    (grams / 1000.0 * 100.0).round() / 100.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Percent 0..=100 from a `0.0..=1.0` fraction. `as u8` on a float is a
/// saturating cast in Rust (no UB, no panic) — same guard-by-cast convention
/// as `SpoolView::remaining_pct` for an out-of-range ratio.
fn pct(fraction: f64) -> u8 {
    (fraction * 100.0).round() as u8
}

fn render(
    st: &AppState,
    locale: &str,
    theme_attr: &str,
    overview: DashboardOverview,
    low_stock_threshold_pct: u8,
    nav_spool_count: u64,
    nav_printer_count: usize,
) -> Response {
    let view = DashboardView::localized(overview, &st.renderer, locale);
    let mut ctx = Context::new();
    ctx.insert("stock_value", &view.stock_value);
    ctx.insert("remaining_kg", &view.remaining_kg);
    ctx.insert("total_count", &view.total_count);
    ctx.insert("active_count", &view.active_count);
    ctx.insert("empty_count", &view.empty_count);
    ctx.insert("alert_count", &view.alert_count);
    ctx.insert("material_breakdown", &view.material_breakdown);
    ctx.insert("soon_empty", &view.soon_empty);
    ctx.insert("low_stock_threshold_pct", &low_stock_threshold_pct);
    ctx.insert("nav_spool_count", &nav_spool_count);
    ctx.insert("nav_printer_count", &nav_printer_count);
    // Read by `base.html` to mark the "Tableau de bord" nav item active.
    ctx.insert("page", "dashboard");
    match st
        .renderer
        .render("dashboard.html", locale, theme_attr, ctx)
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn index(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let configuration = match st.instance_configuration.get().await {
        Ok(configuration) => configuration,
        Err(e) => return internal_error(e),
    };
    let threshold = configuration.low_stock_threshold;
    match st.dashboard.overview(threshold).await {
        Ok(overview) => {
            let nav_spool_count = st.nav_spool_count().await;
            let nav_printer_count = st.nav_printer_count().await;
            render(
                &st,
                &locale,
                theme.data_attr(),
                overview,
                threshold.percent(),
                nav_spool_count,
                nav_printer_count,
            )
        }
        Err(e) => internal_error(e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;
    use domain::shared::{Grams, MaterialId, Money};

    fn breakdown_row() -> MaterialBreakdownView {
        MaterialBreakdownView {
            material_name: "PLA".into(),
            spool_count: 3,
            remaining_kg: 2.4,
            bar_pct: 100,
        }
    }

    fn soon_empty_row() -> SoonEmptyView {
        SoonEmptyView {
            spool_id: "01HSP".into(),
            manufacturer_name: Some("Prusament".into()),
            material_name: "PETG".into(),
            colour_hex: "#1A9E4B".into(),
            colour_label: "vert sapin".into(),
            location_name: Some("Shelf A".into()),
            remaining_g: 42.0,
            remaining_pct: 4,
        }
    }

    fn full_ctx() -> Context {
        let mut ctx = Context::new();
        ctx.insert("stock_value", "137.50");
        ctx.insert("remaining_kg", &4.2);
        ctx.insert("total_count", &5usize);
        ctx.insert("active_count", &3usize);
        ctx.insert("empty_count", &2usize);
        ctx.insert("alert_count", &1usize);
        ctx.insert("material_breakdown", &vec![breakdown_row()]);
        ctx.insert("soon_empty", &vec![soon_empty_row()]);
        ctx.insert("low_stock_threshold_pct", &20u8);
        ctx.insert("nav_spool_count", &18u64);
        ctx.insert("nav_printer_count", &3usize);
        ctx.insert("page", "dashboard");
        ctx
    }

    fn empty_ctx() -> Context {
        let mut ctx = Context::new();
        ctx.insert("stock_value", "0.00");
        ctx.insert("remaining_kg", &0.0);
        ctx.insert("total_count", &0usize);
        ctx.insert("active_count", &0usize);
        ctx.insert("empty_count", &0usize);
        ctx.insert("alert_count", &0usize);
        ctx.insert("material_breakdown", &Vec::<MaterialBreakdownView>::new());
        ctx.insert("soon_empty", &Vec::<SoonEmptyView>::new());
        ctx.insert("low_stock_threshold_pct", &15u8);
        ctx.insert("page", "dashboard");
        ctx
    }

    #[test]
    fn seeded_page_shows_kpis_breakdown_and_soon_empty_no_raw_keys() {
        let r = Renderer::new(Catalog::load("en"));
        let html = r.render("dashboard.html", "en", "", full_ctx()).unwrap();
        assert!(html.contains("137.50"));
        assert!(html.contains(r#"4.2 <span class="kpi-unit">kg</span>"#));
        assert!(html.contains(r#"id="kpi-spools""#));
        assert!(html.contains("PLA"));
        assert!(html.contains("3 spools"));
        assert!(html.contains("remaining weight"));
        assert!(html.contains("width: 100%"));
        assert!(html.contains("threshold 20 %"));
        assert!(html.contains("Prusament · vert sapin"));
        assert!(html.contains("PETG · Shelf A"));
        assert!(html.contains("PETG"));
        assert!(html.contains("Shelf A"));
        assert!(html.contains("42.0 g"));
        assert!(html.contains("4 %"));
        assert!(html.contains("/spools/01HSP"));
        assert!(!html.contains("dashboard.kpi."));
        assert!(!html.contains("dashboard.breakdown."));
        assert!(!html.contains("dashboard.soon_empty."));
        assert!(!html.contains("spools.location."));
    }

    #[test]
    fn nav_dashboard_item_is_marked_active() {
        let r = Renderer::new(Catalog::load("en"));
        let html = r.render("dashboard.html", "en", "", full_ctx()).unwrap();
        assert!(html.contains(r#"href="/" class="active""#));
        assert!(html.contains(r#"<span class="nav-count">18</span>"#));
    }

    #[test]
    fn empty_stock_renders_all_zeros_and_empty_states_no_panic() {
        let r = Renderer::new(Catalog::load("en"));
        let html = r.render("dashboard.html", "en", "", empty_ctx()).unwrap();
        assert!(html.contains("0.00"));
        assert!(html.contains(r#"0 <span class="kpi-unit">kg</span>"#));
        assert!(html.contains("No stock yet."));
        assert!(html.contains("Nothing running low."));
        assert!(!html.contains("dashboard.breakdown."));
        assert!(!html.contains("dashboard.soon_empty."));
    }

    #[test]
    fn page_localises_to_french_no_raw_keys() {
        let r = Renderer::new(Catalog::load("en"));
        let html = r.render("dashboard.html", "fr", "", empty_ctx()).unwrap();
        assert!(html.contains("Tableau de bord"));
        assert!(html.contains("Répartition par matériau"));
        assert!(html.contains("Bientôt vides"));
        assert!(html.contains("poids restant"));
        assert!(html.contains("seuil 15 %"));
        assert!(html.contains("Aucun stock pour le moment."));
        assert!(!html.contains("dashboard.breakdown."));
        assert!(!html.contains("dashboard.soon_empty."));
    }

    #[test]
    fn no_humidity_section_rendered() {
        let r = Renderer::new(Catalog::load("en"));
        let html = r.render("dashboard.html", "en", "", full_ctx()).unwrap();
        assert!(!html.to_lowercase().contains("humid"));
        assert!(!html.to_lowercase().contains("drybox"));
    }

    // --- View-mapping unit tests: `DashboardOverview` -> `DashboardView`.

    fn sample_overview() -> DashboardOverview {
        DashboardOverview {
            stock_value: Money::new(2500, 2).unwrap(),
            total_remaining: Grams::new(1234.0).unwrap(),
            total_count: 2,
            active_count: 1,
            empty_count: 1,
            alert_count: 1,
            material_breakdown: vec![MaterialStockRow {
                material_name: "PLA".into(),
                spool_count: 2,
                remaining_weight: Grams::new(500.0).unwrap(),
                bar_fraction: 1.0,
            }],
            soon_empty: vec![SoonEmptyItem {
                spool_id: "01HSP".into(),
                manufacturer_name: Some("Prusament".into()),
                material_name: "PLA".into(),
                colour_hex: "#1A9E4B".into(),
                colour_name: None,
                location_name: None,
                remaining_weight: Grams::new(50.0).unwrap(),
                remaining_ratio: 0.05,
            }],
        }
    }

    #[test]
    fn view_maps_kg_percent_and_money_formatting() {
        let renderer = Renderer::new(Catalog::load("en"));
        let view = DashboardView::localized(sample_overview(), &renderer, "en");
        assert_eq!(view.stock_value, "25.00");
        assert_eq!(view.remaining_kg, 1.23);
        assert_eq!(view.material_breakdown[0].remaining_kg, 0.5);
        assert_eq!(view.material_breakdown[0].bar_pct, 100);
        assert_eq!(view.soon_empty[0].remaining_pct, 5);
        assert_eq!(
            view.soon_empty[0].manufacturer_name.as_deref(),
            Some("Prusament")
        );
        assert_eq!(view.soon_empty[0].colour_label, "Green");
    }

    #[test]
    fn view_of_empty_overview_is_all_zeros() {
        let overview =
            DashboardOverview::from_rows(Vec::new(), domain::shared::LowStockThreshold::default());
        let renderer = Renderer::new(Catalog::load("en"));
        let view = DashboardView::localized(overview, &renderer, "en");
        // `Money`'s `Display` always renders to the cent (TD-011), so zero
        // stock shows "0.00", not a raw "0".
        assert_eq!(view.stock_value, "0.00");
        assert_eq!(view.remaining_kg, 0.0);
        assert_eq!(view.total_count, 0);
        assert!(view.material_breakdown.is_empty());
        assert!(view.soon_empty.is_empty());
    }

    // --- Handler-level test: exercises `index` directly against a stub-backed
    // `AppState`, mirroring `web::spools`'s `handlers` test module.
    mod handlers {
        use super::*;
        use crate::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
        use axum::body::to_bytes;
        use domain::dashboard::stubs::StubDashboardRepository;
        use domain::dashboard::{DashboardService, DashboardUseCases, SpoolStockRow, StockStatus};
        use domain::locations::stubs::StubLocationRepository;
        use domain::locations::{LocationsService, LocationsUseCases};
        use domain::materials::stubs::StubMaterialRepository;
        use domain::materials::{MaterialsService, MaterialsUseCases};
        use domain::spools::stubs::StubSpoolRepository;
        use domain::spools::{SpoolsService, SpoolsUseCases};
        use sqlx::PgPool;
        use std::sync::Arc;

        fn test_state(dashboard: Arc<dyn DashboardUseCases>) -> AppState {
            let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(Arc::new(
                StubMaterialRepository::new(),
            )));
            let spools: Arc<dyn SpoolsUseCases> =
                Arc::new(SpoolsService::new(Arc::new(StubSpoolRepository::new())));
            let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(Arc::new(
                StubLocationRepository::new(),
            )));
            let manufacturers: Arc<dyn domain::manufacturers::ManufacturersUseCases> =
                Arc::new(domain::manufacturers::ManufacturersService::new(Arc::new(
                    domain::manufacturers::stubs::StubManufacturerRepository::new(),
                )));
            let db = PgPool::connect_lazy("postgres://user:pass@localhost/db").unwrap();
            let cfg = Config {
                server: ServerConfig {
                    bind: "127.0.0.1:0".into(),
                },
                database: DatabaseConfig {
                    url: "postgres://user:pass@localhost/db".into(),
                },
                i18n: I18nConfig {
                    default_locale: "en".into(),
                },
            };
            AppState::new(
                db,
                &cfg,
                materials,
                spools,
                locations,
                manufacturers,
                dashboard,
                Arc::new(
                    domain::instance_configuration::InstanceConfigurationService::new(Arc::new(
                        domain::instance_configuration::stubs::StubInstanceConfigurationRepository::new(),
                    )),
                ),
                Arc::new(domain::instance_transfer::InstanceTransferService::new(
                    Arc::new(
                        domain::instance_transfer::stubs::StubInstanceTransferRepository::default(),
                    ),
                )),
            )
        }

        async fn body_of(res: Response) -> String {
            let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }

        #[tokio::test]
        async fn get_root_on_empty_stock_returns_200_with_all_zeros_no_panic() {
            let repo = StubDashboardRepository::new();
            let dashboard: Arc<dyn DashboardUseCases> =
                Arc::new(DashboardService::new(Arc::new(repo)));
            let st = test_state(dashboard);
            let res = index(State(st), HeaderMap::new()).await;
            assert_eq!(res.status(), axum::http::StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("No stock yet."));
            assert!(html.contains("Nothing running low."));
            assert!(html.contains(r#"<span class="nav-count">0</span>"#));
            assert!(!html.contains("dashboard.breakdown."));
            assert!(!html.contains("dashboard.soon_empty."));
        }

        #[tokio::test]
        async fn get_root_on_seeded_stock_renders_kpis_and_regions() {
            let row = SpoolStockRow {
                spool_id: "01HSP".to_string(),
                material_id: MaterialId::new("m1"),
                material_name: "PLA".to_string(),
                manufacturer_name: Some("Prusament".to_string()),
                colour_hex: "#1A9E4B".to_string(),
                colour_name: Some("vert sapin".to_string()),
                status: StockStatus::Open,
                remaining_weight: Grams::new(50.0).unwrap(),
                net_weight: Grams::new(1000.0).unwrap(),
                price_paid: Money::new(2000, 2).unwrap(),
                location_name: None,
            };
            let repo = StubDashboardRepository::with(vec![row]);
            let dashboard: Arc<dyn DashboardUseCases> =
                Arc::new(DashboardService::new(Arc::new(repo)));
            let st = test_state(dashboard);
            let res = index(State(st), HeaderMap::new()).await;
            assert_eq!(res.status(), axum::http::StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("PLA"));
            assert!(html.contains("/spools/01HSP"));
            assert!(html.contains(r#"id="kpi-alerts""#));
            assert!(!html.contains("dashboard.kpi."));
        }

        #[tokio::test]
        async fn get_root_uses_configured_low_stock_threshold() {
            let row = SpoolStockRow {
                spool_id: "01HSP".to_string(),
                material_id: MaterialId::new("m1"),
                material_name: "PLA".to_string(),
                manufacturer_name: None,
                colour_hex: "#1A9E4B".to_string(),
                colour_name: None,
                status: StockStatus::Open,
                remaining_weight: Grams::new(200.0).unwrap(),
                net_weight: Grams::new(1000.0).unwrap(),
                price_paid: Money::new(2000, 2).unwrap(),
                location_name: None,
            };
            let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(Arc::new(
                StubDashboardRepository::with(vec![row]),
            )));
            let st = test_state(dashboard);
            st.instance_configuration
                .update_low_stock_threshold(domain::shared::LowStockThreshold::new(20).unwrap())
                .await
                .unwrap();

            let res = index(State(st), HeaderMap::new()).await;
            assert_eq!(res.status(), axum::http::StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains(r#"<div class="kpi-value">1</div>"#));
            assert!(html.contains("threshold 20 %"));
            assert!(html.contains("/spools/01HSP"));
        }
    }
}
