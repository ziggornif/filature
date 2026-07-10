use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

pub mod locations;
pub mod materials;
pub mod spools;

pub type Db = Pool<Postgres>;

/// Open the pool and run embedded migrations.
/// `url` is a `postgres://` connection string (ADR-0003 — PostgreSQL is the
/// persistence engine; no WAL/journal-mode concern, that was SQLite-specific).
pub async fn connect_and_migrate(url: &str) -> Result<Db, sqlx::Error> {
    let pool = PgPoolOptions::new().connect(url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
