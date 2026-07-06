use crate::spools::model::{NewSpool, Spool, SpoolId};
use crate::spools::ports::api::SpoolsUseCases;
use crate::spools::ports::spi::{RepositoryError, SpoolFilter, SpoolRepository, SpoolSort};
use crate::spools::read_models::{SpoolDetail, SpoolListItem};
use async_trait::async_trait;
use std::sync::Arc;

pub struct SpoolsService {
    repo: Arc<dyn SpoolRepository>,
}

impl SpoolsService {
    pub fn new(repo: Arc<dyn SpoolRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl SpoolsUseCases for SpoolsService {
    async fn add(&self, s: NewSpool) -> Result<Spool, RepositoryError> {
        self.repo.insert(s).await
    }

    async fn edit(&self, mut s: Spool) -> Result<Spool, RepositoryError> {
        let new_net = s.net_weight;
        s.set_net_clamping(new_net);
        self.repo.update(s).await
    }

    async fn list(
        &self,
        filter: SpoolFilter,
        sort: SpoolSort,
    ) -> Result<Vec<SpoolListItem>, RepositoryError> {
        self.repo.list(filter, sort).await
    }

    async fn view(&self, id: SpoolId) -> Result<SpoolDetail, RepositoryError> {
        self.repo
            .get(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::shared::{Grams, MaterialId, Money};
    use crate::spools::model::{Colour, Diameter, SpoolStatus};
    use crate::spools::stubs::StubSpoolRepository;

    fn svc() -> SpoolsService {
        SpoolsService::new(Arc::new(StubSpoolRepository::new()))
    }

    fn sample_new_spool(material_id: &str) -> NewSpool {
        NewSpool {
            material_id: MaterialId::new(material_id),
            colour: Colour::new("#1A9E4B".into(), None).unwrap(),
            diameter: Diameter::Mm1_75,
            net_weight: Grams::new(1000.0).unwrap(),
            price_paid: Money::new(2500, 2),
        }
    }

    #[tokio::test]
    async fn add_persists_sealed_and_full() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        assert_eq!(created.status, SpoolStatus::Sealed);
        assert_eq!(created.remaining_weight.value(), created.net_weight.value());
        assert_eq!(created.net_weight.value(), 1000.0);
    }

    #[tokio::test]
    async fn edit_clamps_remaining_when_net_lowered_below_it() {
        let s = svc();
        let mut created = s.add(sample_new_spool("material-1")).await.unwrap();
        // Simulate remaining having been drawn down before this edit.
        created.remaining_weight = Grams::new(800.0).unwrap();
        s.edit(created.clone()).await.unwrap();
        created.net_weight = Grams::new(500.0).unwrap();
        let edited = s.edit(created).await.unwrap();
        assert_eq!(edited.remaining_weight.value(), 500.0);
        assert_eq!(edited.net_weight.value(), 500.0);
    }

    #[tokio::test]
    async fn view_unknown_id_returns_not_found() {
        let s = svc();
        let err = s.view(SpoolId::new("does-not-exist")).await.unwrap_err();
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[tokio::test]
    async fn view_known_id_returns_detail() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        let detail = s.view(created.id.clone()).await.unwrap();
        assert_eq!(detail.id, created.id);
        assert_eq!(detail.material_id, created.material_id);
    }

    #[tokio::test]
    async fn list_applies_material_filter() {
        let s = svc();
        s.add(sample_new_spool("material-1")).await.unwrap();
        s.add(sample_new_spool("material-2")).await.unwrap();
        let filtered = s
            .list(
                SpoolFilter {
                    material_id: Some(MaterialId::new("material-2")),
                    status: None,
                },
                SpoolSort::CreatedDesc,
            )
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].material_name, "stub");
    }

    #[tokio::test]
    async fn list_applies_status_filter() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        let mut opened = created.clone();
        opened.status = SpoolStatus::Open;
        s.edit(opened).await.unwrap();
        s.add(sample_new_spool("material-1")).await.unwrap(); // stays Sealed

        let sealed_only = s
            .list(
                SpoolFilter {
                    material_id: None,
                    status: Some(SpoolStatus::Sealed),
                },
                SpoolSort::CreatedDesc,
            )
            .await
            .unwrap();
        assert_eq!(sealed_only.len(), 1);
        assert_eq!(sealed_only[0].status, SpoolStatus::Sealed);
    }

    #[tokio::test]
    async fn list_applies_remaining_ratio_sort() {
        let s = svc();
        let mut low = s.add(sample_new_spool("material-1")).await.unwrap();
        low.remaining_weight = Grams::new(100.0).unwrap();
        s.edit(low.clone()).await.unwrap();

        let mut high = s.add(sample_new_spool("material-1")).await.unwrap();
        high.remaining_weight = Grams::new(900.0).unwrap();
        s.edit(high.clone()).await.unwrap();

        let asc = s
            .list(SpoolFilter::default(), SpoolSort::RemainingRatioAsc)
            .await
            .unwrap();
        assert_eq!(asc[0].id, low.id);
        assert_eq!(asc[1].id, high.id);

        let desc = s
            .list(SpoolFilter::default(), SpoolSort::RemainingRatioDesc)
            .await
            .unwrap();
        assert_eq!(desc[0].id, high.id);
        assert_eq!(desc[1].id, low.id);
    }

    #[tokio::test]
    async fn list_created_desc_returns_most_recent_first() {
        let s = svc();
        let first = s.add(sample_new_spool("material-1")).await.unwrap();
        let second = s.add(sample_new_spool("material-1")).await.unwrap();
        let third = s.add(sample_new_spool("material-1")).await.unwrap();

        let items = s
            .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
            .await
            .unwrap();

        assert_eq!(items[0].id, third.id);
        assert_eq!(items[1].id, second.id);
        assert_eq!(items[2].id, first.id);
    }
}
