use crate::manufacturers::model::{Manufacturer, NewManufacturer};
use crate::manufacturers::ports::api::ManufacturersUseCases;
use crate::manufacturers::ports::spi::{ManufacturerRepository, RepositoryError};
use crate::manufacturers::seed;
use crate::shared::{DomainError, ManufacturerId};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ManufacturersService {
    repo: Arc<dyn ManufacturerRepository>,
}

impl ManufacturersService {
    pub fn new(repo: Arc<dyn ManufacturerRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ManufacturersUseCases for ManufacturersService {
    async fn list(&self) -> Result<Vec<Manufacturer>, RepositoryError> {
        self.repo.list().await
    }

    async fn list_with_spool_counts(&self) -> Result<Vec<(Manufacturer, u64)>, RepositoryError> {
        let manufacturers = self.repo.list().await?;
        let mut out = Vec::with_capacity(manufacturers.len());
        for m in manufacturers {
            let count = self.repo.count_spools(&m.id).await?;
            out.push((m, count));
        }
        Ok(out)
    }

    async fn add(&self, m: NewManufacturer) -> Result<Manufacturer, RepositoryError> {
        self.repo.insert(m).await
    }

    async fn delete(&self, id: ManufacturerId) -> Result<(), RepositoryError> {
        let count = self.repo.count_spools(&id).await?;
        if count > 0 {
            return Err(RepositoryError::Domain(DomainError::ManufacturerInUse {
                count,
            }));
        }
        self.repo.delete(&id).await
    }

    async fn seed_defaults(&self) -> Result<(), RepositoryError> {
        for nm in seed::builtin() {
            if !self.repo.exists_by_name(nm.name.as_str()).await? {
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
    use crate::manufacturers::model::ManufacturerName;
    use crate::manufacturers::stubs::StubManufacturerRepository;
    use std::sync::Arc;

    fn svc() -> ManufacturersService {
        ManufacturersService::new(Arc::new(StubManufacturerRepository::new()))
    }

    fn new_manufacturer(name: &str) -> NewManufacturer {
        NewManufacturer {
            name: ManufacturerName::new(name.to_string()).unwrap(),
            country: None,
        }
    }

    #[tokio::test]
    async fn add_then_list_returns_the_new_manufacturer() {
        let s = svc();
        let created = s.add(new_manufacturer("Polymaker")).await.unwrap();
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, created.id);
        assert_eq!(all[0].name.as_str(), "Polymaker");
    }

    #[tokio::test]
    async fn delete_with_zero_spools_succeeds() {
        let s = svc();
        let created = s.add(new_manufacturer("Polymaker")).await.unwrap();
        s.delete(created.id.clone()).await.unwrap();
        assert!(s.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_with_spools_in_use_is_blocked() {
        let repo = Arc::new(StubManufacturerRepository::new());
        repo.set_spool_count(4);
        let s = ManufacturersService::new(repo);
        let created = s.add(new_manufacturer("Polymaker")).await.unwrap();
        let err = s.delete(created.id.clone()).await.unwrap_err();
        assert_eq!(
            err,
            RepositoryError::Domain(DomainError::ManufacturerInUse { count: 4 })
        );
        // refused delete leaves it present
        assert_eq!(s.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_with_spool_counts_pairs_each_manufacturer_with_its_count() {
        let repo = Arc::new(StubManufacturerRepository::new());
        repo.set_spool_count(2);
        let s = ManufacturersService::new(repo);
        let created = s.add(new_manufacturer("Polymaker")).await.unwrap();
        let all = s.list_with_spool_counts().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0.id, created.id);
        assert_eq!(all[0].1, 2);
    }

    #[tokio::test]
    async fn seed_defaults_inserts_all_builtins() {
        let s = svc();
        s.seed_defaults().await.unwrap();
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), seed::builtin().len());
        assert!(all.iter().any(|m| m.name.as_str() == "Prusament"));
        assert!(all.iter().any(|m| m.name.as_str() == "Polymaker"));
    }

    #[tokio::test]
    async fn seed_defaults_is_idempotent() {
        let s = svc();
        s.seed_defaults().await.unwrap();
        s.seed_defaults().await.unwrap(); // second run must not duplicate
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), seed::builtin().len());
    }
}
