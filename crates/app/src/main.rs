use domain::dashboard::{DashboardRepository, DashboardService, DashboardUseCases};
use domain::instance_configuration::{
    InstanceConfigurationRepository, InstanceConfigurationService, InstanceConfigurationUseCases,
};
use domain::instance_transfer::{
    InstanceTransferRepository, InstanceTransferService, InstanceTransferUseCases,
};
use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::manufacturers::{ManufacturerRepository, ManufacturersService, ManufacturersUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
use filature::persistence::dashboard::SqlxDashboardRepository;
use filature::persistence::instance_configuration::SqlxInstanceConfigurationRepository;
use filature::persistence::instance_transfer::SqlxInstanceTransferRepository;
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
    // `filature hash-password <password>` — print an argon2 PHC string for the
    // `[auth]` config table and exit, before any server/DB setup. Keeps the
    // single-binary shape (no separate hashing tool).
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("hash-password") {
        let Some(password) = args.get(2) else {
            eprintln!("usage: filature hash-password <password>");
            std::process::exit(2);
        };
        println!("{}", web::auth::hash_password(password));
        return Ok(());
    }

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

    let instance_configuration_repo: Arc<dyn InstanceConfigurationRepository> =
        Arc::new(SqlxInstanceConfigurationRepository::new(db.clone()));
    let instance_configuration: Arc<dyn InstanceConfigurationUseCases> = Arc::new(
        InstanceConfigurationService::new(instance_configuration_repo),
    );
    // Load once during boot so a persisted configuration failure prevents the
    // service from starting with silently different alert behaviour.
    instance_configuration.get().await?;

    let instance_transfer_repository: Arc<dyn InstanceTransferRepository> =
        Arc::new(SqlxInstanceTransferRepository::new(db.clone()));
    let instance_transfer: Arc<dyn InstanceTransferUseCases> =
        Arc::new(InstanceTransferService::new(instance_transfer_repository));

    // Demo-auth gate (slice 08): load the `[auth]` credential and wrap the app
    // in the login/session layer. Required in production — a missing `[auth]`
    // table fails the boot here rather than silently serving an open instance.
    let auth = web::auth::AuthConfig::load("filature.toml")?;
    let renderer =
        web::templates::Renderer::new(web::i18n::Catalog::load(&cfg.i18n.default_locale));

    let app = web::auth::protect(
        web::router(web::AppState::new(
            db,
            &cfg,
            materials,
            spools,
            locations,
            manufacturers,
            dashboard,
            instance_configuration,
            instance_transfer,
        )),
        auth,
        renderer,
        cfg.i18n.default_locale.clone(),
    )
    // Per-request tracing spans (method, path, status, latency) — the
    // structured request observability TD-002 called for. Verbosity is
    // governed by the `tower_http` target in the env filter above.
    .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    tracing::info!(bind = %cfg.server.bind, "filature listening");
    axum::serve(listener, app).await?;
    Ok(())
}
