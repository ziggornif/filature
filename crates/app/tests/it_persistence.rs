mod support;

use filature::persistence::connect_and_migrate;

#[tokio::test]
async fn opens_and_migrates_against_postgres() {
    let url = support::postgres_url().await;
    let db = connect_and_migrate(&url).await.unwrap();
    // migrations ran => the sqlx bookkeeping table exists
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(count >= 1);
}
