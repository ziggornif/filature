//! The driving (Axum) adapter for the locations slice: an htmx editable
//! table (`GET /locations`) whose rows are edited/created/deleted in place
//! via row-fragment responses (`POST /locations`, `PUT /locations/{id}`,
//! `DELETE /locations/{id}`) — mirrors `web::materials`.

use crate::web::router::{resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use domain::locations::{Location, LocationName, NewLocation, RepositoryError, note_from};
use domain::shared::{DomainError, LocationId};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Template-shaped view of a `Location` paired with its assigned-spool
/// count (from `LocationsUseCases::list_with_spool_counts`) — the domain
/// never exposes a bespoke "view model", so the pairing happens here.
#[derive(Serialize)]
pub struct LocationView {
    pub id: String,
    pub name: String,
    pub note: String,
    pub spool_count: i64,
}

impl From<(Location, u64)> for LocationView {
    fn from((l, count): (Location, u64)) -> Self {
        Self {
            id: l.id.as_str().to_string(),
            name: l.name.as_str().to_string(),
            note: l.note.unwrap_or_default(),
            spool_count: count as i64,
        }
    }
}

/// The htmx form payload for both create (`POST /locations`) and edit
/// (`PUT /locations/{id}`) — field names must match the `<input name=...>`
/// attributes in `_location_row.html`.
#[derive(Deserialize)]
pub struct LocationForm {
    pub name: String,
    pub note: String,
}

impl LocationForm {
    /// Maps the raw form into a domain `NewLocation`, rejecting a blank name
    /// with a plain client-facing error message (the caller turns this into
    /// a 422) rather than panicking or 500-ing. The note is normalised via
    /// `note_from` (trims; empty ⇒ `None`).
    fn to_new(&self) -> Result<NewLocation, String> {
        Ok(NewLocation {
            name: LocationName::new(self.name.clone()).map_err(|e| e.to_string())?,
            note: note_from(&self.note),
        })
    }
}

fn not_found(st: &AppState, locale: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(st.renderer.t(locale, "locations.not_found")),
    )
        .into_response()
}

fn render_row(st: &AppState, locale: &str, status: StatusCode, view: LocationView) -> Response {
    let mut ctx = Context::new();
    ctx.insert("l", &view);
    match st.renderer.render("_location_row.html", locale, "", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Renders the standalone `#locations-msg` slot (`_locations_msg.html`) with
/// `message` — used for the 409 delete-blocked response, which the htmx
/// `response-targets` extension routes to that slot via `hx-target-409`
/// (see `_location_row.html`'s delete button), leaving the table untouched.
fn render_message(st: &AppState, locale: &str, status: StatusCode, message: String) -> Response {
    let mut ctx = Context::new();
    ctx.insert("message", &message);
    match st.renderer.render("_locations_msg.html", locale, "", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Looks up `id`'s current spool count via `list_with_spool_counts` — used
/// after a successful edit (whose `Ok(Location)` carries no count). Falls
/// back to `0` if the location can't be found or the lookup errors, rather
/// than failing the whole request over a display-only count.
async fn spool_count_for(st: &AppState, id: &LocationId) -> u64 {
    st.locations
        .list_with_spool_counts()
        .await
        .ok()
        .and_then(|all| all.into_iter().find(|(l, _)| l.id == *id).map(|(_, c)| c))
        .unwrap_or(0)
}

async fn list_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.locations.list_with_spool_counts().await {
        Ok(items) => {
            let views: Vec<LocationView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("locations", &views);
            match st
                .renderer
                .render("locations.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LocationForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let new = match f.to_new() {
        Ok(n) => n,
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, e).into_response(),
    };
    match st.locations.add(new).await {
        Ok(l) => {
            let view: LocationView = (l, 0u64).into(); // freshly created: no spools yet
            render_row(&st, &locale, StatusCode::OK, view)
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn edit(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(f): Form<LocationForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let new = match f.to_new() {
        Ok(n) => n,
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, e).into_response(),
    };
    let location = Location {
        id: LocationId::new(id),
        name: new.name,
        note: new.note,
    };
    match st.locations.edit(location).await {
        Ok(l) => {
            let count = spool_count_for(&st, &l.id).await;
            let view: LocationView = (l, count).into();
            render_row(&st, &locale, StatusCode::OK, view)
        }
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `DELETE /locations/{id}` — removes the location if it has no spools
/// assigned. 200 with an empty body on success (the htmx row swap removes
/// the `<tr>`); 409 with an i18n-formatted "N spools assigned" message when
/// `LocationsUseCases::delete` refuses (`DomainError::LocationInUse`) —
/// `t()` has no param interpolation, so the count (already carried by the
/// domain error — no extra lookup needed) is substituted into the localized
/// template string here. The 409 body is routed to the dedicated
/// `#locations-msg` slot by the htmx `response-targets` extension
/// (`hx-target-409` on the delete button in `_location_row.html`), so the
/// table itself is never touched on this path.
async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let location_id = LocationId::new(id);
    match st.locations.delete(location_id.clone()).await {
        Ok(()) => (StatusCode::OK, Html("")).into_response(),
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(RepositoryError::Domain(DomainError::LocationInUse { count })) => {
            let message = st
                .renderer
                .t(&locale, "locations.delete.blocked")
                .replace("{count}", &count.to_string());
            render_message(&st, &locale, StatusCode::CONFLICT, message)
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/locations", get(list_page).post(create))
        .route("/locations/{id}", axum::routing::put(edit).delete(delete))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn view() -> LocationView {
        LocationView {
            id: "01HZID".into(),
            name: "Shelf A".into(),
            note: "Near the door".into(),
            spool_count: 3,
        }
    }

    fn render(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("locations", &vec![view()]);
        r.render("locations.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn table_shows_name_note_and_count_no_raw_keys() {
        let html = render("en");
        assert!(html.contains("Shelf A"));
        assert!(html.contains("Near the door"));
        assert!(html.contains('3')); // spool_count
        assert!(!html.contains("locations.")); // no raw i18n key leaks
    }

    #[test]
    fn table_localises_to_french() {
        let html = render("fr");
        assert!(html.contains("Emplacement") || html.contains("Remarque"));
        assert!(!html.contains("locations."));
    }

    #[test]
    fn locations_msg_renders_the_interpolated_count_no_raw_keys() {
        // Mirrors what `delete()` renders into `#locations-msg` on a 409 —
        // the i18n template string with `{count}` substituted before render.
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert(
            "message",
            "This location has 3 spool(s) assigned and cannot be deleted.",
        );
        let html = r.render("_locations_msg.html", "en", "", ctx).unwrap();
        assert!(html.contains(r#"id="locations-msg""#));
        assert!(html.contains("3 spool(s) assigned"));
        assert!(!html.contains("locations."));
    }
}
