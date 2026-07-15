mod support;

use domain::instance_transfer::{InstanceTransferRepository, SnapshotManufacturer, TransferError};
use filature::persistence::connect_and_migrate;
use filature::persistence::instance_transfer::SqlxInstanceTransferRepository;

#[tokio::test]
async fn exported_snapshot_round_trips_and_failed_replace_rolls_back() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repository = SqlxInstanceTransferRepository::new(pool.clone());

    sqlx::raw_sql("DELETE FROM printer_slots; DELETE FROM printers; DELETE FROM spools; DELETE FROM materials; DELETE FROM locations; DELETE FROM manufacturers; DELETE FROM instance_configuration")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"INSERT INTO materials
           (id, name, density, drying_temp_c, drying_time_h, sensitivity, nozzle_c, bed_c)
           VALUES ('mat-1', 'PLA', 1.24, 45, 6, 'Low', 210, 60);
           INSERT INTO manufacturers (id, name, country) VALUES ('maker-1', 'Acme', 'FR');
           INSERT INTO locations (id, name, note) VALUES ('loc-1', 'Shelf', 'Dry');
           INSERT INTO spools
             (id, material_id, spool_type, colour_hex, colour_name, diameter, net_weight,
              remaining_weight, price_paid, status, location_id, manufacturer_id, notes,
              purchased_at, opened_at, created_at)
           VALUES
             ('spool-1', 'mat-1', 'Complete', '#AABBCC', '#AABBCC', '1.75', 1000,
              420, 24.9900, 'Open', 'loc-1', 'maker-1', 'Prototype spool',
              '2026-07-01', '2026-07-12', '2026-07-13T12:00:00Z'),
             ('spool-2', 'mat-1', 'Complete', '#112233', 'Navy', '1.75', 1000,
              800, 19.5000, 'Sealed', 'loc-1', 'maker-1', NULL,
              NULL, NULL, '2026-07-14T12:00:00Z');
           INSERT INTO printers (id, name, brand, model, module_kind, module_count)
             VALUES
               ('printer-1', 'Workshop', 'other', 'Palette', 'multi_colour', 3),
               ('printer-2', 'Core', 'prusa', 'CORE One', 'indx', 8);
           INSERT INTO printer_slots
             (id, printer_id, group_label, slot_key, position, spool_id)
           VALUES
             ('slot-1', 'printer-1', 'multi_colour', 'multi-0', 0, 'spool-1'),
             ('slot-2', 'printer-1', 'multi_colour', 'multi-1', 1, NULL),
             ('slot-3', 'printer-1', 'multi_colour', 'multi-2', 2, 'spool-2');
           INSERT INTO instance_configuration (singleton, low_stock_threshold) VALUES (TRUE, 21)"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let original = repository.export_snapshot().await.unwrap();
    assert_eq!(original.spools[0].notes.as_deref(), Some("Prototype spool"));
    assert_eq!(
        original.spools[0].purchased_at.unwrap().to_string(),
        "2026-07-01"
    );
    assert_eq!(
        original.spools[0].opened_at.unwrap().to_string(),
        "2026-07-12"
    );
    assert_eq!(original.printers.len(), 2);
    let indx = original
        .printers
        .iter()
        .find(|printer| printer.id == "printer-2")
        .unwrap();
    assert_eq!(indx.module_kind, "indx");
    assert_eq!(indx.module_count, Some(8));
    assert_eq!(original.printers[0].slots.len(), 3);
    assert_eq!(
        original.printers[0].slots[0].spool_id.as_deref(),
        Some("spool-1")
    );
    assert_eq!(original.printers[0].slots[1].spool_id, None);
    assert_eq!(
        original.printers[0].slots[2].spool_id.as_deref(),
        Some("spool-2")
    );
    sqlx::raw_sql("DELETE FROM printer_slots; DELETE FROM printers; DELETE FROM spools; UPDATE instance_configuration SET low_stock_threshold = 99")
        .execute(&pool)
        .await
        .unwrap();

    repository.replace_snapshot(original.clone()).await.unwrap();
    assert_eq!(repository.export_snapshot().await.unwrap(), original);
    assert!(
        sqlx::query(
            "UPDATE printer_slots SET spool_id = 'spool-1' WHERE printer_id = 'printer-1' AND slot_key = 'multi-1'",
        )
        .execute(&pool)
        .await
        .is_err()
    );

    // This duplicate unique name fails after the transaction has already
    // deleted existing rows and inserted the first manufacturer. Dropping the
    // failed transaction must restore the exact prior instance.
    let mut invalid = original.clone();
    invalid.manufacturers.push(SnapshotManufacturer {
        id: "maker-2".into(),
        name: "Acme".into(),
        country: None,
    });
    assert!(matches!(
        repository.replace_snapshot(invalid).await,
        Err(TransferError::Backend(_))
    ));
    assert_eq!(repository.export_snapshot().await.unwrap(), original);
}
