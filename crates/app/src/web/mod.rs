pub mod i18n;
pub mod templates;
pub mod theme;

use crate::config::Config;
use crate::persistence::Db;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use i18n::Catalog;
use rust_embed::RustEmbed;
use templates::Renderer;
use tera::Context;
use theme::Theme;

#[derive(RustEmbed)]
#[folder = "assets/static"]
struct StaticAssets;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub renderer: Renderer,
    pub default_locale: String,
}

impl AppState {
    pub fn new(db: Db, cfg: &Config) -> Self {
        let catalog = Catalog::load(&cfg.i18n.default_locale);
        Self {
            db,
            renderer: Renderer::new(catalog),
            default_locale: cfg.i18n.default_locale.clone(),
        }
    }
}

/// Resolve locale from the `lang` cookie, else the configured default.
/// An unknown cookie locale (no catalog for it) falls back to the default, so a
/// bogus `lang=zz` never reaches the `lang` attribute or the `t` lookups.
fn resolve_locale(headers: &HeaderMap, st: &AppState) -> String {
    read_cookie(headers, "lang")
        .filter(|loc| st.renderer.knows_locale(loc))
        .unwrap_or_else(|| st.default_locale.clone())
}
fn resolve_theme(headers: &HeaderMap) -> Theme {
    Theme::from_cookie(read_cookie(headers, "theme").as_deref())
}
fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
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
        .route("/static/{*path}", get(static_handler))
        .with_state(state)
}
