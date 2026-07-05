use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

pub type Db = Pool<Sqlite>;

/// Open the pool, enable WAL, and run embedded migrations.
/// `url` accepts "sqlite://file.db" (creates if missing) or "sqlite::memory:".
pub async fn connect_and_migrate(url: &str) -> Result<Db, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
