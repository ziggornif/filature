//! SQLx adapter for the manufacturers slice SPI (`ManufacturerRepository`).
//! Rows are mapped manually: SQLx stores primitives, domain newtypes are
//! reconstructed on read and unwrapped on write (ADR-0003, PostgreSQL).

use crate::persistence::Db;
use async_trait::async_trait;
use domain::manufacturers::{
    Manufacturer, ManufacturerName, ManufacturerRepository, NewManufacturer, RepositoryError,
};
use domain::shared::ManufacturerId;
use ulid::Ulid;

pub struct SqlxManufacturerRepository {
    pool: Db,
}

impl SqlxManufacturerRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

fn backend(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

/// A UNIQUE violation on `manufacturers.name` becomes `Duplicate(name)` so
/// `seed_defaults` stays idempotent under races; anything else is opaque.
fn insert_error(e: sqlx::Error, name: &str) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && db.is_unique_violation()
    {
        return RepositoryError::Duplicate(name.to_string());
    }
    RepositoryError::Backend(e.to_string())
}

#[async_trait]
impl ManufacturerRepository for SqlxManufacturerRepository {
    async fn list(&self) -> Result<Vec<Manufacturer>, RepositoryError> {
        let rows = sqlx::query!(r#"SELECT id, name, country FROM manufacturers ORDER BY name"#)
            .fetch_all(&self.pool)
            .await
            .map_err(backend)?;

        rows.into_iter()
            .map(|r| {
                Ok(Manufacturer {
                    id: ManufacturerId::new(r.id),
                    name: ManufacturerName::new(r.name)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    country: r.country,
                })
            })
            .collect()
    }

    async fn insert(&self, m: NewManufacturer) -> Result<Manufacturer, RepositoryError> {
        let id = Ulid::new().to_string();
        sqlx::query!(
            r#"INSERT INTO manufacturers (id, name, country) VALUES ($1, $2, $3)"#,
            id,
            m.name.as_str(),
            m.country,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| insert_error(e, m.name.as_str()))?;

        Ok(Manufacturer {
            id: ManufacturerId::new(id),
            name: m.name,
            country: m.country,
        })
    }

    async fn delete(&self, id: &ManufacturerId) -> Result<(), RepositoryError> {
        let result = sqlx::query!(r#"DELETE FROM manufacturers WHERE id=$1"#, id.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id.clone()));
        }
        Ok(())
    }

    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM manufacturers WHERE name=$1) AS "exists!""#,
            name
        )
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;

        Ok(row.exists)
    }

    async fn count_spools(&self, id: &ManufacturerId) -> Result<u64, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) AS "count!" FROM spools WHERE manufacturer_id=$1"#,
            id.as_str()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;

        Ok(row.count as u64)
    }
}
