mod support;

use domain::printers::{
    Module, NewPrinter, PrinterBrand, PrinterName, PrinterRepository, RepositoryError,
};
use domain::shared::PrinterId;
use filature::persistence::{connect_and_migrate, printers::SqlxPrinterRepository};

fn sample(name: &str, module: Module) -> NewPrinter {
    NewPrinter {
        name: PrinterName::new(name).unwrap(),
        brand: PrinterBrand::BambuLab,
        model: "P1S".into(),
        module,
    }
}

#[tokio::test]
async fn insert_list_update_preserves_surviving_keys_and_delete_cascades() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxPrinterRepository::new(pool.clone());
    let created = repo.insert(sample("Workshop", Module::Ams)).await.unwrap();
    assert_eq!(created.id.as_str().len(), 26);
    assert_eq!(created.slots.len(), 5);
    let external_id: String =
        sqlx::query_scalar("SELECT id FROM printer_slots WHERE printer_id=$1 AND slot_key='ext'")
            .bind(created.id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut edited = created.clone();
    edited.module = Module::None;
    let edited = repo.update(edited).await.unwrap();
    assert_eq!(edited.slots.len(), 1);
    let after_id: String =
        sqlx::query_scalar("SELECT id FROM printer_slots WHERE printer_id=$1 AND slot_key='ext'")
            .bind(created.id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(external_id, after_id);
    assert_eq!(
        repo.list()
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.id == created.id)
            .unwrap()
            .module,
        Module::None
    );
    repo.delete(&created.id).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM printer_slots WHERE printer_id=$1")
        .bind(created.id.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn unknown_update_and_delete_are_not_found() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxPrinterRepository::new(pool);
    let missing = PrinterId::new("missing");
    let p = domain::printers::Printer {
        id: missing.clone(),
        name: PrinterName::new("Ghost").unwrap(),
        brand: PrinterBrand::Other,
        model: "Ghost".into(),
        module: Module::None,
        slots: vec![],
    };
    assert!(matches!(
        repo.update(p).await,
        Err(RepositoryError::NotFound(_))
    ));
    assert!(matches!(
        repo.delete(&missing).await,
        Err(RepositoryError::NotFound(_))
    ));
}
