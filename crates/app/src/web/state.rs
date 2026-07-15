use crate::config::Config;
use crate::persistence::Db;
use crate::web::i18n::Catalog;
use crate::web::templates::Renderer;
use domain::dashboard::DashboardUseCases;
use domain::instance_configuration::InstanceConfigurationUseCases;
use domain::instance_transfer::InstanceTransferUseCases;
use domain::locations::LocationsUseCases;
use domain::manufacturers::ManufacturersUseCases;
use domain::materials::MaterialsUseCases;
use domain::printers::{PrintersService, PrintersUseCases};
use domain::spools::{SpoolFilter, SpoolsUseCases};
use std::sync::Arc;

/// Wires the DB pool + renderer + default locale +
/// materials/spools/locations/dashboard use cases into the driving (Axum)
/// adapter. Cloned per request by Axum's `State` extractor — cheap: the pool
/// is a handle, the renderer's `Tera` engine is `Arc`-shared, and
/// `materials`/`spools`/`locations`/`dashboard` are `Arc<dyn _>`.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub renderer: Renderer,
    pub default_locale: String,
    pub materials: Arc<dyn MaterialsUseCases>,
    pub spools: Arc<dyn SpoolsUseCases>,
    pub locations: Arc<dyn LocationsUseCases>,
    pub printers: Arc<dyn PrintersUseCases>,
    pub manufacturers: Arc<dyn ManufacturersUseCases>,
    pub dashboard: Arc<dyn DashboardUseCases>,
    pub instance_configuration: Arc<dyn InstanceConfigurationUseCases>,
    pub instance_transfer: Arc<dyn InstanceTransferUseCases>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Db,
        cfg: &Config,
        materials: Arc<dyn MaterialsUseCases>,
        spools: Arc<dyn SpoolsUseCases>,
        locations: Arc<dyn LocationsUseCases>,
        manufacturers: Arc<dyn ManufacturersUseCases>,
        dashboard: Arc<dyn DashboardUseCases>,
        instance_configuration: Arc<dyn InstanceConfigurationUseCases>,
        instance_transfer: Arc<dyn InstanceTransferUseCases>,
    ) -> Self {
        let catalog = Catalog::load(&cfg.i18n.default_locale);
        let printer_repo = Arc::new(crate::persistence::printers::SqlxPrinterRepository::new(
            db.clone(),
        ));
        Self {
            db,
            renderer: Renderer::new(catalog),
            default_locale: cfg.i18n.default_locale.clone(),
            materials,
            spools,
            locations,
            printers: Arc::new(PrintersService::new(printer_repo)),
            manufacturers,
            dashboard,
            instance_configuration,
            instance_transfer,
        }
    }

    pub(crate) async fn nav_spool_count(&self) -> u64 {
        self.spools.count(SpoolFilter::default()).await.unwrap_or(0)
    }

    pub(crate) async fn nav_printer_count(&self) -> usize {
        self.printers.list().await.map_or(0, |items| items.len())
    }
}
