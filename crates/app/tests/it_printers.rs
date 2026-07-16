mod support;

use domain::printers::{
    FeedMode, Module, NewPrinter, PrinterBrand, PrinterName, PrinterRepository, PrintersService,
    PrintersUseCases, RepositoryError,
};
use domain::shared::{PrinterId, SpoolId};
use filature::persistence::{connect_and_migrate, printers::SqlxPrinterRepository};
use std::sync::Arc;

fn sample(name: &str, module: Module) -> NewPrinter {
    NewPrinter {
        name: PrinterName::new(name).unwrap(),
        brand: PrinterBrand::BambuLab,
        model: "P1S".into(),
        heads: 1,
        module,
        ams_units: 0,
        feed_modes: vec![FeedMode::Direct],
    }
}

async fn spool(pool: &sqlx::PgPool, id: &str, status: &str, remaining: f64) {
    sqlx::query("INSERT INTO materials(id,name,density,drying_temp_c,drying_time_h,sensitivity,nozzle_c,bed_c) VALUES('mat','PLA',1.24,50,6,'Low',210,60) ON CONFLICT(id) DO NOTHING")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO manufacturers(id,name,country) VALUES('maker','Acme',NULL) ON CONFLICT(id) DO NOTHING")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO spools(id,material_id,spool_type,colour_hex,colour_name,diameter,net_weight,remaining_weight,price_paid,status,manufacturer_id) VALUES($1,'mat','Complete','#ff0000','Red','1.75',1000,$2,20,$3,'maker')")
        .bind(id).bind(remaining).bind(status).execute(pool).await.unwrap();
}

#[tokio::test]
async fn insert_list_update_preserves_surviving_keys_and_delete_cascades() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxPrinterRepository::new(pool.clone());
    let mut topology = sample("Workshop", Module::None);
    topology.ams_units = 2;
    topology.feed_modes = vec![FeedMode::AmsFed];
    let created = repo.insert(topology).await.unwrap();
    assert_eq!(created.id.as_str().len(), 26);
    assert_eq!(created.slots.len(), 8);
    spool(&pool, "topology-survives", "Open", 700.0).await;
    spool(&pool, "topology-unloads", "Sealed", 1000.0).await;
    repo.set_slot_spool(
        &created.id,
        "ams0-0",
        Some(&SpoolId::new("topology-survives")),
    )
    .await
    .unwrap();
    repo.set_slot_spool(
        &created.id,
        "ams1-0",
        Some(&SpoolId::new("topology-unloads")),
    )
    .await
    .unwrap();
    let surviving_id: String = sqlx::query_scalar(
        "SELECT id FROM printer_slots WHERE printer_id=$1 AND slot_key='ams0-0'",
    )
    .bind(created.id.as_str())
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut edited = created.clone();
    edited.ams_units = 1;
    let edited = repo.update(edited).await.unwrap();
    assert_eq!(edited.slots.len(), 4);
    let after_id: String = sqlx::query_scalar(
        "SELECT id FROM printer_slots WHERE printer_id=$1 AND slot_key='ams0-0'",
    )
    .bind(created.id.as_str())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(surviving_id, after_id);
    let surviving_spool: Option<String> = sqlx::query_scalar(
        "SELECT spool_id FROM printer_slots WHERE printer_id=$1 AND slot_key='ams0-0'",
    )
    .bind(created.id.as_str())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(surviving_spool.as_deref(), Some("topology-survives"));
    let removed_spool_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM printer_slots WHERE spool_id='topology-unloads'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_spool_count, 0);
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
        heads: 1,
        module: Module::None,
        ams_units: 0,
        feed_modes: vec![FeedMode::Direct],
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

#[tokio::test]
async fn loading_moves_atomically_guards_status_and_unloads() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    spool(&pool, "loadable", "Open", 800.0).await;
    spool(&pool, "empty", "Empty", 0.0).await;
    let repo = Arc::new(SqlxPrinterRepository::new(pool.clone()));
    let first = repo.insert(sample("First", Module::None)).await.unwrap();
    let second = repo.insert(sample("Second", Module::None)).await.unwrap();
    let service = PrintersService::new(repo);

    service
        .load_slot(first.id.clone(), "head-0".into(), SpoolId::new("loadable"))
        .await
        .unwrap();
    service
        .load_slot(second.id.clone(), "head-0".into(), SpoolId::new("loadable"))
        .await
        .unwrap();
    let holder: String =
        sqlx::query_scalar("SELECT printer_id FROM printer_slots WHERE spool_id='loadable'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(holder, second.id.as_str());
    assert!(matches!(
        service
            .load_slot(first.id.clone(), "head-0".into(), SpoolId::new("empty"))
            .await,
        Err(RepositoryError::Domain(
            domain::shared::DomainError::SpoolNotLoadable
        ))
    ));
    service
        .unload_slot(second.id, "head-0".into())
        .await
        .unwrap();
    service
        .unload_spool(SpoolId::new("loadable"))
        .await
        .unwrap();
    // Scope to this test's own spools: the shared testcontainer DB (one
    // database reused across all `it_printers` tests, run in parallel) means a
    // global `spool_id IS NOT NULL` count would race with slots loaded by other
    // tests. Asserting on our two spool ids keeps the check isolated.
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM printer_slots WHERE spool_id IN ('loadable','empty')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn adapter_enforces_exclusivity_filters_options_joins_card_and_delete_frees_spool() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    spool(&pool, "open", "Open", 750.0).await;
    spool(&pool, "sealed", "Sealed", 1000.0).await;
    spool(&pool, "archived", "Archived", 500.0).await;
    let repo = SqlxPrinterRepository::new(pool.clone());
    let first = repo.insert(sample("First", Module::None)).await.unwrap();
    let second = repo.insert(sample("Second", Module::None)).await.unwrap();
    repo.set_slot_spool(&first.id, "head-0", Some(&SpoolId::new("open")))
        .await
        .unwrap();

    let duplicate = sqlx::query(
        "UPDATE printer_slots SET spool_id='open' WHERE printer_id=$1 AND slot_key='head-0'",
    )
    .bind(second.id.as_str())
    .execute(&pool)
    .await;
    assert!(
        duplicate
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .is_unique_violation()
    );
    // `loadable_spools(None)` is a global read; the shared testcontainer DB is
    // reused across all `it_printers` tests (run in parallel), so filter to this
    // test's own spool ids before asserting. Among them only `sealed` is
    // loadable: `open` is loaded here, `archived` is not a loadable status.
    let own = ["open", "sealed", "archived"];
    let options = repo.loadable_spools(None).await.unwrap();
    assert_eq!(
        options
            .iter()
            .map(|s| s.id.as_str())
            .filter(|id| own.contains(id))
            .collect::<Vec<_>>(),
        vec!["sealed"]
    );
    let current = repo
        .loadable_spools(Some(&SpoolId::new("open")))
        .await
        .unwrap();
    assert!(current.iter().any(|s| s.id.as_str() == "open"));
    let card = repo
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.id == first.id)
        .unwrap();
    let loaded = card.slots[0].loaded_spool.as_ref().unwrap();
    assert_eq!(loaded.manufacturer_name.as_deref(), Some("Acme"));
    assert_eq!(loaded.colour_hex.as_deref(), Some("#ff0000"));
    assert_eq!(loaded.material_name, "PLA");
    assert_eq!(loaded.remaining_pct(), 75);
    repo.delete(&first.id).await.unwrap();
    assert!(
        repo.loadable_spools(None)
            .await
            .unwrap()
            .iter()
            .any(|s| s.id.as_str() == "open")
    );
}

#[tokio::test]
async fn slot_write_disambiguates_unknown_spool_printer_and_slot() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo = SqlxPrinterRepository::new(pool);
    let printer = repo.insert(sample("Known", Module::None)).await.unwrap();
    assert!(matches!(
        repo.set_slot_spool(&printer.id, "ext", Some(&SpoolId::new("missing")))
            .await,
        Err(RepositoryError::UnknownSpool(_))
    ));
    assert!(matches!(
        repo.set_slot_spool(&PrinterId::new("missing"), "ext", None)
            .await,
        Err(RepositoryError::NotFound(_))
    ));
    assert!(matches!(
        repo.set_slot_spool(&printer.id, "missing", None).await,
        Err(RepositoryError::SlotNotFound { .. })
    ));
}
