mod support;

use domain::materials::{
    Density, DryingParams, MaterialRepository, NewMaterial, RepositoryError, Sensitivity,
    Temperature,
};
use filature::persistence::connect_and_migrate;
use filature::persistence::materials::SqlxMaterialRepository;

fn sample(name: &str) -> NewMaterial {
    NewMaterial {
        name: name.to_string(),
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

/// Expected values for all seven domain fields, used to assert a full
/// round-trip so a column-mapping swap (e.g. bed_c <-> nozzle_c) fails.
struct Expected<'a> {
    name: &'a str,
    density: f64,
    drying_temp: u16,
    drying_time_h: u16,
    sensitivity: Sensitivity,
    nozzle: u16,
    bed: u16,
}

fn assert_fields(m: &domain::materials::Material, expected: Expected) {
    assert_eq!(m.name, expected.name, "name mismatch");
    assert_eq!(m.density.value(), expected.density, "density mismatch");
    assert_eq!(
        m.drying.temp.value(),
        expected.drying_temp,
        "drying temp mismatch"
    );
    assert_eq!(
        m.drying.time_h, expected.drying_time_h,
        "drying time mismatch"
    );
    assert_eq!(m.sensitivity, expected.sensitivity, "sensitivity mismatch");
    assert_eq!(m.nozzle.value(), expected.nozzle, "nozzle mismatch");
    assert_eq!(m.bed.value(), expected.bed, "bed mismatch");
}

#[tokio::test]
async fn insert_assigns_ulid_and_roundtrips() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    let created = repo.insert(sample("PLA")).await.unwrap();
    assert_eq!(created.id.as_str().len(), 26); // ULID is 26 chars
    let expected = || Expected {
        name: "PLA",
        density: 1.24,
        drying_temp: 45,
        drying_time_h: 6,
        sensitivity: Sensitivity::Low,
        nozzle: 210,
        bed: 60,
    };
    assert_fields(&created, expected());

    let all = repo.list().await.unwrap();
    let pla = all.iter().find(|m| m.name == "PLA").unwrap();
    assert_fields(pla, expected());
}

#[tokio::test]
async fn update_persists() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    let mut m = repo.insert(sample("PETG")).await.unwrap();
    m.name = "PETG-CF".to_string();
    m.density = Density::new(1.3).unwrap();
    m.drying = DryingParams {
        temp: Temperature::new(65),
        time_h: 8,
    };
    m.sensitivity = Sensitivity::High;
    m.nozzle = Temperature::new(245);
    m.bed = Temperature::new(90);

    let updated = repo.update(m.clone()).await.unwrap();
    let expected = || Expected {
        name: "PETG-CF",
        density: 1.3,
        drying_temp: 65,
        drying_time_h: 8,
        sensitivity: Sensitivity::High,
        nozzle: 245,
        bed: 90,
    };
    assert_fields(&updated, expected());

    let reread = repo.list().await.unwrap();
    let petg = reread.iter().find(|x| x.name == "PETG-CF").unwrap();
    assert_fields(petg, expected());
}

#[tokio::test]
async fn duplicate_name_maps_to_duplicate_error() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    repo.insert(sample("ASA")).await.unwrap();
    assert!(matches!(
        repo.insert(sample("ASA")).await,
        Err(RepositoryError::Duplicate(_))
    ));
    assert!(repo.exists_by_name("ASA").await.unwrap());
    assert!(!repo.exists_by_name("NOPE-ASA").await.unwrap());
}

#[tokio::test]
async fn update_name_collision_maps_to_duplicate_error_with_name() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    repo.insert(sample("ABS")).await.unwrap();
    let mut other = repo.insert(sample("TPU")).await.unwrap();

    other.name = "ABS".to_string();
    let err = repo.update(other).await.unwrap_err();
    match err {
        RepositoryError::Duplicate(n) => assert_eq!(n, "ABS"),
        other => panic!("expected Duplicate(\"ABS\"), got {other:?}"),
    }
}
