use crate::config::Config;
use crate::persistence::Db;
use crate::web::i18n::Catalog;
use crate::web::templates::Renderer;

/// Wires the DB pool + renderer + default locale into the driving (Axum)
/// adapter. Cloned per request by Axum's `State` extractor — cheap: the pool
/// is a handle and the renderer's `Tera` engine is `Arc`-shared.
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
