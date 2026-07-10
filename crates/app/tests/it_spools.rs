mod support;

use domain::materials::{
    Density, DryingParams, MaterialName, MaterialRepository, NewMaterial, Sensitivity, Temperature,
};
use domain::shared::{Grams, MaterialId, Money};
use domain::spools::{
    Colour, Diameter, NewSpool, RepositoryError, SpoolFilter, SpoolRepository, SpoolSort,
    SpoolStatus,
};
use filature::persistence::connect_and_migrate;
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

fn sample_spool(material_id: MaterialId, net: f64, price: &str) -> NewSpool {
    NewSpool {
        material_id,
        colour: Colour::new("#1A9E4B".into(), Some("vert sapin".into())).unwrap(),
        diameter: Diameter::Mm1_75,
        net_weight: Grams::new(net).unwrap(),
        price_paid: Money::from_decimal(Decimal::from_str_exact(price).unwrap()).unwrap(),
        location_id: None,
    }
}

#[tokio::test]
async fn insert_get_full_roundtrip() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials.insert(sample_material("PLA-A")).await.unwrap();

    let created = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "24.99"))
        .await
        .unwrap();

    assert_eq!(created.id.as_str().len(), 26); // ULID
    assert_eq!(created.material_id, material.id);
    assert_eq!(created.net_weight.value(), 1000.0);
    assert_eq!(created.remaining_weight.value(), 1000.0); // remaining == net on insert
    assert_eq!(created.status, SpoolStatus::Sealed);
    assert_eq!(
        created.price_paid,
        Money::from_decimal(Decimal::from_str_exact("24.99").unwrap()).unwrap()
    );
    assert_eq!(created.colour.hex(), "#1A9E4B");
    assert_eq!(created.colour.name(), Some("vert sapin"));
    assert_eq!(created.diameter, Diameter::Mm1_75);

    let detail = spools.get(&created.id).await.unwrap().unwrap();
    assert_eq!(detail.id, created.id);
    assert_eq!(detail.material_id, material.id);
    assert_eq!(detail.material_name, "PLA-A");
    assert_eq!(detail.density, 1.24);
    assert_eq!(detail.colour.hex(), "#1A9E4B");
    assert_eq!(detail.colour.name(), Some("vert sapin"));
    assert_eq!(detail.diameter, Diameter::Mm1_75);
    assert_eq!(detail.net_weight.value(), 1000.0);
    assert_eq!(detail.remaining_weight.value(), 1000.0);
    assert_eq!(
        detail.price_paid,
        Money::from_decimal(Decimal::from_str_exact("24.99").unwrap()).unwrap()
    );
    assert_eq!(detail.status, SpoolStatus::Sealed);
}

#[tokio::test]
async fn insert_unknown_material_maps_to_unknown_material() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let spools = SqlxSpoolRepository::new(pool);

    let bogus = MaterialId::new("01BOGUSMATERIALIDXXXXXXXXX");
    let err = spools
        .insert(sample_spool(bogus.clone(), 500.0, "10.00"))
        .await
        .unwrap_err();

    match err {
        RepositoryError::UnknownMaterial(id) => assert_eq!(id, bogus),
        other => panic!("expected UnknownMaterial, got {other:?}"),
    }
}

#[tokio::test]
async fn get_unknown_id_returns_none() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let spools = SqlxSpoolRepository::new(pool);

    let missing = domain::spools::SpoolId::new("nope");
    assert!(spools.get(&missing).await.unwrap().is_none());
}

#[tokio::test]
async fn update_persists_changes() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials.insert(sample_material("PLA-B")).await.unwrap();
    let mut created = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "20.00"))
        .await
        .unwrap();

    created.colour = Colour::new("#FF0000".into(), None).unwrap();
    created.status = SpoolStatus::Open;
    created.remaining_weight = Grams::new(400.0).unwrap();
    created.price_paid = Money::from_decimal(Decimal::from_str_exact("21.50").unwrap()).unwrap();

    let updated = spools.update(created.clone()).await.unwrap();
    assert_eq!(updated.colour.hex(), "#FF0000");
    assert_eq!(updated.colour.name(), None);
    assert_eq!(updated.status, SpoolStatus::Open);
    assert_eq!(updated.remaining_weight.value(), 400.0);
    assert_eq!(
        updated.price_paid,
        Money::from_decimal(Decimal::from_str_exact("21.50").unwrap()).unwrap()
    );

    let detail = spools.get(&created.id).await.unwrap().unwrap();
    assert_eq!(detail.colour.hex(), "#FF0000");
    assert_eq!(detail.status, SpoolStatus::Open);
    assert_eq!(detail.remaining_weight.value(), 400.0);
    assert_eq!(
        detail.price_paid,
        Money::from_decimal(Decimal::from_str_exact("21.50").unwrap()).unwrap()
    );
}

#[tokio::test]
async fn list_filters_by_material_and_status() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let mat_a = materials.insert(sample_material("Filter-A")).await.unwrap();
    let mat_b = materials.insert(sample_material("Filter-B")).await.unwrap();

    let s1 = spools
        .insert(sample_spool(mat_a.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();
    let _s2 = spools
        .insert(sample_spool(mat_b.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();

    let mut s1_open = s1.clone();
    s1_open.status = SpoolStatus::Open;
    spools.update(s1_open).await.unwrap();

    let by_material = spools
        .list(
            SpoolFilter {
                material_id: Some(mat_a.id.clone()),
                status: None,
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    assert_eq!(by_material.len(), 1);
    assert_eq!(by_material[0].material_name, "Filter-A");

    // Status filter is asserted via containment rather than exact length:
    // other tests in this binary run concurrently against the same
    // testcontainer DB and may also have `Open` spools in flight.
    let by_status = spools
        .list(
            SpoolFilter {
                material_id: None,
                status: Some(SpoolStatus::Open),
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    assert!(by_status.iter().any(|i| i.id == s1.id));
    assert!(!by_status.iter().any(|i| i.id == _s2.id));
}

#[tokio::test]
async fn list_sorts_by_created_desc_and_remaining_ratio_asc() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let mat = materials.insert(sample_material("Sort-Mat")).await.unwrap();

    let first = spools
        .insert(sample_spool(mat.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();
    let second = spools
        .insert(sample_spool(mat.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();

    // Lower remaining ratio for `first` so ascending sort puts it first.
    let mut first_low = first.clone();
    first_low.remaining_weight = Grams::new(100.0).unwrap();
    spools.update(first_low).await.unwrap();

    let created_desc = spools
        .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
        .await
        .unwrap();
    let ids: Vec<_> = created_desc
        .iter()
        .map(|i| i.id.as_str().to_string())
        .filter(|id| *id == first.id.as_str() || *id == second.id.as_str())
        .collect();
    assert_eq!(ids, vec![second.id.as_str(), first.id.as_str()]);

    let ratio_asc = spools
        .list(SpoolFilter::default(), SpoolSort::RemainingRatioAsc)
        .await
        .unwrap();
    let ids: Vec<_> = ratio_asc
        .iter()
        .map(|i| i.id.as_str().to_string())
        .filter(|id| *id == first.id.as_str() || *id == second.id.as_str())
        .collect();
    assert_eq!(ids, vec![first.id.as_str(), second.id.as_str()]);
}

#[tokio::test]
async fn find_returns_aggregate_for_known_id_and_none_for_unknown() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials.insert(sample_material("PLA-Find")).await.unwrap();
    let created = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "15.00"))
        .await
        .unwrap();

    let found = spools.find(&created.id).await.unwrap().unwrap();
    assert_eq!(found, created);

    let missing = domain::spools::SpoolId::new("nope-find");
    assert!(spools.find(&missing).await.unwrap().is_none());
}

#[tokio::test]
async fn update_unknown_id_returns_not_found() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials
        .insert(sample_material("PLA-NotFound"))
        .await
        .unwrap();

    let fake = domain::spools::Spool {
        id: domain::spools::SpoolId::new("01FAKEIDDOESNOTEXISTXXXXXX"),
        material_id: material.id.clone(),
        colour: Colour::new("#000000".into(), None).unwrap(),
        diameter: Diameter::Mm1_75,
        net_weight: Grams::new(500.0).unwrap(),
        remaining_weight: Grams::new(500.0).unwrap(),
        price_paid: Money::from_decimal(Decimal::from_str_exact("5.00").unwrap()).unwrap(),
        status: SpoolStatus::Sealed,
        location_id: None,
    };

    let err = spools.update(fake.clone()).await.unwrap_err();
    match err {
        RepositoryError::NotFound(id) => assert_eq!(id, fake.id),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn update_then_find_reflects_opened_consumed_spool() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials
        .insert(sample_material("PLA-Consume"))
        .await
        .unwrap();
    let mut created = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "12.00"))
        .await
        .unwrap();

    created.status = SpoolStatus::Open;
    created.remaining_weight = Grams::new(650.0).unwrap();
    spools.update(created.clone()).await.unwrap();

    let found = spools.find(&created.id).await.unwrap().unwrap();
    assert_eq!(found.status, SpoolStatus::Open);
    assert_eq!(found.remaining_weight.value(), 650.0);
}

#[tokio::test]
async fn stock_value_sums_non_archived_and_excludes_archived() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials
        .insert(sample_material("PLA-StockValue"))
        .await
        .unwrap();

    // 1000g @ 24.99, drawn down to 500g remaining -> half the value.
    let mut s1 = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "24.99"))
        .await
        .unwrap();
    s1.status = SpoolStatus::Open;
    s1.remaining_weight = Grams::new(500.0).unwrap();
    spools.update(s1.clone()).await.unwrap();

    // 1000g @ 10.00, still full -> full value.
    let _s2 = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "10.00"))
        .await
        .unwrap();

    // Archived spool must contribute 0 regardless of remaining/net.
    let mut s3 = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "999.00"))
        .await
        .unwrap();
    s3.status = SpoolStatus::Archived;
    spools.update(s3.clone()).await.unwrap();

    let value = spools
        .stock_value(SpoolFilter {
            material_id: Some(material.id.clone()),
            status: None,
        })
        .await
        .unwrap();

    // (500/1000)*24.99 + (1000/1000)*10.00 = 12.495 + 10.00 = 22.495
    let expected = Money::from_decimal(Decimal::from_str_exact("22.495").unwrap()).unwrap();
    assert_eq!(value, expected);
}

#[tokio::test]
async fn list_default_excludes_archived_and_status_filter_includes_it() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool);

    let material = materials
        .insert(sample_material("PLA-Archived"))
        .await
        .unwrap();

    let mut archived = spools
        .insert(sample_spool(material.id.clone(), 1000.0, "8.00"))
        .await
        .unwrap();
    archived.status = SpoolStatus::Archived;
    spools.update(archived.clone()).await.unwrap();

    let default_list = spools
        .list(
            SpoolFilter {
                material_id: Some(material.id.clone()),
                status: None,
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    assert!(!default_list.iter().any(|i| i.id == archived.id));

    let archived_list = spools
        .list(
            SpoolFilter {
                material_id: Some(material.id.clone()),
                status: Some(SpoolStatus::Archived),
            },
            SpoolSort::CreatedDesc,
        )
        .await
        .unwrap();
    assert!(archived_list.iter().any(|i| i.id == archived.id));
}
