//! e2e — drive the spools routes through the full Axum router against a
//! real, seeded Postgres (ADR-0003): add a spool, see it in the (filtered,
//! htmx-fragment) list, then view its detail page. Mirrors
//! `e2e_materials.rs`'s wiring (`seeded_app()` + `mod support;`), but also
//! reaches for the real repos directly (outside the router) to fetch a
//! seeded material id and the created spool's id — the same pattern
//! `it_spools.rs` uses, just against the shared per-binary container.

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use domain::dashboard::{DashboardRepository, DashboardService, DashboardUseCases};
use domain::locations::{LocationRepository, LocationsService, LocationsUseCases};
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::shared::MaterialId;
use domain::spools::{
    SpoolFilter, SpoolRepository, SpoolSort, SpoolStatus, SpoolsService, SpoolsUseCases,
};
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

/// Builds the full app against a real seeded Postgres, wiring both the
/// materials and spools slices into the router — identical to
/// `e2e_materials.rs::seeded_app()`. Idempotent (materials seeding is a
/// no-op past the first call), so it's safe to call from multiple tests
/// sharing the per-binary testcontainer.
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
    let manufacturer_repo: Arc<dyn domain::manufacturers::ManufacturerRepository> =
        Arc::new(filature::persistence::manufacturers::SqlxManufacturerRepository::new(db.clone()));
    let manufacturers: Arc<dyn domain::manufacturers::ManufacturersUseCases> = Arc::new(
        domain::manufacturers::ManufacturersService::new(manufacturer_repo),
    );
    let dash_repo: Arc<dyn DashboardRepository> =
        Arc::new(SqlxDashboardRepository::new(db.clone()));
    let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(dash_repo));
    web::router(web::AppState::new(
        db,
        &test_config(&url),
        materials,
        spools,
        locations,
        manufacturers,
        dashboard,
        Arc::new(
            domain::instance_configuration::InstanceConfigurationService::new(Arc::new(
                domain::instance_configuration::stubs::StubInstanceConfigurationRepository::new(),
            )),
        ),
        Arc::new(domain::instance_transfer::InstanceTransferService::new(
            Arc::new(domain::instance_transfer::stubs::StubInstanceTransferRepository::default()),
        )),
    ))
}

async fn body_of(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The full flow: add a spool via the real `POST /spools` form, see it in
/// the list (and the htmx `/spools/rows` fragment, filtered by its
/// material), then view its detail page. Runs as a single test (rather than
/// several) so the created spool's identity can be threaded step to step
/// without racing other tests over the shared testcontainer database.
#[tokio::test]
async fn add_then_list_then_view_spool() {
    let app = seeded_app().await;

    // A real seeded material to hang the new spool off — fetched straight
    // from the same Postgres the router uses, via the real repo (mirrors
    // `it_spools.rs`'s direct-repo style) rather than screen-scraping the
    // add-form's dropdown.
    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let material_repo: Arc<dyn MaterialRepository> =
        Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials_uc: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(material_repo));
    let all_materials = materials_uc.list().await.unwrap();
    let material = all_materials
        .first()
        .expect("materials seeded by seeded_app()");
    let material_id = material.id.as_str().to_string();
    let material_name = material.name.as_str().to_string();

    // --- POST /spools: add a spool for that material.
    let form = format!(
        "condition=new&material_id={material_id}&colour_hex=%231A9E4B&colour_name=Test+Green&diameter=1.75&net_weight=1000&price_paid=24.90"
    );
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(
        res.headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("/spools/")
    );

    // --- GET /spools: the new spool shows up in the full list page.
    let res = app
        .clone()
        .oneshot(Request::get("/spools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(&material_name));
    assert!(html.contains("#1A9E4B"));

    // --- GET /spools/rows?material_id=...: the htmx fragment, filtered by
    // this spool's material, contains it — and is the tbody fragment only
    // (no base-layout wrapper).
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/rows?material_id={material_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rows_html = body_of(res).await;
    assert!(rows_html.contains("#1A9E4B"));
    assert!(rows_html.contains(&material_name));
    assert!(!rows_html.contains("<html")); // fragment only, no full page shell
    assert!(!rows_html.contains("<table")); // tbody content only, no wrapper

    // --- Find the created spool's id via the real repo (list filtered by
    // material, matching on the colour we just posted — unique enough for
    // this test's own data even though other tests may add rows to the
    // shared container).
    let spool_repo = SqlxSpoolRepository::new(db.clone());
    let items = spool_repo
        .list(
            SpoolFilter {
                material_id: Some(MaterialId::new(material_id.clone())),
                status: None,
                ..Default::default()
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    let created = items
        .iter()
        .find(|i| i.colour.as_ref().is_some_and(|c| c.hex() == "#1A9E4B"))
        .expect("the spool just posted is present in the filtered list");
    let spool_id = created.id.as_str().to_string();

    // --- GET /spools/{id}: the detail page shows the spool's fields.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let detail_html = body_of(res).await;
    assert!(detail_html.contains(&material_name));
    // Colour name is derived from the hex now (never free-typed): #1A9E4B is
    // not a preset, so the displayed name is the upper-cased hex itself.
    assert!(detail_html.contains("#1A9E4B"));
    assert!(detail_html.contains("1.75"));
    assert!(detail_html.contains("1000")); // net + remaining weight (equal on a fresh spool)
    assert!(detail_html.contains("100%")); // remaining ratio on a fresh spool
    assert!(detail_html.contains("24.90")); // price paid
    assert!(detail_html.contains(&format!("/spools/{spool_id}/edit")));
}

/// The ops journey: add a spool, draw it down via two `consume` posts
/// (Sealed -> Open -> Empty), archive it (drops out of the default list,
/// shows up under the `Archived` filter), then restore it (back in the
/// default list, with its status still derived as `Empty` — restore never
/// resets the remaining weight). Runs as a single test, same rationale as
/// `add_then_list_then_view_spool`: the created spool's identity threads
/// through every step without racing other tests over the shared
/// testcontainer database.
#[tokio::test]
async fn consume_archive_restore_journey() {
    let app = seeded_app().await;

    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let material_repo: Arc<dyn MaterialRepository> =
        Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials_uc: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(material_repo));
    let all_materials = materials_uc.list().await.unwrap();
    let material = all_materials
        .first()
        .expect("materials seeded by seeded_app()");
    let material_id = material.id.as_str().to_string();

    // --- POST /spools: add a fresh spool (net 1000g, price 24.99) with a
    // colour unique to this test, so it can be picked out of the shared
    // container's rows later.
    let form = format!(
        "condition=new&material_id={material_id}&colour_hex=%23FF00A0&colour_name=Journey+Pink&diameter=1.75&net_weight=1000&price_paid=24.99"
    );
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let spool_repo = SqlxSpoolRepository::new(db.clone());
    let items = spool_repo
        .list(
            SpoolFilter {
                material_id: Some(MaterialId::new(material_id.clone())),
                status: None,
                ..Default::default()
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    let created = items
        .iter()
        .find(|i| i.colour.as_ref().is_some_and(|c| c.hex() == "#FF00A0"))
        .expect("the spool just posted is present in the filtered list");
    let spool_id = created.id.as_str().to_string();
    let row_marker = format!("spool-row-{spool_id}");

    // --- 1. GET /spools/{id}: fresh spool starts Sealed.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_of(res).await.contains("Sealed"));

    // --- 2. POST /spools/{id}/consume amount=300 -> Open, remaining 700.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/consume"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("amount=300"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains("Open"));
    assert!(html.contains("700"));

    // --- 3. POST /spools/{id}/consume amount=700 -> exhausted, Empty,
    // remaining 0.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/consume"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("amount=700"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains("Empty"));
    let after_consume = spool_repo.find(&created.id).await.unwrap().unwrap();
    assert_eq!(after_consume.remaining_weight.value(), 0.0);
    assert_eq!(after_consume.status, SpoolStatus::Empty);

    // --- 4. POST /spools/{id}/archive -> default list excludes it, the
    // `Archived` filter includes it.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/archive"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(Request::get("/spools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(!html.contains(&row_marker));

    let res = app
        .clone()
        .oneshot(
            Request::get("/spools?status=Archived")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(&row_marker));

    // --- 5. POST /spools/{id}/restore -> back in the default list, with
    // its derived status still Empty (restore doesn't touch remaining).
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/restore"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(Request::get("/spools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(&row_marker));
    assert!(html.contains("spool-row--empty"));

    // --- 6. The list header carries a stock-value figure.
    assert!(html.contains(r#"id="stock-value""#));
}

/// The Stock Value stat respects the active filter: with two spools of
/// different materials and different prices, `GET /spools/rows` filtered by
/// one material must report only that spool's value — not the whole
/// table's. Closes a review gap (a filter-blind stat would silently sum
/// every material's stock).
#[tokio::test]
async fn stock_value_stat_respects_material_filter() {
    let app = seeded_app().await;

    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let material_repo: Arc<dyn MaterialRepository> =
        Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials_uc: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(material_repo));

    // Two distinct materials with distinct prices, so their stock values
    // can't accidentally coincide and mask a filter bug.
    let mat_a = materials_uc
        .add(domain::materials::NewMaterial {
            name: domain::materials::MaterialName::new("E2E-Filter-A").unwrap(),
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
    let mat_b = materials_uc
        .add(domain::materials::NewMaterial {
            name: domain::materials::MaterialName::new("E2E-Filter-B").unwrap(),
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
    let mat_a_id = mat_a.id.as_str().to_string();
    let mat_b_id = mat_b.id.as_str().to_string();

    // Material A: 1000g @ 20.00, drawn down to 400g remaining (via
    // `consume` over the router) -> (400/1000) * 20.00 = 8.00.
    let form_a = format!(
        "condition=new&material_id={mat_a_id}&colour_hex=%23112233&colour_name=Filter+A&diameter=1.75&net_weight=1000&price_paid=20.00"
    );
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_a))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Material B: 1000g @ 50.00, left sealed -> full value 50.00.
    let form_b = format!(
        "condition=new&material_id={mat_b_id}&colour_hex=%23445566&colour_name=Filter+B&diameter=1.75&net_weight=1000&price_paid=50.00"
    );
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_b))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let spool_repo = SqlxSpoolRepository::new(db.clone());
    let a_items = spool_repo
        .list(
            SpoolFilter {
                material_id: Some(MaterialId::new(mat_a_id.clone())),
                status: None,
                ..Default::default()
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    let spool_a_id = a_items
        .iter()
        .find(|i| i.colour.as_ref().is_some_and(|c| c.hex() == "#112233"))
        .expect("spool A just posted")
        .id
        .as_str()
        .to_string();

    // Draw spool A down to 400g remaining via the real router op, so its
    // contribution to the stock value (8.00) is a genuine fraction, not
    // just the sealed full price.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_a_id}/consume"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("amount=600"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // --- GET /spools/rows?material_id=<A>: filtered stock value must be
    // exactly material A's contribution (8.00), not A+B (58.00).
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/rows?material_id={mat_a_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(r#"id="stock-value" hx-swap-oob="true""#));
    assert!(html.contains("8.00")); // (400/1000) * 20.00, material A only
    assert!(!html.contains("58.00")); // would be A+B if the filter leaked
    assert!(!html.contains("50.00")); // material B's own (unfiltered) value

    // --- Sanity: filtering by material B alone reports its own full value.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/rows?material_id={mat_b_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(html.contains(r#"id="stock-value" hx-swap-oob="true""#));
    assert!(html.contains("50.00"));
    assert!(!html.contains("8.00"));
}

/// The Location-assignment journey across all three web surfaces (Task 9):
/// add a spool with a location selected, confirm the edit form round-trips
/// it (unchanged on resubmission, unassigned on a blank resubmission), then
/// drive the detail-card reassign control (assign, unassign, and the
/// defensive unknown-location/unknown-spool 404s). Runs as a single test —
/// same rationale as the other journeys in this file: one spool's identity
/// threads through every step without racing other tests over the shared
/// testcontainer database.
#[tokio::test]
async fn spool_location_assignment_journey() {
    let app = seeded_app().await;

    let url = support::postgres_url().await;
    let db = persistence::connect_and_migrate(&url).await.unwrap();
    let material_repo: Arc<dyn MaterialRepository> =
        Arc::new(SqlxMaterialRepository::new(db.clone()));
    let materials_uc: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(material_repo));
    let all_materials = materials_uc.list().await.unwrap();
    let material = all_materials
        .first()
        .expect("materials seeded by seeded_app()");
    let material_id = material.id.as_str().to_string();

    let location_repo = SqlxLocationRepository::new(db.clone());
    let location = location_repo
        .insert(domain::locations::NewLocation {
            name: domain::locations::LocationName::new("E2E Shelf").unwrap(),
            note: None,
        })
        .await
        .unwrap();
    let location_id = location.id.as_str().to_string();

    // --- 1. POST /spools with a location selected -> persists, and the
    // detail page shows the assigned location's name.
    let form = format!(
        "condition=new&material_id={material_id}&colour_hex=%23ABCDEF&colour_name=Loc+Journey&diameter=1.75&net_weight=1000&price_paid=15.00&location_id={location_id}"
    );
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let spool_repo = SqlxSpoolRepository::new(db.clone());
    let items = spool_repo
        .list(
            SpoolFilter {
                material_id: Some(MaterialId::new(material_id.clone())),
                status: None,
                ..Default::default()
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    let created = items
        .iter()
        .find(|i| i.colour.as_ref().is_some_and(|c| c.hex() == "#ABCDEF"))
        .expect("the spool just posted is present in the filtered list");
    let spool_id = created.id.as_str().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_of(res).await.contains("E2E Shelf"));

    // --- 2. GET /spools/{id}/edit -> the edit form preselects the current
    // location.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}/edit"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let edit_html = body_of(res).await;
    assert!(edit_html.contains(&format!(r#"value="{location_id}" selected"#)));

    // --- 3. PUT /spools/{id} resubmitting the SAME location -> the location
    // is NOT wiped by an edit that doesn't touch it.
    let form = format!(
        "condition=new&material_id={material_id}&colour_hex=%23ABCDEF&colour_name=Loc+Journey&diameter=1.75&net_weight=1000&price_paid=15.00&location_id={location_id}"
    );
    let res = app
        .clone()
        .oneshot(
            Request::put(format!("/spools/{spool_id}"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_of(res).await.contains("E2E Shelf"));

    // --- 4. PUT /spools/{id} with a BLANK location -> unassigns.
    let form = format!(
        "condition=new&material_id={material_id}&colour_hex=%23ABCDEF&colour_name=Loc+Journey&diameter=1.75&net_weight=1000&price_paid=15.00&location_id="
    );
    let res = app
        .clone()
        .oneshot(
            Request::put(format!("/spools/{spool_id}"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/spools/{spool_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_of(res).await;
    // The location still exists (still listed as a reassign option), but
    // must no longer be the *selected* one, and the display falls back to
    // the "unassigned" label rather than the location's name.
    assert!(html.contains("E2E Shelf")); // still a selectable option
    assert!(!html.contains(&format!(r#"value="{location_id}" selected"#)));
    assert!(html.contains("Unassigned"));

    // --- 5. POST /spools/{id}/location (detail-card reassign) with the real
    // location id -> reassigns and returns the swapped card fragment.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/location"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("location_id={location_id}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(!html.contains("<html")); // fragment only, no page shell
    assert!(html.contains(&format!(r#"value="{location_id}" selected"#)));

    // --- 6. POST /spools/{id}/location with a BLANK value -> unassigns via
    // the reassign control too.
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/location"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("location_id="))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_of(res).await;
    assert!(!html.contains(&format!(r#"value="{location_id}" selected"#)));
    assert!(html.contains("Unassigned"));

    // --- 7. POST /spools/{id}/location with an UNKNOWN location id -> 404,
    // never a 500 and never misreported as an unknown material (ids come
    // from a rendered <select>, so this is defensive).
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/spools/{spool_id}/location"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("location_id=does-not-exist"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // --- 8. POST /spools/{id}/location on an unknown SPOOL id -> 404.
    let res = app
        .clone()
        .oneshot(
            Request::post("/spools/does-not-exist/location")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("location_id="))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
