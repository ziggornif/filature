//! e2e — drive the locations routes through the full Axum router against a
//! real, seeded Postgres (ADR-0003). Mirrors `e2e_materials.rs`'s wiring
//! (`seeded_app()` + `mod support;`), plus reaches for the real spool repo
//! directly (outside the router) to assign/unassign a spool to a location
//! via raw SQL — the same pattern `it_locations.rs`'s
//! `count_spools_zero_then_n_after_assigning_spools` test uses, since
//! `NewSpool` has no `location_id` field yet.
//!
//! Exercises the status-code mappings that are the core of the locations
//! web adapter: 422 (blank name), 404 (unknown id), 409 (delete blocked by
//! an assigned spool, with the count interpolated into the message), and
//! 200 (delete succeeds once unassigned).

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use domain::dashboard::{DashboardRepository, DashboardService, DashboardUseCases};
use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::shared::{Grams, MaterialId, Money};
use domain::spools::{Colour, Diameter, NewSpool, SpoolRepository};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
use filature::persistence::dashboard::SqlxDashboardRepository;
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use rust_decimal::Decimal;
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

/// Builds the full app against a real seeded Postgres — identical wiring to
/// `e2e_materials.rs`/`e2e_spools.rs::seeded_app()`.
async fn seeded_app() -> axum::Router {
    let url = support::postgres_url().await;
    let db = filature::persistence::connect_and_migrate(&url)
        .await
        .unwrap();
    let repo: Arc<dyn MaterialRepository> = Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(repo));
    materials.seed_defaults().await.unwrap(); // idempotent — safe under the shared container
    let spool_repo: Arc<dyn SpoolRepository> = Arc::new(SqlxSpoolRepository::new(db.clone()));
    let spools: Arc<dyn domain::spools::SpoolsUseCases> =
        Arc::new(domain::spools::SpoolsService::new(spool_repo));
    let location_repo: Arc<dyn LocationRepository> =
        Arc::new(SqlxLocationRepository::new(db.clone()));
    let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(location_repo));
    let manufacturer_repo: Arc<dyn domain::manufacturers::ManufacturerRepository> =
        Arc::new(filature::persistence::manufacturers::SqlxManufacturerRepository::new(db.clone()));
    let manufacturers: Arc<dyn domain::manufacturers::ManufacturersUseCases> = Arc::new(
        domain::manufacturers::ManufacturersService::new(manufacturer_repo),
    );
    let dash_repo: Arc<dyn DashboardRepository> =
        Arc::new(SqlxDashboardRepository::new(db.clone()));
    let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(dash_repo));
    filature::web::router(filature::web::AppState::new(
        db,
        &test_config(&url),
        materials,
        spools,
        locations,
        manufacturers,
        dashboard,
    ))
}

async fn body_of(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn post_blank_name_is_422_with_surfaced_message() {
    let app = seeded_app().await;
    let res = app
        .oneshot(
            Request::post("/locations")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=&note=irrelevant"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // TD-009: the error is routed to the message slot (HX-Reswap normalizes
    // the swap) and carries a human-readable body, not a silent failure.
    assert_eq!(
        res.headers().get("HX-Reswap").and_then(|v| v.to_str().ok()),
        Some("innerHTML")
    );
    let body = body_of(res).await;
    assert!(body.contains("Invalid location"), "body was: {body}");
}

#[tokio::test]
async fn put_unknown_id_is_404() {
    let app = seeded_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/locations/01UNKNOWNLOCATIONIDXXXXXXX")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=Ghost&note="))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// The full delete-guard journey: create a location, assign a spool to it
/// via raw SQL (mirroring `it_locations.rs`), attempt delete -> 409 with the
/// count interpolated into the `#locations-msg` fragment and the table
/// untouched; unassign the spool, delete again -> 200 and the row gone from
/// the list. Runs as a single test so the created location/spool ids thread
/// through without racing other tests over the shared testcontainer.
#[tokio::test]
async fn delete_blocked_then_allowed_after_unassign() {
    let app = seeded_app().await;

    let url = support::postgres_url().await;
    let db = filature::persistence::connect_and_migrate(&url)
        .await
        .unwrap();

    // --- POST /locations: create a fresh location via the real router.
    let res = app
        .clone()
        .oneshot(
            Request::post("/locations")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=E2E+Delete+Guard+Shelf&note=near+the+press",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    // Pull the location id out of the row id it just rendered
    // (`id="location-row-<ULID>"`) rather than re-listing, so this test
    // doesn't depend on ordering among other locations in the shared DB.
    let marker = "location-row-";
    let start = html.find(marker).expect("row id present") + marker.len();
    let rest = &html[start..];
    let end = rest.find('"').expect("closing quote");
    let location_id = rest[..end].to_string();

    // --- Seed a material + spool, then assign the spool to this location
    // via raw SQL (NewSpool has no location_id field yet — same technique
    // `it_locations.rs::count_spools_zero_then_n_after_assigning_spools`
    // uses).
    let material_repo = SqlxMaterialRepository::new(db.clone());
    let material = material_repo
        .insert(domain::materials::NewMaterial {
            name: domain::materials::MaterialName::new("E2E-Loc-Guard-Mat").unwrap(),
            density: domain::materials::Density::new(1.24).unwrap(),
            drying: domain::materials::DryingParams {
                temp: domain::materials::Temperature::new(45),
                time_h: 6,
            },
            sensitivity: domain::materials::Sensitivity::Low,
            nozzle: domain::materials::Temperature::new(210),
            bed: domain::materials::Temperature::new(60),
        })
        .await
        .unwrap();

    let spool_repo = SqlxSpoolRepository::new(db.clone());
    let spool = spool_repo
        .insert(NewSpool {
            material_id: MaterialId::new(material.id.as_str().to_string()),
            colour: Colour::new("#0F0F0F".into(), Some("guard black".into())).unwrap(),
            diameter: Diameter::Mm1_75,
            net_weight: Grams::new(1000.0).unwrap(),
            price_paid: Money::from_decimal(Decimal::from_str_exact("10.00").unwrap()).unwrap(),
            location_id: None,
            manufacturer_id: None,
        })
        .await
        .unwrap();

    sqlx::query!(
        "UPDATE spools SET location_id = $1 WHERE id = $2",
        location_id,
        spool.id.as_str()
    )
    .execute(&db)
    .await
    .unwrap();

    // --- DELETE /locations/{id}: blocked (409), the count is interpolated
    // into the `#locations-msg` fragment, and the table itself is not part
    // of the response (no `<tr>`/`<table` in the 409 body).
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/locations/{location_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let html = body_of(res).await;
    assert!(html.contains(r#"id="locations-msg""#));
    assert!(html.contains("1 spool(s) assigned"));
    assert!(!html.contains("<tr"));

    // --- GET /locations: the location is still listed (delete refused).
    let res = app
        .clone()
        .oneshot(Request::get("/locations").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(&format!("location-row-{location_id}")));

    // --- Unassign the spool, then delete again -> 200, empty body (the
    // htmx row swap removes the `<tr>` client-side), and the row is gone
    // from a subsequent GET /locations.
    sqlx::query!(
        "UPDATE spools SET location_id = NULL WHERE id = $1",
        spool.id.as_str()
    )
    .execute(&db)
    .await
    .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/locations/{location_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.is_empty());

    let res = app
        .clone()
        .oneshot(Request::get("/locations").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(!html.contains(&format!("location-row-{location_id}")));
}
