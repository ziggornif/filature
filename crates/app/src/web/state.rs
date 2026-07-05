use crate::config::Config;
use crate::persistence::Db;
use crate::web::i18n::Catalog;
use crate::web::templates::Renderer;
use domain::materials::MaterialsUseCases;
use std::sync::Arc;

/// Wires the DB pool + renderer + default locale + materials use cases into
/// the driving (Axum) adapter. Cloned per request by Axum's `State`
/// extractor — cheap: the pool is a handle, the renderer's `Tera` engine is
/// `Arc`-shared, and `materials` is an `Arc<dyn _>`.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub renderer: Renderer,
    pub default_locale: String,
    pub materials: Arc<dyn MaterialsUseCases>,
}

impl AppState {
    pub fn new(db: Db, cfg: &Config, materials: Arc<dyn MaterialsUseCases>) -> Self {
        let catalog = Catalog::load(&cfg.i18n.default_locale);
        Self {
            db,
            renderer: Renderer::new(catalog),
            default_locale: cfg.i18n.default_locale.clone(),
            materials,
        }
    }
}
