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

#[tokio::test]
async fn insert_assigns_ulid_and_roundtrips() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    let created = repo.insert(sample("PLA")).await.unwrap();
    assert_eq!(created.id.as_str().len(), 26); // ULID is 26 chars
    assert_eq!(created.name, "PLA");

    let all = repo.list().await.unwrap();
    let pla = all.iter().find(|m| m.name == "PLA").unwrap();
    assert_eq!(pla.density.value(), 1.24);
    assert_eq!(pla.sensitivity, Sensitivity::Low);
}

#[tokio::test]
async fn update_persists() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxMaterialRepository::new(pool);

    let mut m = repo.insert(sample("PETG")).await.unwrap();
    m.nozzle = Temperature::new(245);
    repo.update(m.clone()).await.unwrap();

    let reread = repo.list().await.unwrap();
    let petg = reread.iter().find(|x| x.name == "PETG").unwrap();
    assert_eq!(petg.nozzle.value(), 245);
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
