use crate::locations::model::{Location, NewLocation};
use crate::locations::ports::spi::{LocationRepository, RepositoryError};
use crate::shared::LocationId;
use async_trait::async_trait;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct StubLocationRepository {
    rows: Mutex<Vec<Location>>,
    spool_count: AtomicU64,
}

impl StubLocationRepository {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
            spool_count: AtomicU64::new(0),
        }
    }
    pub fn with(locations: Vec<Location>) -> Self {
        Self {
            rows: Mutex::new(locations),
            spool_count: AtomicU64::new(0),
        }
    }
    /// Test hook: sets the count returned by `count_spools` for any id.
    pub fn set_spool_count(&self, count: u64) {
        self.spool_count.store(count, Ordering::SeqCst);
    }
}

impl Default for StubLocationRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LocationRepository for StubLocationRepository {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn insert(&self, l: NewLocation) -> Result<Location, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let location = Location {
            id: LocationId::new(format!("stub-{}", rows.len())),
            name: l.name,
            note: l.note,
        };
        rows.push(location.clone());
        Ok(location)
    }
    async fn update(&self, l: Location) -> Result<Location, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        match rows.iter_mut().find(|r| r.id == l.id) {
            Some(slot) => {
                *slot = l.clone();
                Ok(l)
            }
            None => Err(RepositoryError::NotFound(l.id)),
        }
    }
    async fn delete(&self, id: &LocationId) -> Result<(), RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let len_before = rows.len();
        rows.retain(|r| r.id != *id);
        if rows.len() == len_before {
            return Err(RepositoryError::NotFound(id.clone()));
        }
        Ok(())
    }
    async fn count_spools(&self, _id: &LocationId) -> Result<u64, RepositoryError> {
        Ok(self.spool_count.load(Ordering::SeqCst))
    }
}
