use filature::persistence::connect_and_migrate;

#[tokio::test]
async fn opens_and_migrates_in_memory() {
    let db = connect_and_migrate("sqlite::memory:").await.unwrap();
    // migrations ran => the sqlx bookkeeping table exists
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(count >= 1);
}
