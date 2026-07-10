//! e2e — drive the materials routes through the full Axum router against a
//! real, seeded Postgres (ADR-0003). Unlike `it_materials.rs` (repository
//! only) or `web::materials` unit tests (renderer only), this exercises the
//! whole stack: router -> handler -> use case -> SQLx repository -> DB.

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use domain::dashboard::{DashboardRepository, DashboardService, DashboardUseCases};
use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
use filature::persistence::dashboard::SqlxDashboardRepository;
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use filature::{persistence, web};
use std::sync::Arc;
use tower::ServiceExt; // oneshot

fn test_config(database_url: &str) -> Config {
    Config {
        server: ServerConfig {
            bind: "127.0.0.1:0".into(),
        },
        database: DatabaseConfig {
            url: database_url.into(),
        },
        i18n: I18nConfig {
            default_locale: "en".into(),
        },
    }
}

/// Builds the full app against a real seeded Postgres — the real
/// `SqlxMaterialRepository` behind `MaterialsService`, seeded via
/// `seed_defaults()` so `GET /materials` returns real rows. Idempotent, so
/// it's safe to call from multiple tests sharing the per-binary container.
async fn seeded_app() -> axum::Router {
    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let repo: Arc<dyn MaterialRepository> = Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(repo));
    materials.seed_defaults().await.unwrap(); // idempotent — safe under the shared container
    let spool_repo: Arc<dyn SpoolRepository> = Arc::new(SqlxSpoolRepository::new(db.clone()));
    let spools: Arc<dyn SpoolsUseCases> = Arc::new(SpoolsService::new(spool_repo));
    let location_repo: Arc<dyn LocationRepository> =
        Arc::new(SqlxLocationRepository::new(db.clone()));
    let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(location_repo));
    let dash_repo: Arc<dyn DashboardRepository> =
        Arc::new(SqlxDashboardRepository::new(db.clone()));
    let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(dash_repo));
    web::router(web::AppState::new(
        db,
        &test_config(&url),
        materials,
        spools,
        locations,
        dashboard,
    ))
}

#[tokio::test]
async fn get_materials_lists_seeded_rows() {
    let app = seeded_app().await;
    let res = app
        .oneshot(Request::get("/materials").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("PLA"));
    assert!(html.contains("PA-CF"));
}

#[tokio::test]
async fn post_materials_adds_a_row() {
    let app = seeded_app().await;
    let form = "name=FOO&density=1.10&drying_temp_c=50&drying_time_h=5&sensitivity=Low&nozzle_c=215&bed_c=60";
    let res = app
        .oneshot(
            Request::post("/materials")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("FOO"));
}
