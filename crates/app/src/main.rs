use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use filature::{config::Config, persistence, web};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

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

    let app = web::router(web::AppState::new(db, &cfg, materials, spools, locations));
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    tracing::info!(bind = %cfg.server.bind, "filature listening");
    axum::serve(listener, app).await?;
    Ok(())
}
