use crate::web::state::AppState;
use crate::web::theme::Theme;
use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;
use tera::Context;

#[derive(RustEmbed)]
#[folder = "assets/static"]
struct StaticAssets;

/// Resolve locale from the `lang` cookie, else the configured default.
/// An unknown cookie locale (no catalog for it) falls back to the default, so a
/// bogus `lang=zz` never reaches the `lang` attribute or the `t` lookups.
///
/// `pub(crate)`: shared with other driving-adapter handlers (e.g.
/// `web::materials`) so cookie/locale/theme resolution has one implementation.
pub(crate) fn resolve_locale(headers: &HeaderMap, st: &AppState) -> String {
    read_cookie(headers, "lang")
        .filter(|loc| st.renderer.knows_locale(loc))
        .unwrap_or_else(|| st.default_locale.clone())
}

pub(crate) fn resolve_theme(headers: &HeaderMap) -> Theme {
    Theme::from_cookie(read_cookie(headers, "theme").as_deref())
}

/// Logs an internal error server-side (`tracing::error!`) and returns a generic
/// 500 to the client. Raw error detail (e.g. sqlx text) must never reach the
/// response body — the operator reads it from the server logs instead (AR-002).
///
/// `pub(crate)`: the single 500-path builder shared by every driving-adapter
/// handler, so no call site reconstructs the leak by hand.
pub(crate) fn internal_error<E: std::fmt::Display>(e: E) -> Response {
    tracing::error!(error = %e, "internal server error");
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
}

/// Renders a form-validation error into the page's shared message slot
/// (`#…-msg`) as an escaped fragment (`_form_error.html`), tagged with an
/// `HX-Reswap: innerHTML` header (TD-009).
///
/// The htmx `response-targets` extension retargets the 4xx to `#…-msg` (via
/// each control's `hx-target-error`) but otherwise reuses the control's own
/// `hx-swap` — which differs between the inline edit inputs (`outerHTML`) and
/// the add form (`beforeend`). Forcing `innerHTML` here normalizes both so the
/// message always fills the slot cleanly instead of nesting or wiping it.
///
/// `pub(crate)`: shared by the materials/locations create/edit handlers so the
/// error-feedback wiring has one implementation.
pub(crate) fn form_error(
    st: &AppState,
    locale: &str,
    status: StatusCode,
    message: &str,
) -> Response {
    let mut ctx = Context::new();
    ctx.insert("message", message);
    match st.renderer.render("_form_error.html", locale, "", ctx) {
        Ok(html) => (
            status,
            [(
                header::HeaderName::from_static("hx-reswap"),
                header::HeaderValue::from_static("innerHTML"),
            )],
            Html(html),
        )
            .into_response(),
        Err(e) => internal_error(e),
    }
}

pub(crate) fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

async fn static_handler(Path(path): Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(crate::web::dashboard::routes())
        .merge(crate::web::materials::routes())
        .merge(crate::web::printers::routes())
        .merge(crate::web::locations::routes())
        .merge(crate::web::manufacturers::routes())
        .merge(crate::web::spools::routes())
        .merge(crate::web::settings::routes())
        .route("/static/{*path}", get(static_handler))
        .with_state(state)
}
