use domain::dashboard::{DashboardRepository, DashboardService, DashboardUseCases};
use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::manufacturers::{ManufacturerRepository, ManufacturersService, ManufacturersUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
use filature::persistence::dashboard::SqlxDashboardRepository;
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::manufacturers::SqlxManufacturerRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use filature::{config::Config, persistence, web};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Log level is config-driven via the `RUST_LOG` env var (the standard
    // 12-factor / docker-compose knob); absent that, a sensible default keeps
    // request/response traces on without drowning in sqlx query logs.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug,sqlx=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cfg = Config::load("filature.toml")?;
    let db = persistence::connect_and_migrate(&cfg.database.url).await?;

    let repo: Arc<dyn MaterialRepository> = Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(repo));
    materials.seed_defaults().await?;

    let spool_repo: Arc<dyn SpoolRepository> = Arc::new(SqlxSpoolRepository::new(db.clone()));
    let spools: Arc<dyn SpoolsUseCases> = Arc::new(SpoolsService::new(spool_repo));

    let location_repo: Arc<dyn LocationRepository> =
        Arc::new(SqlxLocationRepository::new(db.clone()));
    let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(location_repo));

    let manufacturer_repo: Arc<dyn ManufacturerRepository> =
        Arc::new(SqlxManufacturerRepository::new(db.clone()));
    let manufacturers: Arc<dyn ManufacturersUseCases> =
        Arc::new(ManufacturersService::new(manufacturer_repo));
    manufacturers.seed_defaults().await?;

    let dash_repo: Arc<dyn DashboardRepository> =
        Arc::new(SqlxDashboardRepository::new(db.clone()));
    let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(dash_repo));

    let app = web::router(web::AppState::new(
        db,
        &cfg,
        materials,
        spools,
        locations,
        manufacturers,
        dashboard,
    ))
    // Per-request tracing spans (method, path, status, latency) — the
    // structured request observability TD-002 called for. Verbosity is
    // governed by the `tower_http` target in the env filter above.
    .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    tracing::info!(bind = %cfg.server.bind, "filature listening");
    axum::serve(listener, app).await?;
    Ok(())
}
