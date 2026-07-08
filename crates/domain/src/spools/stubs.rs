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
    }
}

fn to_detail(s: &Spool) -> SpoolDetail {
    SpoolDetail {
        id: s.id.clone(),
        material_id: s.material_id.clone(),
        material_name: STUB_MATERIAL_NAME.to_string(),
        colour: s.colour.clone(),
        diameter: s.diameter,
        net_weight: s.net_weight,
        remaining_weight: s.remaining_weight,
        price_paid: s.price_paid,
        status: s.status,
        density: STUB_DENSITY,
    }
}

#[async_trait]
impl SpoolRepository for StubSpoolRepository {
    async fn insert(&self, s: NewSpool) -> Result<Spool, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let spool = Spool {
            id: SpoolId::new(format!("stub-{}", rows.len())),
            material_id: s.material_id,
            colour: s.colour,
            diameter: s.diameter,
            net_weight: s.net_weight,
            remaining_weight: s.net_weight,
            price_paid: s.price_paid,
            status: SpoolStatus::Sealed,
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
            .filter(|r| {
                filter
                    .material_id
                    .as_ref()
                    .is_none_or(|mid| mid == &r.material_id)
            })
            .filter(|r| filter.status.is_none_or(|st| st == r.status))
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
            .filter(|r| {
                filter
                    .material_id
                    .as_ref()
                    .is_none_or(|mid| mid == &r.material_id)
            })
            .filter(|r| filter.status.is_none_or(|st| st == r.status))
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
