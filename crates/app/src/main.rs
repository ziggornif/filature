use filature::{config::Config, persistence, web};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load("filature.toml")?;
    let db = persistence::connect_and_migrate(&cfg.database.url).await?;
    let app = web::router(web::AppState::new(db, &cfg));
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    println!("filature listening on {}", cfg.server.bind);
    axum::serve(listener, app).await?;
    Ok(())
}
