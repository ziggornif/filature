use crate::manufacturers::model::{Manufacturer, NewManufacturer};
use crate::manufacturers::ports::spi::{ManufacturerRepository, RepositoryError};
use crate::shared::ManufacturerId;
use async_trait::async_trait;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct StubManufacturerRepository {
    rows: Mutex<Vec<Manufacturer>>,
    spool_count: AtomicU64,
}

impl StubManufacturerRepository {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
            spool_count: AtomicU64::new(0),
        }
    }
    pub fn with(manufacturers: Vec<Manufacturer>) -> Self {
        Self {
            rows: Mutex::new(manufacturers),
            spool_count: AtomicU64::new(0),
        }
    }
    /// Test hook: sets the count returned by `count_spools` for any id.
    pub fn set_spool_count(&self, count: u64) {
        self.spool_count.store(count, Ordering::SeqCst);
    }
}

impl Default for StubManufacturerRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ManufacturerRepository for StubManufacturerRepository {
    async fn list(&self) -> Result<Vec<Manufacturer>, RepositoryError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn insert(&self, m: NewManufacturer) -> Result<Manufacturer, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        if rows.iter().any(|r| r.name == m.name) {
            return Err(RepositoryError::Duplicate(m.name.as_str().to_string()));
        }
        let manufacturer = Manufacturer {
            id: ManufacturerId::new(format!("stub-{}", rows.len())),
            name: m.name,
            country: m.country,
        };
        rows.push(manufacturer.clone());
        Ok(manufacturer)
    }
    async fn delete(&self, id: &ManufacturerId) -> Result<(), RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let len_before = rows.len();
        rows.retain(|r| r.id != *id);
        if rows.len() == len_before {
            return Err(RepositoryError::NotFound(id.clone()));
        }
        Ok(())
    }
    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.name.as_str() == name))
    }
    async fn count_spools(&self, _id: &ManufacturerId) -> Result<u64, RepositoryError> {
        Ok(self.spool_count.load(Ordering::SeqCst))
    }
}
