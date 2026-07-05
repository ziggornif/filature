use crate::materials::model::{Material, MaterialId, NewMaterial};
use crate::materials::ports::spi::{MaterialRepository, RepositoryError};
use async_trait::async_trait;
use std::sync::Mutex;

pub struct StubMaterialRepository {
    rows: Mutex<Vec<Material>>,
}

impl StubMaterialRepository {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
        }
    }
    pub fn with(materials: Vec<Material>) -> Self {
        Self {
            rows: Mutex::new(materials),
        }
    }
}

impl Default for StubMaterialRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MaterialRepository for StubMaterialRepository {
    async fn list(&self) -> Result<Vec<Material>, RepositoryError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn insert(&self, m: NewMaterial) -> Result<Material, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        if rows.iter().any(|r| r.name == m.name) {
            return Err(RepositoryError::Duplicate(m.name));
        }
        let material = Material {
            id: MaterialId::new(format!("stub-{}", rows.len())),
            name: m.name,
            density: m.density,
            drying: m.drying,
            sensitivity: m.sensitivity,
            nozzle: m.nozzle,
            bed: m.bed,
        };
        rows.push(material.clone());
        Ok(material)
    }
    async fn update(&self, m: Material) -> Result<Material, RepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        match rows.iter_mut().find(|r| r.id == m.id) {
            Some(slot) => {
                *slot = m.clone();
                Ok(m)
            }
            None => Err(RepositoryError::Backend("no such id".into())),
        }
    }
    async fn exists_by_name(&self, name: &str) -> Result<bool, RepositoryError> {
        Ok(self.rows.lock().unwrap().iter().any(|r| r.name == name))
    }
}
