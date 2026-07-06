//! SQLx adapter for the materials slice SPI (`MaterialRepository`).
//! Rows are mapped manually: SQLx stores primitives, domain newtypes are
//! reconstructed on read and unwrapped on write (ADR-0003, PostgreSQL).

use crate::persistence::Db;
use async_trait::async_trait;
use domain::materials::{
    Density, DryingParams, Material, MaterialId, MaterialName, MaterialRepository, NewMaterial,
    RepositoryError, Sensitivity, Temperature,
};
use ulid::Ulid;

pub struct SqlxMaterialRepository {
    pool: Db,
}

impl SqlxMaterialRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

/// A UNIQUE violation on `materials.name` becomes `Duplicate(name)`; anything
/// else is an opaque `Backend` error (the domain never sees SQL details).
fn backend(e: sqlx::Error, name: &str) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && db.is_unique_violation()
    {
        return RepositoryError::Duplicate(name.to_string());
    }
    RepositoryError::Backend(e.to_string())
}

#[async_trait]
impl MaterialRepository for SqlxMaterialRepository {
    async fn list(&self) -> Result<Vec<Material>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT id, name, density, drying_temp_c, drying_time_h,
                      sensitivity, nozzle_c, bed_c
               FROM materials ORDER BY name"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        rows.into_iter()
            .map(|r| {
                Ok(Material {
                    id: MaterialId::new(r.id),
                    name: MaterialName::new(r.name)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    density: Density::new(r.density)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    drying: DryingParams {
                        temp: Temperature::new(r.drying_temp_c as u16),
                        time_h: r.drying_time_h as u16,
                    },
                    sensitivity: Sensitivity::parse(&r.sensitivity)
                        .map_err(|e| RepositoryError::Backend(e.to_string()))?,
                    nozzle: Temperature::new(r.nozzle_c as u16),
                    bed: Temperature::new(r.bed_c as u16),
                })
            })
            .collect()
    }

    async fn insert(&self, m: NewMaterial) -> Result<Material, RepositoryError> {
        let id = Ulid::new().to_string();
        sqlx::query!(
            r#"INSERT INTO materials
               (id, name, density, drying_temp_c, drying_time_h, sensitivity, nozzle_c, bed_c)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
            id,
            m.name.as_str(),
            m.density.value(),
            m.drying.temp.value() as i32,
            m.drying.time_h as i32,
            m.sensitivity.as_str(),
            m.nozzle.value() as i32,
            m.bed.value() as i32,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| backend(e, m.name.as_str()))?;

        Ok(Material {
            id: MaterialId::new(id),
            name: m.name,
            density: m.density,
            drying: m.drying,
            sensitivity: m.sensitivity,
            nozzle: m.nozzle,
            bed: m.bed,
        })
    }

    async fn update(&self, m: Material) -> Result<Material, RepositoryError> {
        sqlx::query!(
            r#"UPDATE materials SET
                 name=$2, density=$3, drying_temp_c=$4, drying_time_h=$5,
                 sensitivity=$6, nozzle_c=$7, bed_c=$8
               WHERE id=$1"#,
            m.id.as_str(),
            m.name.as_str(),
            m.density.value(),
            m.drying.temp.value() as i32,
            m.drying.time_h as i32,
            m.sensitivity.as_str(),
            m.nozzle.value() as i32,
            m.bed.value() as i32,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| backend(e, m.name.as_str()))?;
        Ok(m)
    }

    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM materials WHERE name=$1) AS "exists!""#,
            name
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;
        Ok(row.exists)
    }
}
