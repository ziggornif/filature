//! The driving (Axum) adapter for the spools slice: a read-only, filterable
//! and sortable list (`GET /spools`) whose filter/sort bar issues htmx GETs
//! to `GET /spools/rows`, swapping only the `<tbody>` — no full reload.
//! Mirrors `web::materials` for locale/theme resolution and Tera rendering.
//!
//! The material dropdown is populated by calling `AppState::materials`
//! (the materials use cases already wired into shared state) — this is
//! web-layer composition across two driving-adapter handlers, not a domain
//! import across slices (the spools domain never depends on materials).

use crate::web::router::{resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use domain::shared::MaterialId;
use domain::spools::{SpoolFilter, SpoolListItem, SpoolSort, SpoolStatus};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Template-shaped view of a `SpoolListItem`: plain strings/numbers plus the
/// derived percentage/length fields (the domain never exposes a "view
/// model").
#[derive(Serialize)]
pub struct SpoolView {
    pub id: String,
    pub material_name: String,
    pub colour_hex: String,
    /// The colour's human name if set, else the hex code — always
    /// something displayable (e.g. as a swatch `title`).
    pub colour_label: String,
    pub diameter: String, // "1.75" | "2.85"
    pub remaining_weight: f64,
    pub net_weight: f64,
    pub remaining_pct: u8,
    pub remaining_length_m: f64,
    pub status: String, // "Sealed" | "Open" | "Empty" | "Archived"
}

impl From<SpoolListItem> for SpoolView {
    fn from(item: SpoolListItem) -> Self {
        let remaining_pct = (item.remaining_ratio() * 100.0).round();
        let remaining_length_m = round1(item.remaining_length_m());
        Self {
            id: item.id.as_str().to_string(),
            material_name: item.material_name.clone(),
            colour_hex: item.colour.hex().to_string(),
            colour_label: item
                .colour
                .name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| item.colour.hex().to_string()),
            diameter: item.diameter.as_str().to_string(),
            remaining_weight: round1(item.remaining_weight.value()),
            net_weight: round1(item.net_weight.value()),
            remaining_pct: remaining_pct as u8, // saturating cast — no panic on out-of-range ratios
            remaining_length_m,
            status: item.status.as_str().to_string(),
        }
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// A material option for the filter dropdown — the web layer's own
/// composition of `AppState::materials`, not a domain read model.
#[derive(Serialize)]
pub struct MaterialOption {
    pub id: String,
    pub name: String,
}

/// Query params accepted by both `GET /spools` and `GET /spools/rows`.
/// `#[serde(default)]` so any/all of them may be absent from the query
/// string (a fresh page load supplies none).
#[derive(Debug, Deserialize, Default)]
pub struct SpoolQuery {
    #[serde(default)]
    pub material_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
}

impl SpoolQuery {
    fn to_filter(&self) -> SpoolFilter {
        SpoolFilter {
            material_id: self
                .material_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| MaterialId::new(s.to_string())),
            status: self
                .status
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| SpoolStatus::parse(s).ok()),
        }
    }

    fn to_sort(&self) -> SpoolSort {
        match self.sort.as_deref() {
            Some("remaining_asc") => SpoolSort::RemainingRatioAsc,
            Some("remaining_desc") => SpoolSort::RemainingRatioDesc,
            _ => SpoolSort::CreatedDesc,
        }
    }

    fn selected_sort(&self) -> &str {
        match self.sort.as_deref() {
            Some("remaining_asc") => "remaining_asc",
            Some("remaining_desc") => "remaining_desc",
            _ => "created_desc",
        }
    }
}

async fn material_options(st: &AppState) -> Result<Vec<MaterialOption>, Response> {
    st.materials
        .list()
        .await
        .map(|ms| {
            ms.into_iter()
                .map(|m| MaterialOption {
                    id: m.id.as_str().to_string(),
                    name: m.name.as_str().to_string(),
                })
                .collect()
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
}

fn render_rows(st: &AppState, locale: &str, items: Vec<SpoolListItem>) -> Response {
    let views: Vec<SpoolView> = items.into_iter().map(Into::into).collect();
    let mut ctx = Context::new();
    ctx.insert("spools", &views);
    match st.renderer.render("_spool_rows.html", locale, "", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn list_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let materials = match material_options(&st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };

    match st.spools.list(q.to_filter(), q.to_sort()).await {
        Ok(items) => {
            let views: Vec<SpoolView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("spools", &views);
            ctx.insert("materials", &materials);
            ctx.insert("selected_material", q.material_id.as_deref().unwrap_or(""));
            ctx.insert("selected_status", q.status.as_deref().unwrap_or(""));
            ctx.insert("selected_sort", q.selected_sort());
            match st
                .renderer
                .render("spools.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn rows(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    match st.spools.list(q.to_filter(), q.to_sort()).await {
        Ok(items) => render_rows(&st, &locale, items),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/spools", get(list_page))
        .route("/spools/rows", get(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn view(id: &str, status: &str) -> SpoolView {
        SpoolView {
            id: id.into(),
            material_name: "PLA".into(),
            colour_hex: "#1A9E4B".into(),
            colour_label: "vert sapin".into(),
            diameter: "1.75".into(),
            remaining_weight: 800.0,
            net_weight: 1000.0,
            remaining_pct: 80,
            remaining_length_m: 268.2,
            status: status.into(),
        }
    }

    fn material_option() -> MaterialOption {
        MaterialOption {
            id: "01HMAT".into(),
            name: "PLA".into(),
        }
    }

    fn render_list(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Sealed")]);
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("selected_material", "");
        ctx.insert("selected_status", "");
        ctx.insert("selected_sort", "created_desc");
        r.render("spools.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn list_page_shows_spool_and_material_option_no_raw_keys() {
        let html = render_list("en");
        assert!(html.contains("PLA"));
        assert!(html.contains("80")); // remaining pct
        assert!(html.contains("#1A9E4B"));
        assert!(html.contains("/spools/01HSP"));
        assert!(!html.contains("spools.col.")); // no raw i18n key leaks
        assert!(!html.contains("spools.status.")); // status label resolved
    }

    #[test]
    fn list_page_localises_to_french() {
        let html = render_list("fr");
        assert!(html.contains("Bobines") || html.contains("Matériau"));
        assert!(!html.contains("spools.col."));
    }

    #[test]
    fn rows_fragment_renders_only_rows_no_page_shell() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Open")]);
        let html = r.render("_spool_rows.html", "en", "", ctx).unwrap();
        assert!(html.contains("01HSP"));
        assert!(!html.contains("<html")); // fragment only, no full page shell
        assert!(!html.contains("<table")); // tbody content only, no wrapper
    }

    #[test]
    fn query_maps_status_and_material_filter() {
        let q = SpoolQuery {
            material_id: Some("01HMAT".into()),
            status: Some("Open".into()),
            sort: Some("remaining_asc".into()),
        };
        let filter = q.to_filter();
        assert_eq!(filter.material_id, Some(MaterialId::new("01HMAT")));
        assert_eq!(filter.status, Some(SpoolStatus::Open));
        assert_eq!(q.to_sort(), SpoolSort::RemainingRatioAsc);
    }

    #[test]
    fn empty_query_means_no_filter_and_created_desc_sort() {
        let q = SpoolQuery::default();
        let filter = q.to_filter();
        assert_eq!(filter.material_id, None);
        assert_eq!(filter.status, None);
        assert_eq!(q.to_sort(), SpoolSort::CreatedDesc);
        assert_eq!(q.selected_sort(), "created_desc");
    }
}
