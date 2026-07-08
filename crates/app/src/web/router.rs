use crate::web::state::AppState;
use crate::web::theme::Theme;
use axum::{
    Router,
    extract::{Path, State},
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

pub(crate) fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

async fn index(State(st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st
        .renderer
        .render("index.html", &locale, theme.data_attr(), Context::new())
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
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
        .route("/", get(index))
        .merge(crate::web::materials::routes())
        .merge(crate::web::locations::routes())
        .merge(crate::web::spools::routes())
        .route("/static/{*path}", get(static_handler))
        .with_state(state)
}
