use crate::shared::Money;
use crate::spools::model::{NewSpool, Spool, SpoolId, SpoolStatus};
use crate::spools::ports::spi::{RepositoryError, SpoolFilter, SpoolRepository, SpoolSort};
use crate::spools::read_models::{SpoolDetail, SpoolListItem};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Mutex;

/// The stub has no materials table to join against, so list/detail rows
/// get a placeholder name and density. The real values come from the SQL
/// join in the persistence adapter (a later task); the placeholder is only
/// good enough to exercise filter/sort behaviour in use-case tests.
const STUB_MATERIAL_NAME: &str = "stub";
const STUB_DENSITY: f64 = 1.24;

pub struct StubSpoolRepository {
    rows: Mutex<Vec<Spool>>,
}

impl StubSpoolRepository {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
        }
    }
    pub fn with(spools: Vec<Spool>) -> Self {
        Self {
            rows: Mutex::new(spools),
        }
    }
}

impl Default for StubSpoolRepository {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a spool matches the material/status/manufacturer/location/search
/// facets of a filter (the archived-visibility rule is applied separately by
/// each caller). The stub has no manufacturer *name* to join, so `search`
/// only matches the colour name here; the real substring-match against the
/// manufacturer name lives in the SQL adapter.
fn matches_filter(s: &Spool, f: &SpoolFilter) -> bool {
    f.material_id.as_ref().is_none_or(|m| m == &s.material_id)
        && f.status.is_none_or(|st| st == s.status)
        && f.manufacturer_id
            .as_ref()
            .is_none_or(|m| s.manufacturer_id.as_ref() == Some(m))
        && f.location_id
            .as_ref()
            .is_none_or(|l| s.location_id.as_ref() == Some(l))
        && f.search.as_ref().is_none_or(|term| {
            let needle = term.to_lowercase();
            s.colour
                .as_ref()
                .and_then(|colour| colour.name())
                .is_some_and(|n| n.to_lowercase().contains(&needle))
        })
}

fn to_list_item(s: &Spool) -> SpoolListItem {
    SpoolListItem {
        id: s.id.clone(),
        material_name: STUB_MATERIAL_NAME.to_string(),
        colour: s.colour.clone(),
        diameter: s.diameter,
        remaining_weight: s.remaining_weight,
        net_weight: s.net_weight,
        status: s.status,
        density: STUB_DENSITY,
        // The stub has no locations table to join against; the real
        // location_name comes from the SQL join in the persistence
        // adapter (Task 8). None is enough to exercise use-case behaviour.
        location_name: None,
        manufacturer_name: None,
    }
}

fn to_detail(s: &Spool) -> SpoolDetail {
    SpoolDetail {
        id: s.id.clone(),
        material_id: s.material_id.clone(),
        material_name: STUB_MATERIAL_NAME.to_string(),
        spool_type: s.spool_type,
        colour: s.colour.clone(),
        diameter: s.diameter,
        net_weight: s.net_weight,
        remaining_weight: s.remaining_weight,
        price_paid: s.price_paid,
        status: s.status,
        density: STUB_DENSITY,
        // Task 8 will populate `location_name` from the SQL join; the stub
        // has no locations table so it stays None. `location_id`, however,
        // is known straight from the stored `Spool` (no join needed).
        location_name: None,
        location_id: s.location_id.as_ref().map(|l| l.as_str().to_string()),
        manufacturer_name: None,
        manufacturer_id: s.manufacturer_id.as_ref().map(|m| m.as_str().to_string()),
    }
}

#[async_trait]
impl SpoolRepository for StubSpoolRepository {
    async fn insert(&self, s: NewSpool) -> Result<Spool, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let remaining_weight = s.initial_remaining_weight();
        let status = s.initial_status();
        let spool_type = s.spool_type();
        let spool = Spool {
            id: SpoolId::new(format!("stub-{}", rows.len())),
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
        };
        rows.push(spool.clone());
        Ok(spool)
    }

    async fn update(&self, s: Spool) -> Result<Spool, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        match rows.iter_mut().find(|r| r.id == s.id) {
            Some(slot) => {
                *slot = s.clone();
                Ok(s)
            }
            None => Err(RepositoryError::NotFound(s.id)),
        }
    }

    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError> {
        let rows = self.rows.lock().unwrap();
        let mut items: Vec<SpoolListItem> = rows
            .iter()
            .filter(|r| matches_filter(r, &filter))
            .filter(|r| {
                filter.status == Some(SpoolStatus::Archived) || r.status != SpoolStatus::Archived
            })
            .map(to_list_item)
            .collect();
        match sort {
            SpoolSort::CreatedDesc => items.reverse(),
            SpoolSort::RemainingRatioAsc => items.sort_by(|a, b| {
                a.remaining_ratio()
                    .partial_cmp(&b.remaining_ratio())
                    .unwrap()
            }),
            SpoolSort::RemainingRatioDesc => items.sort_by(|a, b| {
                b.remaining_ratio()
                    .partial_cmp(&a.remaining_ratio())
                    .unwrap()
            }),
        }
        Ok(items)
    }

    async fn get(&self, id: &SpoolId) -> Result<Option<SpoolDetail>, RepositoryError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|r| &r.id == id)
            .map(to_detail))
    }

    async fn find(&self, id: &SpoolId) -> Result<Option<Spool>, RepositoryError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|r| &r.id == id)
            .cloned())
    }

    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError> {
        let rows = self.rows.lock().unwrap();
        let sum: Decimal = rows
            .iter()
            .filter(|r| matches_filter(r, &filter))
            .filter(|r| r.status != SpoolStatus::Archived)
            .map(|r| {
                let net = r.net_weight.value();
                let ratio = if net <= 0.0 {
                    0.0
                } else {
                    r.remaining_weight.value() / net
                };
                Decimal::try_from(ratio).unwrap() * r.price_paid.value()
            })
            .fold(Decimal::ZERO, |acc, v| acc + v);
        Money::from_decimal(sum).map_err(RepositoryError::Domain)
    }
}
