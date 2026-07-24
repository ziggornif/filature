mod support;

use domain::locations::{LocationName, LocationRepository, NewLocation, RepositoryError};
use domain::materials::{
    Density, DryingParams, MaterialName, MaterialRepository, NewMaterial, Sensitivity, Temperature,
};
use domain::shared::{Grams, LocationId, MaterialId, Money};
use domain::spools::{Colour, Diameter, NewSpool, SpoolRepository};
use filature::persistence::connect_and_migrate;
use filature::persistence::locations::SqlxLocationRepository;
use filature::persistence::materials::SqlxMaterialRepository;
use filature::persistence::spools::SqlxSpoolRepository;
use rust_decimal::Decimal;

fn sample(name: &str, note: Option<&str>) -> NewLocation {
    NewLocation {
        name: LocationName::new(name).unwrap(),
        note: note.map(|n| n.to_string()),
    }
}

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

fn sample_spool(material_id: MaterialId) -> NewSpool {
    NewSpool {
        condition: domain::spools::SpoolCondition::New,
        material_id,
        colour: Some(Colour::new("#1A9E4B".into(), Some("vert sapin".into())).unwrap()),
        diameter: Diameter::Mm1_75,
        net_weight: Grams::new(1000.0).unwrap(),
        price_paid: Money::from_decimal(Decimal::from_str_exact("10.00").unwrap()).unwrap(),
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
async fn insert_assigns_ulid_and_roundtrips() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let created = repo
        .insert(sample("Warehouse A", Some("Near the door")))
        .await
        .unwrap();
    assert_eq!(created.id.as_str().len(), 26); // ULID is 26 chars
    assert_eq!(created.name.as_str(), "Warehouse A");
    assert_eq!(created.note, Some("Near the door".to_string()));

    let all = repo.list().await.unwrap();
    let found = all
        .iter()
        .find(|l| l.name.as_str() == "Warehouse A")
        .unwrap();
    assert_eq!(found.name.as_str(), "Warehouse A");
    assert_eq!(found.note, Some("Near the door".to_string()));
}

#[tokio::test]
async fn insert_without_note_roundtrips_as_none() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let created = repo.insert(sample("Shelf B", None)).await.unwrap();
    assert_eq!(created.note, None);

    let all = repo.list().await.unwrap();
    let found = all.iter().find(|l| l.name.as_str() == "Shelf B").unwrap();
    assert_eq!(found.note, None);
}

#[tokio::test]
async fn update_persists_name_and_note() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let mut loc = repo
        .insert(sample("Old Name", Some("Old note")))
        .await
        .unwrap();
    loc.name = LocationName::new("New Name").unwrap();
    loc.note = Some("New note".to_string());

    let updated = repo.update(loc.clone()).await.unwrap();
    assert_eq!(updated.name.as_str(), "New Name");
    assert_eq!(updated.note, Some("New note".to_string()));

    let all = repo.list().await.unwrap();
    let found = all.iter().find(|l| l.id == loc.id).unwrap();
    assert_eq!(found.name.as_str(), "New Name");
    assert_eq!(found.note, Some("New note".to_string()));
}

#[tokio::test]
async fn update_unknown_id_returns_not_found() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let fake = domain::locations::Location {
        id: LocationId::new("01FAKELOCATIONIDDOESNOTEXIS"),
        name: LocationName::new("Ghost").unwrap(),
        note: None,
    };

    let err = repo.update(fake.clone()).await.unwrap_err();
    match err {
        RepositoryError::NotFound(id) => assert_eq!(id, fake.id),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn delete_removes_row() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let created = repo.insert(sample("To Delete", None)).await.unwrap();
    repo.delete(&created.id).await.unwrap();

    let all = repo.list().await.unwrap();
    assert!(!all.iter().any(|l| l.id == created.id));
}

#[tokio::test]
async fn delete_unknown_id_returns_not_found() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxLocationRepository::new(pool);

    let missing = LocationId::new("01MISSINGLOCATIONIDXXXXXXXX");
    let err = repo.delete(&missing).await.unwrap_err();
    match err {
        RepositoryError::NotFound(id) => assert_eq!(id, missing),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn count_spools_zero_then_n_after_assigning_spools() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let locations = SqlxLocationRepository::new(pool.clone());
    let materials = SqlxMaterialRepository::new(pool.clone());
    let spools = SqlxSpoolRepository::new(pool.clone());

    let location = locations
        .insert(sample("Count Location", None))
        .await
        .unwrap();
    assert_eq!(locations.count_spools(&location.id).await.unwrap(), 0);

    let material = locations_test_material(&materials).await;

    let s1 = spools
        .insert(sample_spool(material.id.clone()))
        .await
        .unwrap();
    let s2 = spools
        .insert(sample_spool(material.id.clone()))
        .await
        .unwrap();

    // NewSpool has no location_id field yet — assign it directly via SQL,
    // mirroring how the brief allows setting up the FK for this test.
    sqlx::query!(
        "UPDATE spools SET location_id = $1 WHERE id = $2",
        location.id.as_str(),
        s1.id.as_str()
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(locations.count_spools(&location.id).await.unwrap(), 1);

    sqlx::query!(
        "UPDATE spools SET location_id = $1 WHERE id = $2",
        location.id.as_str(),
        s2.id.as_str()
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(locations.count_spools(&location.id).await.unwrap(), 2);
}

async fn locations_test_material(
    materials: &SqlxMaterialRepository,
) -> domain::materials::Material {
    materials
        .insert(sample_material("Loc-Test-Mat"))
        .await
        .unwrap()
}
