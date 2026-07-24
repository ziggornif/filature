//! SQLx adapter for the spools slice SPI (`SpoolRepository`). Mirrors the
//! materials adapter: rows are mapped manually, domain newtypes are
//! reconstructed on read and unwrapped on write (ADR-0003, PostgreSQL).
//! `list`/`get` join `materials` to populate the display-only
//! `material_name`/`density` fields, and LEFT JOIN `locations` to populate
//! the display-only `location_name` field, carried by the shared read
//! models.

use crate::persistence::Db;
use async_trait::async_trait;
use domain::shared::{Grams, LocationId, ManufacturerId, MaterialId, Money};
use domain::spools::{
    Colour, Diameter, NewSpool, ReconcilableSpool, RepositoryError, Spool, SpoolDetail,
    SpoolFilter, SpoolId, SpoolListItem, SpoolRepository, SpoolSort, SpoolStatus, SpoolType,
};
use rust_decimal::Decimal;
use time::Date;
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
fn write_error(
    e: sqlx::Error,
    material_id: &str,
    location_id: Option<&str>,
    manufacturer_id: Option<&str>,
) -> RepositoryError {
    if let sqlx::Error::Database(db) = &e
        && let Some(constraint) = db.constraint()
    {
        if constraint.contains("location_id") {
            return RepositoryError::UnknownLocation(LocationId::new(
                location_id.unwrap_or_default().to_string(),
            ));
        }
        if constraint.contains("manufacturer_id") {
            return RepositoryError::UnknownManufacturer(ManufacturerId::new(
                manufacturer_id.unwrap_or_default().to_string(),
            ));
        }
        if constraint.contains("material_id") {
            return RepositoryError::UnknownMaterial(MaterialId::new(material_id.to_string()));
        }
    }
    RepositoryError::Backend(e.to_string())
}

fn build_colour(hex: Option<String>) -> Result<Option<Colour>, RepositoryError> {
    hex.map(Colour::from_hex)
        .transpose()
        .map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_diameter(s: &str) -> Result<Diameter, RepositoryError> {
    Diameter::parse(s).map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_status(s: &str) -> Result<SpoolStatus, RepositoryError> {
    SpoolStatus::parse(s).map_err(|e| RepositoryError::Backend(e.to_string()))
}

fn build_spool_type(s: &str) -> Result<SpoolType, RepositoryError> {
    SpoolType::parse(s).map_err(|e| RepositoryError::Backend(e.to_string()))
}

#[derive(sqlx::FromRow)]
struct ListRow {
    id: String,
    colour_hex: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    status: String,
    material_name: String,
    density: f64,
    location_name: Option<String>,
    manufacturer_name: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DetailRow {
    id: String,
    material_id: String,
    spool_type: String,
    colour_hex: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    price_paid: Decimal,
    status: String,
    location_id: Option<String>,
    manufacturer_id: Option<String>,
    material_name: String,
    density: f64,
    location_name: Option<String>,
    manufacturer_name: Option<String>,
    notes: Option<String>,
    purchased_at: Option<Date>,
    opened_at: Option<Date>,
}

#[derive(sqlx::FromRow)]
struct SpoolRow {
    id: String,
    material_id: String,
    spool_type: String,
    colour_hex: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    price_paid: Decimal,
    status: String,
    location_id: Option<String>,
    manufacturer_id: Option<String>,
    notes: Option<String>,
    purchased_at: Option<Date>,
    opened_at: Option<Date>,
    ams_tag_uid: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReconcilableRow {
    id: String,
    material_name: String,
    colour_hex: Option<String>,
    ams_tag_uid: Option<String>,
    status: String,
    remaining_percent: f64,
    loaded: bool,
}

/// Escape the LIKE/ILIKE metacharacters (`\`, `%`, `_`) in a user-supplied
/// search term so they match literally instead of acting as wildcards.
/// Postgres uses `\` as the default escape character, so no explicit `ESCAPE`
/// clause is needed.
fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn build_grams(v: f64) -> Result<Grams, RepositoryError> {
    Grams::new(v).map_err(|e| RepositoryError::Backend(e.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn to_list_item(
    id: String,
    material_name: String,
    colour_hex: Option<String>,
    diameter: String,
    net_weight: f64,
    remaining_weight: f64,
    status: String,
    density: f64,
    location_name: Option<String>,
    manufacturer_name: Option<String>,
) -> Result<SpoolListItem, RepositoryError> {
    Ok(SpoolListItem {
        id: SpoolId::new(id),
        material_name,
        colour: build_colour(colour_hex)?,
        diameter: build_diameter(&diameter)?,
        remaining_weight: build_grams(remaining_weight)?,
        net_weight: build_grams(net_weight)?,
        status: build_status(&status)?,
        density,
        location_name,
        manufacturer_name,
    })
}

#[async_trait]
impl SpoolRepository for SqlxSpoolRepository {
    async fn insert(&self, s: NewSpool) -> Result<Spool, RepositoryError> {
        let id = Ulid::new().to_string();
        let location_id = s.location_id.as_ref().map(|l| l.as_str());
        let manufacturer_id = s.manufacturer_id.as_ref().map(|m| m.as_str());
        let colour_hex = s.colour.as_ref().map(Colour::hex);
        let colour_name = s.colour.as_ref().and_then(Colour::name);
        let remaining_weight = s.initial_remaining_weight();
        let status = s.initial_status();
        let spool_type = s.spool_type();
        sqlx::query(
            r#"INSERT INTO spools
               (id, material_id, spool_type, colour_hex, colour_name, diameter, net_weight,
                remaining_weight, price_paid, status, location_id, manufacturer_id, notes,
                purchased_at, opened_at, ams_tag_uid)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
        )
        .bind(&id)
        .bind(s.material_id.as_str())
        .bind(spool_type.as_str())
        .bind(colour_hex)
        .bind(colour_name)
        .bind(s.diameter.as_str())
        .bind(s.net_weight.value())
        .bind(remaining_weight.value())
        .bind(s.price_paid.value())
        .bind(status.as_str())
        .bind(location_id)
        .bind(manufacturer_id)
        .bind(s.notes.as_deref())
        .bind(s.purchased_at)
        .bind(s.opened_at)
        .bind(s.ams_tag_uid.as_deref())
        .execute(&self.pool)
        .await
        .map_err(|e| write_error(e, s.material_id.as_str(), location_id, manufacturer_id))?;

        Ok(Spool {
            id: SpoolId::new(id),
            material_id: s.material_id,
            spool_type,
            colour: s.colour,
            diameter: s.diameter,
            net_weight: s.net_weight,
            remaining_weight,
            price_paid: s.price_paid,
            status,
            location_id: s.location_id,
            manufacturer_id: s.manufacturer_id,
            notes: s.notes,
            purchased_at: s.purchased_at,
            opened_at: s.opened_at,
            ams_tag_uid: s.ams_tag_uid,
        })
    }

    async fn update(&self, s: Spool) -> Result<Spool, RepositoryError> {
        let location_id = s.location_id.as_ref().map(|l| l.as_str());
        let manufacturer_id = s.manufacturer_id.as_ref().map(|m| m.as_str());
        let colour_hex = s.colour.as_ref().map(Colour::hex);
        let colour_name = s.colour.as_ref().and_then(Colour::name);
        let result = sqlx::query(
            r#"UPDATE spools SET
                 material_id=$2, spool_type=$3, colour_hex=$4, colour_name=$5, diameter=$6,
                 net_weight=$7, remaining_weight=$8, price_paid=$9, status=$10, location_id=$11,
                 manufacturer_id=$12, notes=$13, purchased_at=$14, opened_at=$15,
                 ams_tag_uid=$16
               WHERE id=$1"#,
        )
        .bind(s.id.as_str())
        .bind(s.material_id.as_str())
        .bind(s.spool_type.as_str())
        .bind(colour_hex)
        .bind(colour_name)
        .bind(s.diameter.as_str())
        .bind(s.net_weight.value())
        .bind(s.remaining_weight.value())
        .bind(s.price_paid.value())
        .bind(s.status.as_str())
        .bind(location_id)
        .bind(manufacturer_id)
        .bind(s.notes.as_deref())
        .bind(s.purchased_at)
        .bind(s.opened_at)
        .bind(s.ams_tag_uid.as_deref())
        .execute(&self.pool)
        .await
        .map_err(|e| write_error(e, s.material_id.as_str(), location_id, manufacturer_id))?;
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
        let manufacturer_id = filter.manufacturer_id.as_ref().map(|m| m.as_str());
        let location_id = filter.location_id.as_ref().map(|l| l.as_str());
        let search = filter
            .search
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(escape_like);

        let mut query = sqlx::QueryBuilder::new(
            r#"SELECT s.id, s.colour_hex, s.diameter, s.net_weight, s.remaining_weight,
                      s.status, m.name AS material_name, m.density, l.name AS location_name,
                      mf.name AS manufacturer_name
               FROM spools s
               JOIN materials m ON m.id = s.material_id
               LEFT JOIN locations l ON l.id = s.location_id
               LEFT JOIN manufacturers mf ON mf.id = s.manufacturer_id
               WHERE 1 = 1"#,
        );
        if let Some(value) = material_id {
            query.push(" AND s.material_id = ").push_bind(value);
        }
        if let Some(value) = status {
            query.push(" AND s.status = ").push_bind(value);
        } else {
            query.push(" AND s.status <> 'Archived'");
        }
        if let Some(value) = manufacturer_id {
            query.push(" AND s.manufacturer_id = ").push_bind(value);
        }
        if let Some(value) = location_id {
            query.push(" AND s.location_id = ").push_bind(value);
        }
        if let Some(value) = search {
            let pattern = format!("%{value}%");
            query
                .push(" AND (mf.name ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR s.colour_name ILIKE ")
                .push_bind(pattern)
                .push(")");
        }
        query.push(" ORDER BY ");
        query.push(match sort {
            SpoolSort::CreatedDesc => "s.created_at DESC",
            SpoolSort::RemainingRatioAsc => "(s.remaining_weight / s.net_weight) ASC",
            SpoolSort::RemainingRatioDesc => "(s.remaining_weight / s.net_weight) DESC",
        });
        let rows = query
            .build_query_as::<ListRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| backend(e, ""))?;
        let items = rows
            .into_iter()
            .map(|r| {
                to_list_item(
                    r.id,
                    r.material_name,
                    r.colour_hex,
                    r.diameter,
                    r.net_weight,
                    r.remaining_weight,
                    r.status,
                    r.density,
                    r.location_name,
                    r.manufacturer_name,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    async fn get(&self, id: &SpoolId) -> Result<Option<SpoolDetail>, RepositoryError> {
        let row = sqlx::query_as::<_, DetailRow>(
            r#"SELECT s.id, s.material_id, s.spool_type, s.colour_hex, s.diameter,
                      s.net_weight, s.remaining_weight, s.price_paid, s.status,
                      s.location_id, s.manufacturer_id,
                      m.name AS material_name, m.density,
                      l.name AS location_name,
                      mf.name AS manufacturer_name,
                      s.notes, s.purchased_at, s.opened_at
               FROM spools s
               JOIN materials m ON m.id = s.material_id
               LEFT JOIN locations l ON l.id = s.location_id
               LEFT JOIN manufacturers mf ON mf.id = s.manufacturer_id
               WHERE s.id = $1"#,
        )
        .bind(id.as_str())
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
            spool_type: build_spool_type(&r.spool_type)?,
            colour: build_colour(r.colour_hex)?,
            diameter: build_diameter(&r.diameter)?,
            net_weight: build_grams(r.net_weight)?,
            remaining_weight: build_grams(r.remaining_weight)?,
            price_paid: Money::from_decimal(r.price_paid)
                .map_err(|e| RepositoryError::Backend(e.to_string()))?,
            status: build_status(&r.status)?,
            density: r.density,
            location_name: r.location_name,
            location_id: r.location_id,
            manufacturer_name: r.manufacturer_name,
            manufacturer_id: r.manufacturer_id,
            notes: r.notes,
            purchased_at: r.purchased_at,
            opened_at: r.opened_at,
        }))
    }

    async fn find(&self, id: &SpoolId) -> Result<Option<Spool>, RepositoryError> {
        let row = sqlx::query_as::<_, SpoolRow>(
            r#"SELECT id, material_id, spool_type, colour_hex, diameter,
                      net_weight, remaining_weight, price_paid, status, location_id,
                      manufacturer_id, notes, purchased_at, opened_at, ams_tag_uid
               FROM spools WHERE id = $1"#,
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        let Some(r) = row else {
            return Ok(None);
        };

        Ok(Some(Spool {
            id: SpoolId::new(r.id),
            material_id: MaterialId::new(r.material_id),
            spool_type: build_spool_type(&r.spool_type)?,
            colour: build_colour(r.colour_hex)?,
            diameter: build_diameter(&r.diameter)?,
            net_weight: build_grams(r.net_weight)?,
            remaining_weight: build_grams(r.remaining_weight)?,
            price_paid: Money::from_decimal(r.price_paid)
                .map_err(|e| RepositoryError::Backend(e.to_string()))?,
            status: build_status(&r.status)?,
            location_id: r.location_id.map(LocationId::new),
            manufacturer_id: r.manufacturer_id.map(ManufacturerId::new),
            notes: r.notes,
            purchased_at: r.purchased_at,
            opened_at: r.opened_at,
            ams_tag_uid: r.ams_tag_uid,
        }))
    }

    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError> {
        let material_id = filter.material_id.as_ref().map(|m| m.as_str());
        let status = filter.status.map(|s| s.as_str());
        let manufacturer_id = filter.manufacturer_id.as_ref().map(|m| m.as_str());
        let location_id = filter.location_id.as_ref().map(|l| l.as_str());
        let search = filter
            .search
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(escape_like);

        // Same filter predicates as `list` so the Stock Value stat always
        // reflects the currently-visible, filtered set. The manufacturer join
        // exists only so `search` can match the manufacturer name.
        let row = sqlx::query!(
            r#"SELECT COALESCE(SUM((CAST(s.remaining_weight AS NUMERIC)/CAST(s.net_weight AS NUMERIC)) * s.price_paid), 0)::numeric AS value
               FROM spools s
               LEFT JOIN manufacturers mf ON mf.id = s.manufacturer_id
               WHERE s.status <> 'Archived'
                 AND ($1::text IS NULL OR s.material_id = $1)
                 AND ($2::text IS NULL OR s.status = $2)
                 AND ($3::text IS NULL OR s.manufacturer_id = $3)
                 AND ($4::text IS NULL OR s.location_id = $4)
                 AND ($5::text IS NULL
                      OR mf.name ILIKE '%' || $5 || '%'
                      OR s.colour_name ILIKE '%' || $5 || '%')"#,
            material_id,
            status,
            manufacturer_id,
            location_id,
            search
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| backend(e, ""))?;

        let value = row.value.ok_or_else(|| {
            RepositoryError::Backend("stock_value query returned no value".into())
        })?;

        Money::from_decimal(value).map_err(|e| RepositoryError::Backend(e.to_string()))
    }

    async fn count(&self, filter: SpoolFilter) -> Result<u64, RepositoryError> {
        let material_id = filter.material_id.as_ref().map(|m| m.as_str());
        let status = filter.status.map(|s| s.as_str());
        let manufacturer_id = filter.manufacturer_id.as_ref().map(|m| m.as_str());
        let location_id = filter.location_id.as_ref().map(|l| l.as_str());
        let search = filter
            .search
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(escape_like);

        let mut query = sqlx::QueryBuilder::new(
            r#"SELECT COUNT(*)
               FROM spools s
               LEFT JOIN manufacturers mf ON mf.id = s.manufacturer_id
               WHERE s.status <> 'Archived'"#,
        );
        if let Some(value) = material_id {
            query.push(" AND s.material_id = ").push_bind(value);
        }
        if let Some(value) = status {
            query.push(" AND s.status = ").push_bind(value);
        }
        if let Some(value) = manufacturer_id {
            query.push(" AND s.manufacturer_id = ").push_bind(value);
        }
        if let Some(value) = location_id {
            query.push(" AND s.location_id = ").push_bind(value);
        }
        if let Some(value) = search {
            let pattern = format!("%{value}%");
            query
                .push(" AND (mf.name ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR s.colour_name ILIKE ")
                .push_bind(pattern)
                .push(")");
        }

        let count = query
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(|e| backend(e, ""))?;
        u64::try_from(count)
            .map_err(|_| RepositoryError::Backend("count query returned a negative value".into()))
    }

    async fn reconcilable(&self) -> Result<Vec<ReconcilableSpool>, RepositoryError> {
        let rows = sqlx::query_as::<_, ReconcilableRow>(
            r#"SELECT s.id, m.name AS material_name, s.colour_hex, s.ams_tag_uid, s.status,
                      (s.remaining_weight / s.net_weight * 100.0) AS remaining_percent,
                      EXISTS (SELECT 1 FROM printer_slots ps WHERE ps.spool_id = s.id) AS loaded
               FROM spools s
               JOIN materials m ON m.id = s.material_id
               WHERE s.status IN ('Sealed', 'Open')
               ORDER BY s.created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| backend(error, ""))?;
        rows.into_iter()
            .map(|row| {
                Ok(ReconcilableSpool {
                    id: SpoolId::new(row.id),
                    material_name: row.material_name,
                    colour_hex: row.colour_hex,
                    ams_tag_uid: row.ams_tag_uid,
                    status: build_status(&row.status)?,
                    remaining_percent: row.remaining_percent.round().clamp(0.0, 100.0) as u8,
                    loaded: row.loaded,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn escape_like_neutralises_wildcards() {
        assert_eq!(escape_like("50%"), "50\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("c\\d"), "c\\\\d");
        assert_eq!(escape_like("plain"), "plain"); // untouched
    }
}
