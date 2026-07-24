mod support;

use domain::dashboard::{DashboardOverview, DashboardRepository, StockStatus};
use domain::locations::{LocationName, LocationRepository, NewLocation};
use domain::manufacturers::{ManufacturerName, ManufacturerRepository, NewManufacturer};
use domain::materials::{
    Density, DryingParams, MaterialName, MaterialRepository, NewMaterial, Sensitivity, Temperature,
};
use domain::shared::{Grams, MaterialId, Money};
use domain::spools::{Colour, Diameter, NewSpool, SpoolFilter, SpoolRepository, SpoolStatus};
use filature::persistence::connect_and_migrate;
use filature::persistence::dashboard::SqlxDashboardRepository;
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::manufacturers::SqlxManufacturerRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use rust_decimal::Decimal;

fn sample_material(name: &str) -> NewMaterial {
    NewMaterial {
        name: MaterialName::new(name).unwrap(),
        density: Density::new(1.24).unwrap(),
        drying: DryingParams {
            temp: Temperature::new(45),
            time_h: 6,
        },
        sensitivity: Sensitivity::Low,
        nozzle: Temperature::new(210),
        bed: Temperature::new(60),
    }
}

fn sample_location(name: &str) -> NewLocation {
    NewLocation {
        name: LocationName::new(name).unwrap(),
        note: None,
    }
}

fn sample_spool(material_id: MaterialId, net: f64, price: &str) -> NewSpool {
    NewSpool {
        condition: domain::spools::SpoolCondition::New,
        material_id,
        colour: Some(Colour::new("#1A9E4B".into(), Some("vert sapin".into())).unwrap()),
        diameter: Diameter::Mm1_75,
        net_weight: Grams::new(net).unwrap(),
        price_paid: Money::from_decimal(Decimal::from_str_exact(price).unwrap()).unwrap(),
        location_id: None,
        manufacturer_id: None,
        notes: None,
        purchased_at: None,
        opened_at: None,
        ams_tag_uid: None,
        remaining_weight: None,
    }
}

#[tokio::test]
async fn stock_rows_excludes_archived_and_maps_fields_correctly() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let locations = SqlxLocationRepository::new(pool.clone());
    let manufacturers = SqlxManufacturerRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool.clone());
    let dashboard = SqlxDashboardRepository::new(pool.clone());

    let mat_a = materials.insert(sample_material("Dash-PLA")).await.unwrap();
    let mat_b = materials
        .insert(sample_material("Dash-PETG"))
        .await
        .unwrap();
    let location = locations
        .insert(sample_location("Dash-Shelf"))
        .await
        .unwrap();
    let manufacturer = manufacturers
        .insert(NewManufacturer {
            name: ManufacturerName::new("Dash-Prusament").unwrap(),
            country: Some("CZ".to_string()),
        })
        .await
        .unwrap();

    // s1: Sealed, full, no location.
    let s1 = spools
        .insert(sample_spool(mat_a.id.clone(), 1000.0, "20.00"))
        .await
        .unwrap();

    // s2: Open, low-stock (ratio 0.10 < 0.15), with a location.
    let mut s2 = spools
        .insert(sample_spool(mat_a.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();
    s2.status = SpoolStatus::Open;
    s2.remaining_weight = Grams::new(100.0).unwrap();
    s2.location_id = Some(location.id.clone());
    s2.manufacturer_id = Some(manufacturer.id.clone());
    spools.update(s2.clone()).await.unwrap();

    // s3: Empty at 0g — not low-stock (already finished), no location.
    let mut s3 = spools
        .insert(sample_spool(mat_b.id.clone(), 1000.0, "15.00"))
        .await
        .unwrap();
    s3.status = SpoolStatus::Empty;
    s3.remaining_weight = Grams::new(0.0).unwrap();
    spools.update(s3.clone()).await.unwrap();

    // s4: Archived — must be excluded entirely from stock_rows.
    let mut s4 = spools
        .insert(sample_spool(mat_b.id.clone(), 1000.0, "999.00"))
        .await
        .unwrap();
    s4.status = SpoolStatus::Archived;
    spools.update(s4.clone()).await.unwrap();

    let all_rows = dashboard.stock_rows().await.unwrap();
    // Scope to this test's own spools — the testcontainer DB is shared across
    // tests in this binary, which may run concurrently.
    let our_ids = [
        s1.id.as_str(),
        s2.id.as_str(),
        s3.id.as_str(),
        s4.id.as_str(),
    ];
    let rows: Vec<_> = all_rows
        .into_iter()
        .filter(|r| our_ids.contains(&r.spool_id.as_str()))
        .collect();

    assert_eq!(rows.len(), 3, "archived spool must be excluded");
    assert!(
        !rows.iter().any(|r| r.spool_id == s4.id.as_str()),
        "archived spool id must not appear"
    );

    let row1 = rows.iter().find(|r| r.spool_id == s1.id.as_str()).unwrap();
    assert_eq!(row1.material_id, mat_a.id);
    assert_eq!(row1.material_name, "Dash-PLA");
    assert_eq!(row1.manufacturer_name, None);
    assert_eq!(row1.colour_hex, "#1A9E4B");
    assert_eq!(row1.colour_name, Some("green".to_string()));
    assert_eq!(row1.status, StockStatus::Sealed);
    assert_eq!(row1.remaining_weight.value(), 1000.0);
    assert_eq!(row1.net_weight.value(), 1000.0);
    assert_eq!(
        row1.price_paid,
        Money::from_decimal(Decimal::from_str_exact("20.00").unwrap()).unwrap()
    );
    assert_eq!(row1.location_name, None);

    let row2 = rows.iter().find(|r| r.spool_id == s2.id.as_str()).unwrap();
    assert_eq!(row2.material_id, mat_a.id);
    assert_eq!(row2.status, StockStatus::Open);
    assert_eq!(row2.remaining_weight.value(), 100.0);
    assert_eq!(row2.manufacturer_name, Some("Dash-Prusament".to_string()));
    assert_eq!(row2.location_name, Some("Dash-Shelf".to_string()));

    let row3 = rows.iter().find(|r| r.spool_id == s3.id.as_str()).unwrap();
    assert_eq!(row3.material_id, mat_b.id);
    assert_eq!(row3.material_name, "Dash-PETG");
    assert_eq!(row3.status, StockStatus::Empty);
    assert_eq!(row3.remaining_weight.value(), 0.0);
    assert_eq!(row3.location_name, None);
}

#[tokio::test]
async fn overview_from_rows_matches_stock_value_for_all_stock_scope() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool.clone());
    let dashboard = SqlxDashboardRepository::new(pool.clone());

    let mat = materials
        .insert(sample_material("Dash-Compare"))
        .await
        .unwrap();

    // Full spool.
    let s1 = spools
        .insert(sample_spool(mat.id.clone(), 1000.0, "24.99"))
        .await
        .unwrap();

    // Half-consumed spool.
    let mut s2 = spools
        .insert(sample_spool(mat.id.clone(), 1000.0, "12.00"))
        .await
        .unwrap();
    s2.status = SpoolStatus::Open;
    s2.remaining_weight = Grams::new(500.0).unwrap();
    spools.update(s2.clone()).await.unwrap();

    // Archived spool — must not contribute to either figure.
    let mut s3 = spools
        .insert(sample_spool(mat.id.clone(), 1000.0, "500.00"))
        .await
        .unwrap();
    s3.status = SpoolStatus::Archived;
    spools.update(s3.clone()).await.unwrap();

    let expected = spools
        .stock_value(SpoolFilter {
            material_id: Some(mat.id.clone()),
            status: None,
            ..Default::default()
        })
        .await
        .unwrap();

    let all_rows = dashboard.stock_rows().await.unwrap();
    let rows: Vec<_> = all_rows
        .into_iter()
        .filter(|r| r.material_id == mat.id)
        .collect();
    assert_eq!(
        rows.len(),
        2,
        "archived row must be excluded from the scope"
    );
    assert!(!rows.iter().any(|r| r.spool_id == s3.id.as_str()));

    let overview = DashboardOverview::from_rows(rows, domain::shared::LowStockThreshold::default());
    assert_eq!(overview.stock_value, expected);
    assert_eq!(overview.total_count, 2);
    assert_eq!(overview.active_count, 2);
    assert_eq!(overview.empty_count, 0);

    let _ = s1;
}
