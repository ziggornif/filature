use axum::body::to_bytes;
use axum::http::{header, Request, StatusCode};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
use filature::{persistence, web};
use tower::ServiceExt; // oneshot

fn test_config() -> Config {
    Config {
        server: ServerConfig {
            bind: "127.0.0.1:0".into(),
        },
        database: DatabaseConfig {
            url: "sqlite::memory:".into(),
        },
        i18n: I18nConfig {
            default_locale: "en".into(),
        },
    }
}

async fn app() -> axum::Router {
    let db = persistence::connect_and_migrate("sqlite::memory:")
        .await
        .unwrap();
    web::router(web::AppState::new(db, &test_config()))
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
