//! SQLx adapter for the spools slice SPI (`SpoolRepository`). Mirrors the
//! materials adapter: rows are mapped manually, domain newtypes are
//! reconstructed on read and unwrapped on write (ADR-0003, PostgreSQL).
//! `list`/`get` join `materials` to populate the display-only
//! `material_name`/`density` fields, and LEFT JOIN `locations` to populate
//! the display-only `location_name` field, carried by the shared read
//! models.

use crate::persistence::Db;
use async_trait::async_trait;
use domain::shared::{Grams, LocationId, MaterialId, Money};
use domain::spools::{
    Colour, Diameter, NewSpool, RepositoryError, Spool, SpoolDetail, SpoolFilter, SpoolId,
    SpoolListItem, SpoolRepository, SpoolSort, SpoolStatus,
};
use ulid::Ulid;

pub struct SqlxSpoolRepository {
    pool: Db,
}

impl SqlxSpoolRepository {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

/// A foreign-key violation on `spools.material_id` becomes
/// `UnknownMaterial(material_id)`; anything else is an opaque `Backend`
/// error (the domain never sees SQL details). Used on SELECT paths, which
/// can never raise an FK violation, so `material_id` is passed as `""`.
fn backend(e: sqlx::Error, material_id: &str) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && db.is_foreign_key_violation()
    {
        return RepositoryError::UnknownMaterial(MaterialId::new(material_id.to_string()));
    }
    RepositoryError::Backend(e.to_string())
}

/// INSERT/UPDATE on `spools` can violate either the `material_id` or the
/// `location_id` foreign key. Postgres auto-names these constraints
/// `spools_material_id_fkey` / `spools_location_id_fkey`, so we disambiguate
/// by inspecting the violated constraint name rather than assuming which FK
/// failed. Unrecognised constraints (or no constraint name at all) fall back
/// to an opaque `Backend` error.
fn write_error(e: sqlx::Error, material_id: &str, location_id: Option<&str>) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && let Some(constraint) = db.constraint()
    {
        if constraint.contains("location_id") {
            return RepositoryError::UnknownLocation(LocationId::new(
                location_id.unwrap_or_default().to_string(),
            ));
        }
        if constraint.contains("material_id") {
            return RepositoryError::UnknownMaterial(MaterialId::new(material_id.to_string()));
        }
    }
    RepositoryError::Backend(e.to_string())
}

fn build_colour(hex: String, name: Option<String>) -> Result<Colour, RepositoryError> {
    Colour::new(hex, name).map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_diameter(s: &str) -> Result<Diameter, RepositoryError> {
    Diameter::parse(s).map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_status(s: &str) -> Result<SpoolStatus, RepositoryError> {
    SpoolStatus::parse(s).map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_grams(v: f64) -> Result<Grams, RepositoryError> {
    Grams::new(v).map_err(|e| RepositoryError::Backend(e.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn to_list_item(
    id: String,
    material_name: String,
    colour_hex: String,
    colour_name: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    status: String,
    density: f64,
    location_name: Option<String>,
) -> Result<SpoolListItem, RepositoryError> {
    Ok(SpoolListItem {
        id: SpoolId::new(id),
        material_name,
        colour: build_colour(colour_hex, colour_name)?,
        diameter: build_diameter(&diameter)?,
        remaining_weight: build_grams(remaining_weight)?,
        net_weight: build_grams(net_weight)?,
        status: build_status(&status)?,
        density,
        location_name,
    })
}

#[async_trait]
impl SpoolRepository for SqlxSpoolRepository {
    async fn insert(&self, s: NewSpool) -> Result<Spool, RepositoryError> {
        let id = Ulid::new().to_string();
        let location_id = s.location_id.as_ref().map(|l| l.as_str());
        sqlx::query!(
            r#"INSERT INTO spools
               (id, material_id, colour_hex, colour_name, diameter, net_weight, remaining_weight, price_paid, status, location_id)
               VALUES ($1, $2, $3, $4, $5, $6, $6, $7, 'Sealed', $8)"#,
            id,
            s.material_id.as_str(),
            s.colour.hex(),
            s.colour.name(),
            s.diameter.as_str(),
            s.net_weight.value(),
            s.price_paid.value(),
            location_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| write_error(e, s.material_id.as_str(), location_id))?;

        Ok(Spool {
            id: SpoolId::new(id),
            material_id: s.material_id,
            colour: s.colour,
            diameter: s.diameter,
            net_weight: s.net_weight,
            remaining_weight: s.net_weight,
            price_paid: s.price_paid,
            status: SpoolStatus::Sealed,
            location_id: s.location_id,
        })
    }

    async fn update(&self, s: Spool) -> Result<Spool, RepositoryError> {
        let location_id = s.location_id.as_ref().map(|l| l.as_str());
        let result = sqlx::query!(
            r#"UPDATE spools SET
                 material_id=$2, colour_hex=$3, colour_name=$4, diameter=$5,
                 net_weight=$6, remaining_weight=$7, price_paid=$8, status=$9, location_id=$10
               WHERE id=$1"#,
            s.id.as_str(),
            s.material_id.as_str(),
            s.colour.hex(),
            s.colour.name(),
            s.diameter.as_str(),
            s.net_weight.value(),
            s.remaining_weight.value(),
            s.price_paid.value(),
            s.status.as_str(),
            location_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| write_error(e, s.material_id.as_str(), location_id))?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(s.id));
        }
        Ok(s)
    }

    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError> {
        let material_id = filter.material_id.as_ref().map(|m| m.as_str());
        let status = filter.status.map(|s| s.as_str());

        let items = match sort {
            SpoolSort::CreatedDesc => {
                let rows = sqlx::query!(
                    r#"SELECT s.id, s.colour_hex, s.colour_name, s.diameter,
                              s.net_weight, s.remaining_weight, s.status,
                              m.name AS material_name, m.density,
                              l.name AS "location_name?"
                       FROM spools s
                       JOIN materials m ON m.id = s.material_id
                       LEFT JOIN locations l ON l.id = s.location_id
                       WHERE ($1::text IS NULL OR s.material_id = $1)
                         AND ($2::text IS NULL OR s.status = $2)
                         AND (s.status <> 'Archived' OR $2 = 'Archived')
                       ORDER BY s.created_at DESC"#,
                    material_id,
                    status
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| backend(e, ""))?;
                rows.into_iter()
                    .map(|r| {
                        to_list_item(
                            r.id,
                            r.material_name,
                            r.colour_hex,
                            r.colour_name,
                            r.diameter,
                            r.net_weight,
                            r.remaining_weight,
                            r.status,
                            r.density,
                            r.location_name,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            SpoolSort::RemainingRatioAsc => {
                let rows = sqlx::query!(
                    r#"SELECT s.id, s.colour_hex, s.colour_name, s.diameter,
                              s.net_weight, s.remaining_weight, s.status,
                              m.name AS material_name, m.density,
                              l.name AS "location_name?"
                       FROM spools s
                       JOIN materials m ON m.id = s.material_id
                       LEFT JOIN locations l ON l.id = s.location_id
                       WHERE ($1::text IS NULL OR s.material_id = $1)
                         AND ($2::text IS NULL OR s.status = $2)
                         AND (s.status <> 'Archived' OR $2 = 'Archived')
                       ORDER BY (s.remaining_weight / s.net_weight) ASC"#,
                    material_id,
                    status
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| backend(e, ""))?;
                rows.into_iter()
                    .map(|r| {
                        to_list_item(
                            r.id,
                            r.material_name,
                            r.colour_hex,
                            r.colour_name,
                            r.diameter,
                            r.net_weight,
                            r.remaining_weight,
                            r.status,
                            r.density,
                            r.location_name,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            SpoolSort::RemainingRatioDesc => {
                let rows = sqlx::query!(
                    r#"SELECT s.id, s.colour_hex, s.colour_name, s.diameter,
                              s.net_weight, s.remaining_weight, s.status,
                              m.name AS material_name, m.density,
                              l.name AS "location_name?"
                       FROM spools s
                       JOIN materials m ON m.id = s.material_id
                       LEFT JOIN locations l ON l.id = s.location_id
                       WHERE ($1::text IS NULL OR s.material_id = $1)
                         AND ($2::text IS NULL OR s.status = $2)
                         AND (s.status <> 'Archived' OR $2 = 'Archived')
                       ORDER BY (s.remaining_weight / s.net_weight) DESC"#,
                    material_id,
                    status
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| backend(e, ""))?;
                rows.into_iter()
                    .map(|r| {
                        to_list_item(
                            r.id,
                            r.material_name,
                            r.colour_hex,
                            r.colour_name,
                            r.diameter,
                            r.net_weight,
                            r.remaining_weight,
                            r.status,
                            r.density,
                            r.location_name,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(items)
    }

    async fn get(&self, id: &SpoolId) -> Result<Option<SpoolDetail>, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT s.id, s.material_id, s.colour_hex, s.colour_name, s.diameter,
                      s.net_weight, s.remaining_weight, s.price_paid, s.status,
                      s.location_id,
                      m.name AS material_name, m.density,
                      l.name AS "location_name?"
               FROM spools s
               JOIN materials m ON m.id = s.material_id
               LEFT JOIN locations l ON l.id = s.location_id
               WHERE s.id = $1"#,
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        let Some(r) = row else {
            return Ok(None);
        };

        Ok(Some(SpoolDetail {
            id: SpoolId::new(r.id),
            material_id: MaterialId::new(r.material_id),
            material_name: r.material_name,
            colour: build_colour(r.colour_hex, r.colour_name)?,
            diameter: build_diameter(&r.diameter)?,
            net_weight: build_grams(r.net_weight)?,
            remaining_weight: build_grams(r.remaining_weight)?,
            price_paid: Money::from_decimal(r.price_paid)
                .map_err(|e| RepositoryError::Backend(e.to_string()))?,
            status: build_status(&r.status)?,
            density: r.density,
            location_name: r.location_name,
            location_id: r.location_id,
        }))
    }

    async fn find(&self, id: &SpoolId) -> Result<Option<Spool>, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT id, material_id, colour_hex, colour_name, diameter,
                      net_weight, remaining_weight, price_paid, status, location_id
               FROM spools WHERE id = $1"#,
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        let Some(r) = row else {
            return Ok(None);
        };

        Ok(Some(Spool {
            id: SpoolId::new(r.id),
            material_id: MaterialId::new(r.material_id),
            colour: build_colour(r.colour_hex, r.colour_name)?,
            diameter: build_diameter(&r.diameter)?,
            net_weight: build_grams(r.net_weight)?,
            remaining_weight: build_grams(r.remaining_weight)?,
            price_paid: Money::from_decimal(r.price_paid)
                .map_err(|e| RepositoryError::Backend(e.to_string()))?,
            status: build_status(&r.status)?,
            location_id: r.location_id.map(LocationId::new),
        }))
    }

    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError> {
        let material_id = filter.material_id.as_ref().map(|m| m.as_str());
        let status = filter.status.map(|s| s.as_str());

        let row = sqlx::query!(
            r#"SELECT COALESCE(SUM((CAST(remaining_weight AS NUMERIC)/CAST(net_weight AS NUMERIC)) * price_paid), 0)::numeric AS value
               FROM spools
               WHERE status <> 'Archived'
                 AND ($1::text IS NULL OR material_id = $1)
                 AND ($2::text IS NULL OR status = $2)"#,
            material_id,
            status
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        let value = row.value.ok_or_else(|| {
            RepositoryError::Backend("stock_value query returned no value".into())
        })?;

        Money::from_decimal(value).map_err(|e| RepositoryError::Backend(e.to_string()))
    }
}
