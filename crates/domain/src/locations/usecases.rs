use crate::locations::model::{Location, NewLocation};
use crate::locations::ports::api::LocationsUseCases;
use crate::locations::ports::spi::{LocationRepository, RepositoryError};
use crate::shared::{DomainError, LocationId};
use async_trait::async_trait;
use std::sync::Arc;

pub struct LocationsService {
    repo: Arc<dyn LocationRepository>,
}

impl LocationsService {
    pub fn new(repo: Arc<dyn LocationRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl LocationsUseCases for LocationsService {
    async fn list(&self) -> Result<Vec<Location>, RepositoryError> {
        self.repo.list().await
    }
    async fn list_with_spool_counts(&self) -> Result<Vec<(Location, u64)>, RepositoryError> {
        let locations = self.repo.list().await?;
        let mut out = Vec::with_capacity(locations.len());
        for location in locations {
            let count = self.repo.count_spools(&location.id).await?;
            out.push((location, count));
        }
        Ok(out)
    }
    async fn add(&self, l: NewLocation) -> Result<Location, RepositoryError> {
        self.repo.insert(l).await
    }
    async fn edit(&self, l: Location) -> Result<Location, RepositoryError> {
        self.repo.update(l).await
    }
    async fn delete(&self, id: LocationId) -> Result<(), RepositoryError> {
        let count = self.repo.count_spools(&id).await?;
        if count > 0 {
            return Err(RepositoryError::Domain(DomainError::LocationInUse {
                count,
            }));
        }
        self.repo.delete(&id).await
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::locations::model::LocationName;
    use crate::locations::stubs::StubLocationRepository;
    use std::sync::Arc;

    fn svc() -> LocationsService {
        LocationsService::new(Arc::new(StubLocationRepository::new()))
    }

    fn new_location(name: &str) -> NewLocation {
        NewLocation {
            name: LocationName::new(name.to_string()).unwrap(),
            note: None,
        }
    }

    #[tokio::test]
    async fn add_then_list_returns_the_new_location() {
        let s = svc();
        let created = s.add(new_location("Shelf A")).await.unwrap();
        let all = s.list().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, created.id);
        assert_eq!(all[0].name.as_str(), "Shelf A");
    }

    #[tokio::test]
    async fn edit_unknown_location_returns_not_found() {
        let s = svc();
        let unknown = Location {
            id: LocationId::new("does-not-exist"),
            name: LocationName::new("Ghost").unwrap(),
            note: None,
        };
        assert!(matches!(
            s.edit(unknown).await,
            Err(RepositoryError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn delete_with_zero_spools_succeeds_and_removes_location() {
        let s = svc();
        let created = s.add(new_location("Shelf A")).await.unwrap();
        s.delete(created.id.clone()).await.unwrap();
        let all = s.list().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn list_with_spool_counts_pairs_each_location_with_its_count() {
        let repo = Arc::new(StubLocationRepository::new());
        repo.set_spool_count(2);
        let s = LocationsService::new(repo);
        let created = s.add(new_location("Shelf A")).await.unwrap();

        let all = s.list_with_spool_counts().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0.id, created.id);
        assert_eq!(all[0].1, 2);
    }

    #[tokio::test]
    async fn list_with_spool_counts_is_empty_when_no_locations() {
        let s = svc();
        assert!(s.list_with_spool_counts().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_with_spools_in_use_returns_location_in_use() {
        let repo = Arc::new(StubLocationRepository::new());
        repo.set_spool_count(3);
        let s = LocationsService::new(repo);
        let created = s.add(new_location("Shelf A")).await.unwrap();
        let err = s.delete(created.id.clone()).await.unwrap_err();
        assert_eq!(
            err,
            RepositoryError::Domain(DomainError::LocationInUse { count: 3 })
        );
        // still present since delete was refused
        assert_eq!(s.list().await.unwrap().len(), 1);
    }
}
