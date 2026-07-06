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
use domain::materials::{MaterialRepository, MaterialsService, MaterialsUseCases};
use domain::shared::{Grams, MaterialId};
use domain::spools::{
    Diameter, SpoolFilter, SpoolRepository, SpoolSort, SpoolsService, SpoolsUseCases,
    remaining_length_m,
};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
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
    web::router(web::AppState::new(
        db,
        &test_config(&url),
        materials,
        spools,
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
        "material_id={material_id}&colour_hex=%231A9E4B&colour_name=Test+Green&diameter=1.75&net_weight=1000&price_paid=24.90"
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
    assert_eq!(
        res.headers().get("location").unwrap().to_str().unwrap(),
        "/spools"
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
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    let created = items
        .iter()
        .find(|i| i.colour.hex() == "#1A9E4B")
        .expect("the spool just posted is present in the filtered list");
    let spool_id = created.id.as_str().to_string();

    // The remaining length is computed off a fresh spool (remaining == net
    // == 1000g), using this material's density — same formula/rounding the
    // web layer applies, so it's a genuine (not just non-zero) expectation.
    let density = material.density.value();
    let expected_len = remaining_length_m(Grams::new(1000.0).unwrap(), density, Diameter::Mm1_75);
    let expected_len_rounded = (expected_len * 10.0).round() / 10.0;
    assert!(expected_len_rounded > 0.0);

    // --- GET /spools/{id}: the detail page shows the spool's fields and a
    // plausible (non-zero, formula-consistent) Remaining Length.
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
    assert!(detail_html.contains("#1A9E4B"));
    assert!(detail_html.contains("Test Green"));
    assert!(detail_html.contains("1.75"));
    assert!(detail_html.contains("1000")); // net + remaining weight (equal on a fresh spool)
    assert!(detail_html.contains("100%")); // remaining ratio on a fresh spool
    assert!(detail_html.contains("24.90")); // price paid
    assert!(detail_html.contains(&format!("{expected_len_rounded}"))); // remaining length, non-zero
    assert!(detail_html.contains(&format!("/spools/{spool_id}/edit")));
}
