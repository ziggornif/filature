//! The driving (Axum) adapter for manufacturer htmx mutations. The table is
//! rendered by the settings shell; rows are created/deleted in place via
//! row-fragment responses (`POST /manufacturers`, `DELETE
//! /manufacturers/{id}`) — mirrors `web::locations`, minus edit (a brand is
//! just a name + country; the operator re-creates rather than renames).

use crate::web::router::{internal_error, resolve_locale};
use crate::web::state::AppState;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use domain::manufacturers::{
    Manufacturer, ManufacturerName, NewManufacturer, RepositoryError, country_from,
};
use domain::shared::{DomainError, ManufacturerId};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Template-shaped view of a `Manufacturer` paired with its referencing-spool
/// count (from `ManufacturersUseCases::list_with_spool_counts`).
#[derive(Serialize)]
pub struct ManufacturerView {
    pub id: String,
    pub name: String,
    pub country: String,
    pub spool_count: i64,
}

impl From<(Manufacturer, u64)> for ManufacturerView {
    fn from((m, count): (Manufacturer, u64)) -> Self {
        Self {
            id: m.id.as_str().to_string(),
            name: m.name.as_str().to_string(),
            country: m.country.unwrap_or_default(),
            spool_count: count as i64,
        }
    }
}

/// The htmx form payload for create (`POST /manufacturers`) — field names
/// must match the `<input name=...>` attributes in the add form.
#[derive(Deserialize)]
pub struct ManufacturerForm {
    pub name: String,
    #[serde(default)]
    pub country: String,
}

impl ManufacturerForm {
    fn to_new(&self) -> Result<NewManufacturer, String> {
        Ok(NewManufacturer {
            name: ManufacturerName::new(self.name.clone()).map_err(|e| e.to_string())?,
            country: country_from(&self.country),
        })
    }
}

fn not_found(st: &AppState, locale: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(st.renderer.t(locale, "manufacturers.not_found")),
    )
        .into_response()
}

fn render_row(st: &AppState, locale: &str, status: StatusCode, view: ManufacturerView) -> Response {
    let mut ctx = Context::new();
    ctx.insert("m", &view);
    match st
        .renderer
        .render("_manufacturer_row.html", locale, "", ctx)
    {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Renders the standalone `#manufacturers-msg` slot with `message` — used for
/// the 409 delete-blocked response, routed there by htmx `response-targets`
/// via `hx-target-409` (see the delete button in `_manufacturer_row.html`).
fn render_message(st: &AppState, locale: &str, status: StatusCode, message: String) -> Response {
    let mut ctx = Context::new();
    ctx.insert("message", &message);
    match st
        .renderer
        .render("_manufacturers_msg.html", locale, "", ctx)
    {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<ManufacturerForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let new = match f.to_new() {
        Ok(n) => n,
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, e).into_response(),
    };
    match st.manufacturers.add(new).await {
        Ok(m) => {
            let view: ManufacturerView = (m, 0u64).into(); // freshly created: no spools yet
            render_row(&st, &locale, StatusCode::OK, view)
        }
        Err(RepositoryError::Duplicate(_)) => {
            let message = st.renderer.t(&locale, "manufacturers.duplicate");
            render_message(&st, &locale, StatusCode::CONFLICT, message)
        }
        Err(e) => internal_error(e),
    }
}

/// `DELETE /manufacturers/{id}` — removes the manufacturer if no spool
/// references it. 200 empty body on success; 409 with an i18n "N spools"
/// message when `ManufacturersUseCases::delete` refuses
/// (`DomainError::ManufacturerInUse`), routed to `#manufacturers-msg`.
async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let manufacturer_id = ManufacturerId::new(id);
    match st.manufacturers.delete(manufacturer_id.clone()).await {
        Ok(()) => (StatusCode::OK, Html("")).into_response(),
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(RepositoryError::Domain(DomainError::ManufacturerInUse { count })) => {
            let message = st
                .renderer
                .t(&locale, "manufacturers.delete.blocked")
                .replace("{count}", &count.to_string());
            render_message(&st, &locale, StatusCode::CONFLICT, message)
        }
        Err(e) => internal_error(e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/manufacturers",
            axum::routing::get(|| async { StatusCode::NOT_FOUND }).post(create),
        )
        .route("/manufacturers/{id}", axum::routing::delete(delete))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn view() -> ManufacturerView {
        ManufacturerView {
            id: "01HZID".into(),
            name: "Prusament".into(),
            country: "CZ".into(),
            spool_count: 3,
        }
    }

    fn render(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("manufacturers", &vec![view()]);
        r.render("_settings_manufacturers.html", locale, "", ctx)
            .unwrap()
    }

    #[test]
    fn table_shows_name_country_and_count_no_raw_keys() {
        let html = render("en");
        assert!(html.contains("Prusament"));
        assert!(html.contains("CZ"));
        assert!(html.contains('3')); // spool_count
        assert!(!html.contains("manufacturers.")); // no raw i18n key leaks
    }

    #[test]
    fn table_localises_to_french() {
        let html = render("fr");
        assert!(html.contains("Fabricant") || html.contains("Pays"));
        assert!(!html.contains("manufacturers."));
    }

    #[test]
    fn table_uses_the_shared_referential_style_without_breaking_delete() {
        let html = render("en");
        assert!(html.contains(r#"class="table-scroll referential-table-scroll""#));
        assert!(html.contains(r#"class="manufacturers-table referential-table""#));
        assert!(html.contains(r#"class="referential-name-badge">Prusament</span>"#));
        assert!(html.contains(r#"hx-delete="/manufacturers/01HZID""#));
        assert!(html.contains(r##"hx-target-409="#manufacturers-msg""##));
    }
}
