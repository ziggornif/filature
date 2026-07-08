mod support;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode, header};
use domain::locations::stubs::StubLocationRepository;
use domain::locations::{LocationsService, LocationsUseCases};
use domain::materials::stubs::StubMaterialRepository;
use domain::materials::{MaterialsService, MaterialsUseCases};
use domain::spools::stubs::StubSpoolRepository;
use domain::spools::{SpoolsService, SpoolsUseCases};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
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

async fn app() -> axum::Router {
    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(Arc::new(
        StubMaterialRepository::new(),
    )));
    let spools: Arc<dyn SpoolsUseCases> =
        Arc::new(SpoolsService::new(Arc::new(StubSpoolRepository::new())));
    let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(Arc::new(
        StubLocationRepository::new(),
    )));
    web::router(web::AppState::new(
        db,
        &test_config(&url),
        materials,
        spools,
        locations,
    ))
}

#[tokio::test]
async fn index_renders_default_locale() {
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Dashboard")); // en default
}

#[tokio::test]
async fn index_honours_lang_cookie() {
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::COOKIE, "lang=fr")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Tableau de bord")); // fr via cookie
}

#[tokio::test]
async fn unknown_lang_cookie_falls_back_to_default() {
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::COOKIE, "lang=zz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains(r#"lang="en""#)); // bogus locale => default en
    assert!(!html.contains(r#"lang="zz""#));
    assert!(html.contains("Dashboard"));
}
