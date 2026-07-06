use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use filature::persistence::materials::SqlxMaterialRepository;
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

    let app = web::router(web::AppState::new(db, &cfg, materials));
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    tracing::info!(bind = %cfg.server.bind, "filature listening");
    axum::serve(listener, app).await?;
    Ok(())
}
