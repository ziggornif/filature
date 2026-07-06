use crate::materials::model::{Material, NewMaterial};
use crate::materials::ports::api::MaterialsUseCases;
use crate::materials::ports::spi::{MaterialRepository, RepositoryError};
use crate::materials::seed;
use async_trait::async_trait;
use std::sync::Arc;

pub struct MaterialsService {
    repo: Arc<dyn MaterialRepository>,
}

impl MaterialsService {
    pub fn new(repo: Arc<dyn MaterialRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl MaterialsUseCases for MaterialsService {
    async fn list(&self) -> Result<Vec<Material>, RepositoryError> {
        self.repo.list().await
    }
    async fn add(&self, m: NewMaterial) -> Result<Material, RepositoryError> {
        self.repo.insert(m).await
    }
    async fn edit(&self, m: Material) -> Result<Material, RepositoryError> {
        self.repo.update(m).await
    }
    async fn seed_defaults(&self) -> Result<(), RepositoryError> {
        for nm in seed::builtin() {
            if !self.repo.exists_by_name(&nm.name).await? {
                match self.repo.insert(nm).await {
                    Ok(_) | Err(RepositoryError::Duplicate(_)) => {} // idempotent under races
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::materials::stubs::StubMaterialRepository;
    use std::sync::Arc;

    fn svc() -> MaterialsService {
        MaterialsService::new(Arc::new(StubMaterialRepository::new()))
    }

    #[tokio::test]
    async fn seed_defaults_inserts_all_builtins() {
        let s = svc();
        s.seed_defaults().await.unwrap();
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), crate::materials::seed::builtin().len());
        assert!(all.iter().any(|m| m.name == "PLA"));
        assert!(all.iter().any(|m| m.name == "PA-CF"));
    }

    #[tokio::test]
    async fn seed_defaults_is_idempotent() {
        let s = svc();
        s.seed_defaults().await.unwrap();
        s.seed_defaults().await.unwrap(); // second run must not duplicate
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), crate::materials::seed::builtin().len());
    }

    #[tokio::test]
    async fn add_rejects_duplicate_name() {
        let s = svc();
        let m = crate::materials::seed::builtin().remove(0); // PLA
        s.add(m.clone()).await.unwrap();
        assert!(matches!(
            s.add(m).await,
            Err(crate::materials::RepositoryError::Duplicate(_))
        ));
    }

    #[tokio::test]
    async fn edit_persists_changes() {
        let s = svc();
        let created = s
            .add(crate::materials::seed::builtin().remove(0))
            .await
            .unwrap();
        let mut changed = created.clone();
        changed.density = crate::materials::Density::new(1.30).unwrap();
        let out = s.edit(changed).await.unwrap();
        assert_eq!(out.density.value(), 1.30);
        assert_eq!(s.list().await.unwrap()[0].density.value(), 1.30);
    }
}
