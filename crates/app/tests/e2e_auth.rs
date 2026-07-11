//! Demo-auth gate (slice 08). Exercises the `web::auth::protect` layer wrapped
//! around the real app router — the same composition `main.rs` ships. The bare
//! `web::router` e2e suites deliberately do NOT apply this layer, so they stay
//! open; auth behaviour is proven only here.

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use domain::dashboard::stubs::StubDashboardRepository;
use domain::dashboard::{DashboardService, DashboardUseCases};
use domain::locations::stubs::StubLocationRepository;
use domain::locations::{LocationsService, LocationsUseCases};
use domain::materials::stubs::StubMaterialRepository;
use domain::materials::{MaterialsService, MaterialsUseCases};
use domain::spools::stubs::StubSpoolRepository;
use domain::spools::{SpoolsService, SpoolsUseCases};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
use filature::web::auth::{self, AuthConfig};
use filature::web::i18n::Catalog;
use filature::web::templates::Renderer;
use filature::{persistence, web};
use std::sync::Arc;
use tower::ServiceExt; // oneshot

const USERNAME: &str = "demo";
const PASSWORD: &str = "s3cret-demo";

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

/// The app router wrapped in the auth gate, exactly as `main.rs` composes it.
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
    let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(Arc::new(
        StubDashboardRepository::new(),
    )));
    let manufacturers: Arc<dyn domain::manufacturers::ManufacturersUseCases> =
        Arc::new(domain::manufacturers::ManufacturersService::new(Arc::new(
            domain::manufacturers::stubs::StubManufacturerRepository::new(),
        )));
    let cfg = test_config(&url);
    let router = web::router(web::AppState::new(
        db,
        &cfg,
        materials,
        spools,
        locations,
        manufacturers,
        dashboard,
    ));
    let auth_cfg = AuthConfig {
        username: USERNAME.into(),
        password_hash: auth::hash_password(PASSWORD),
    };
    let renderer = Renderer::new(Catalog::load("en"));
    auth::protect(router, auth_cfg, renderer, "en".into())
}

fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}

fn get_with_cookie(path: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

fn post_form(path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

/// Extract the `session=…` value from a Set-Cookie header, or None.
fn session_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|c| {
            c.strip_prefix("session=")
                .map(|rest| rest.split(';').next().unwrap_or("").to_string())
        })
}

#[tokio::test]
async fn unauthenticated_root_redirects_to_login() {
    let res = app().await.oneshot(get("/")).await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get(header::LOCATION).unwrap(), "/login");
}

#[tokio::test]
async fn unauthenticated_static_is_also_blocked() {
    // The hard constraint: NO part of the app is served without a session
    // cookie, static assets included.
    let res = app().await.oneshot(get("/static/app.css")).await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get(header::LOCATION).unwrap(), "/login");
}

#[tokio::test]
async fn login_page_is_served_and_self_contained() {
    let res = app().await.oneshot(get("/login")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    // Self-contained: references no /static asset and no htmx script.
    assert!(
        !html.contains("/static/"),
        "login must not pull /static assets"
    );
    assert!(!html.contains("htmx"), "login must not depend on htmx");
    assert!(html.contains("<form"), "login must render a form");
}

#[tokio::test]
async fn good_credentials_set_session_cookie_and_redirect_home() {
    let body = format!("username={USERNAME}&password={PASSWORD}");
    let res = app()
        .await
        .oneshot(post_form("/login", &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get(header::LOCATION).unwrap(), "/");
    let set = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set.starts_with("session="));
    assert!(set.contains("HttpOnly"));
    assert!(set.contains("SameSite=Lax"));
}

#[tokio::test]
async fn bad_password_is_rejected_without_cookie() {
    let body = format!("username={USERNAME}&password=wrong");
    let res = app()
        .await
        .oneshot(post_form("/login", &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(
        res.headers().get(header::SET_COOKIE).is_none(),
        "failed login must not set a session cookie"
    );
}

#[tokio::test]
async fn valid_session_cookie_reaches_the_app() {
    let app = app().await;
    // log in
    let body = format!("username={USERNAME}&password={PASSWORD}");
    let login = app
        .clone()
        .oneshot(post_form("/login", &body))
        .await
        .unwrap();
    let token = session_cookie(login.headers()).expect("session cookie set");
    // reuse the cookie
    let res = app
        .oneshot(get_with_cookie("/", &format!("session={token}")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn logout_clears_cookie_and_redirects_to_login() {
    let app = app().await;
    let body = format!("username={USERNAME}&password={PASSWORD}");
    let login = app
        .clone()
        .oneshot(post_form("/login", &body))
        .await
        .unwrap();
    let token = session_cookie(login.headers()).expect("session cookie set");
    let res = app
        .oneshot(get_with_cookie("/logout", &format!("session={token}")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get(header::LOCATION).unwrap(), "/login");
    let set = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set.contains("session="));
    assert!(set.contains("Max-Age=0"));
}

#[tokio::test]
async fn concurrent_logins_all_succeed() {
    // The login-verify concurrency cap (bounds argon2 memory under a flood) must
    // not deadlock or starve legitimate logins: fire more than the permit count
    // at once and assert every one still authenticates and gets a cookie.
    let app = app().await;
    let mut handles = Vec::new();
    for _ in 0..12 {
        let app = app.clone();
        handles.push(tokio::spawn(async move {
            let body = format!("username={USERNAME}&password={PASSWORD}");
            let res = app.oneshot(post_form("/login", &body)).await.unwrap();
            (res.status(), session_cookie(res.headers()).map(|c| c.len()))
        }));
    }
    for h in handles {
        let (status, cookie_len) = h.await.unwrap();
        assert_eq!(status, StatusCode::SEE_OTHER);
        assert_eq!(cookie_len, Some(64));
    }
}

#[tokio::test]
async fn hash_password_roundtrips() {
    let hash = auth::hash_password("hunter2");
    assert!(hash.starts_with("$argon2"));
    assert!(auth::verify_password("hunter2", &hash));
    assert!(!auth::verify_password("nope", &hash));
}
