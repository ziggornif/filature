//! SQLx adapter for the locations slice SPI (`LocationRepository`).
//! Rows are mapped manually: SQLx stores primitives, domain newtypes are
//! reconstructed on read and unwrapped on write (ADR-0003, PostgreSQL).

use crate::persistence::Db;
use async_trait::async_trait;
use domain::locations::{Location, LocationName, LocationRepository, NewLocation, RepositoryError};
use domain::shared::LocationId;
use ulid::Ulid;

pub struct SqlxLocationRepository {
    pool: Db,
}

impl SqlxLocationRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

fn backend(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

#[async_trait]
impl LocationRepository for SqlxLocationRepository {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError> {
        let rows = sqlx::query!(r#"SELECT id, name, note FROM locations ORDER BY name"#)
            .fetch_all(&self.pool)
            .await
            .map_err(backend)?;

        rows.into_iter()
            .map(|r| {
                Ok(Location {
                    id: LocationId::new(r.id),
                    name: LocationName::new(r.name)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    note: r.note,
                })
            })
            .collect()
    }

    async fn insert(&self, l: NewLocation) -> Result<Location, RepositoryError> {
        let id = Ulid::new().to_string();
        sqlx::query!(
            r#"INSERT INTO locations (id, name, note) VALUES ($1, $2, $3)"#,
            id,
            l.name.as_str(),
            l.note,
        )
        .execute(&self.pool)
        .await
        .map_err(backend)?;

        Ok(Location {
            id: LocationId::new(id),
            name: l.name,
            note: l.note,
        })
    }

    async fn update(&self, l: Location) -> Result<Location, RepositoryError> {
        let result = sqlx::query!(
            r#"UPDATE locations SET name=$2, note=$3 WHERE id=$1"#,
            l.id.as_str(),
            l.name.as_str(),
            l.note,
        )
        .execute(&self.pool)
        .await
        .map_err(backend)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(l.id));
        }
        Ok(l)
    }

    async fn delete(&self, id: &LocationId) -> Result<(), RepositoryError> {
        let result = sqlx::query!(r#"DELETE FROM locations WHERE id=$1"#, id.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id.clone()));
        }
        Ok(())
    }

    async fn count_spools(&self, id: &LocationId) -> Result<u64, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) AS "count!" FROM spools WHERE location_id=$1"#,
            id.as_str()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;

        Ok(row.count as u64)
    }
}
