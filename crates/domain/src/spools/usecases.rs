use crate::shared::{Grams, LocationId, Money};
use crate::spools::model::{EditSpool, NewSpool, Spool, SpoolId};
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

    async fn edit(&self, edit: EditSpool) -> Result<Spool, RepositoryError> {
        let remaining_weight = edit.derived_remaining_weight();
        let status = edit.status();
        let mut spool = self
            .repo
            .find(&edit.id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound(edit.id.clone()))?;
        spool.material_id = edit.material_id;
        spool.colour = edit.colour;
        spool.diameter = edit.diameter;
        spool.net_weight = edit.net_weight;
        spool.remaining_weight = remaining_weight;
        spool.price_paid = edit.price_paid;
        spool.status = status;
        spool.location_id = edit.location_id;
        spool.manufacturer_id = edit.manufacturer_id;
        self.repo.update(spool).await
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

    async fn set_remaining(&self, id: SpoolId, remaining: Grams) -> Result<Spool, RepositoryError> {
        let mut s = self
            .repo
            .find(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        s.set_remaining(remaining)?;
        self.repo.update(s).await
    }

    async fn consume(&self, id: SpoolId, amount: Grams) -> Result<Spool, RepositoryError> {
        let mut s = self
            .repo
            .find(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        s.consume(amount)?;
        self.repo.update(s).await
    }

    async fn archive(&self, id: SpoolId) -> Result<Spool, RepositoryError> {
        let mut s = self
            .repo
            .find(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        s.archive()?;
        self.repo.update(s).await
    }

    async fn restore(&self, id: SpoolId) -> Result<Spool, RepositoryError> {
        let mut s = self
            .repo
            .find(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        s.restore()?;
        self.repo.update(s).await
    }

    async fn assign_location(
        &self,
        id: SpoolId,
        location: Option<LocationId>,
    ) -> Result<Spool, RepositoryError> {
        let mut s = self
            .repo
            .find(&id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        s.assign_location(location);
        self.repo.update(s).await
    }

    async fn stock_value(&self, filter: SpoolFilter) -> Result<Money, RepositoryError> {
        self.repo.stock_value(filter).await
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::shared::{DomainError, Grams, LocationId, MaterialId, Money};
    use crate::spools::model::{
        Colour, Diameter, EditSpool, SpoolCondition, SpoolStatus, SpoolType,
    };
    use crate::spools::stubs::StubSpoolRepository;

    fn svc() -> SpoolsService {
        SpoolsService::new(Arc::new(StubSpoolRepository::new()))
    }

    fn sample_new_spool(material_id: &str) -> NewSpool {
        NewSpool {
            condition: SpoolCondition::New,
            material_id: MaterialId::new(material_id),
            colour: Some(Colour::from_hex("#1A9E4B".into()).unwrap()),
            diameter: Diameter::Mm1_75,
            net_weight: Grams::new(1000.0).unwrap(),
            price_paid: Money::new(2500, 2).unwrap(),
            location_id: None,
            manufacturer_id: None,
            remaining_weight: None,
        }
    }

    fn edit_spool(spool: &Spool, condition: SpoolCondition) -> EditSpool {
        EditSpool {
            id: spool.id.clone(),
            condition,
            material_id: spool.material_id.clone(),
            colour: spool.colour.clone(),
            diameter: spool.diameter,
            net_weight: spool.net_weight,
            price_paid: spool.price_paid,
            location_id: spool.location_id.clone(),
            manufacturer_id: spool.manufacturer_id.clone(),
            remaining_weight: Some(spool.remaining_weight),
        }
    }

    #[tokio::test]
    async fn add_persists_sealed_and_full() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        assert_eq!(created.status, SpoolStatus::Sealed);
        assert_eq!(created.spool_type, SpoolType::Complete);
        assert_eq!(created.remaining_weight.value(), created.net_weight.value());
        assert_eq!(created.net_weight.value(), 1000.0);
    }

    #[tokio::test]
    async fn edit_opened_clamps_entered_remaining_and_derives_status() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        let mut edit = edit_spool(&created, SpoolCondition::Opened);
        edit.net_weight = Grams::new(500.0).unwrap();
        edit.remaining_weight = Some(Grams::new(800.0).unwrap());
        let edited = s.edit(edit).await.unwrap();
        assert_eq!(edited.remaining_weight.value(), 500.0);
        assert_eq!(edited.net_weight.value(), 500.0);
        assert_eq!(edited.status, SpoolStatus::Open);
    }

    #[tokio::test]
    async fn edit_new_resets_remaining_to_net_and_derives_sealed_status() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        s.set_remaining(created.id.clone(), Grams::new(250.0).unwrap())
            .await
            .unwrap();
        let edited = s
            .edit(edit_spool(&created, SpoolCondition::New))
            .await
            .unwrap();
        assert_eq!(edited.remaining_weight.value(), 1000.0);
        assert_eq!(edited.status, SpoolStatus::Sealed);
        assert_eq!(edited.spool_type, SpoolType::Complete);
    }

    #[tokio::test]
    async fn edit_preserves_existing_physical_spool_type() {
        let s = svc();
        let mut new = sample_new_spool("material-1");
        new.condition = SpoolCondition::Refill;
        let created = s.add(new).await.unwrap();
        let edited = s
            .edit(edit_spool(&created, SpoolCondition::New))
            .await
            .unwrap();
        assert_eq!(edited.spool_type, SpoolType::Recharge);
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
                    ..Default::default()
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
        s.set_remaining(created.id, Grams::new(900.0).unwrap())
            .await
            .unwrap();
        s.add(sample_new_spool("material-1")).await.unwrap(); // stays Sealed

        let sealed_only = s
            .list(
                SpoolFilter {
                    material_id: None,
                    status: Some(SpoolStatus::Sealed),
                    ..Default::default()
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
        let low = s.add(sample_new_spool("material-1")).await.unwrap();
        s.set_remaining(low.id.clone(), Grams::new(100.0).unwrap())
            .await
            .unwrap();

        let high = s.add(sample_new_spool("material-1")).await.unwrap();
        s.set_remaining(high.id.clone(), Grams::new(900.0).unwrap())
            .await
            .unwrap();

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

    #[tokio::test]
    async fn consume_drives_sealed_to_open_to_empty() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        assert_eq!(created.status, SpoolStatus::Sealed);

        s.consume(created.id.clone(), Grams::new(300.0).unwrap())
            .await
            .unwrap();
        let after_partial = s.view(created.id.clone()).await.unwrap();
        assert_eq!(after_partial.remaining_weight.value(), 700.0);
        assert_eq!(after_partial.status, SpoolStatus::Open);

        s.consume(created.id.clone(), Grams::new(700.0).unwrap())
            .await
            .unwrap();
        let after_full = s.view(created.id.clone()).await.unwrap();
        assert_eq!(after_full.remaining_weight.value(), 0.0);
        assert_eq!(after_full.status, SpoolStatus::Empty);
    }

    #[tokio::test]
    async fn set_remaining_above_net_is_rejected() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        let err = s
            .set_remaining(created.id.clone(), Grams::new(1001.0).unwrap())
            .await
            .unwrap_err();
        assert_eq!(err, RepositoryError::Domain(DomainError::RemainingAboveNet));
    }

    #[tokio::test]
    async fn set_remaining_on_unknown_id_is_not_found() {
        let s = svc();
        let err = s
            .set_remaining(SpoolId::new("does-not-exist"), Grams::new(10.0).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[tokio::test]
    async fn archive_omits_from_default_list_and_restore_derives_status() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        s.set_remaining(created.id.clone(), Grams::new(400.0).unwrap())
            .await
            .unwrap();
        s.add(sample_new_spool("material-1")).await.unwrap(); // stays Sealed

        let archived = s.archive(created.id.clone()).await.unwrap();
        assert_eq!(archived.status, SpoolStatus::Archived);

        let default_list = s
            .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
            .await
            .unwrap();
        assert!(default_list.iter().all(|i| i.id != created.id));
        assert_eq!(default_list.len(), 1);

        let archived_list = s
            .list(
                SpoolFilter {
                    material_id: None,
                    status: Some(SpoolStatus::Archived),
                    ..Default::default()
                },
                SpoolSort::CreatedDesc,
            )
            .await
            .unwrap();
        assert_eq!(archived_list.len(), 1);
        assert_eq!(archived_list[0].id, created.id);

        let restored = s.restore(created.id.clone()).await.unwrap();
        assert_eq!(restored.status, SpoolStatus::Open);
        assert_eq!(restored.remaining_weight.value(), 400.0);

        let default_list_after_restore = s
            .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
            .await
            .unwrap();
        assert_eq!(default_list_after_restore.len(), 2);
    }

    #[tokio::test]
    async fn stock_value_sums_remaining_ratio_times_price() {
        let s = svc();
        // Full spool: 1000g net, 2500 (25.00) price, remaining == net -> 25.00.
        let full = s.add(sample_new_spool("material-1")).await.unwrap();
        assert_eq!(full.status, SpoolStatus::Sealed);

        // Half-consumed spool: remaining 500/1000 -> 12.50.
        let half = s.add(sample_new_spool("material-1")).await.unwrap();
        s.set_remaining(half.id.clone(), Grams::new(500.0).unwrap())
            .await
            .unwrap();

        let value = s.stock_value(SpoolFilter::default()).await.unwrap();
        assert_eq!(value, Money::new(3750, 2).unwrap());
    }

    #[tokio::test]
    async fn assign_location_sets_location_on_the_spool() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        assert_eq!(created.location_id, None);

        let location = LocationId::new("warehouse-1");
        let assigned = s
            .assign_location(created.id.clone(), Some(location.clone()))
            .await
            .unwrap();
        assert_eq!(assigned.location_id, Some(location.clone()));

        let found = s.view(created.id.clone()).await.unwrap();
        assert_eq!(found.id, created.id);
    }

    #[tokio::test]
    async fn assign_location_none_unassigns() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        s.assign_location(created.id.clone(), Some(LocationId::new("warehouse-1")))
            .await
            .unwrap();

        let unassigned = s.assign_location(created.id.clone(), None).await.unwrap();
        assert_eq!(unassigned.location_id, None);
    }

    #[tokio::test]
    async fn assign_location_on_unknown_id_is_not_found() {
        let s = svc();
        let err = s
            .assign_location(
                SpoolId::new("does-not-exist"),
                Some(LocationId::new("warehouse-1")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[tokio::test]
    async fn view_reflects_assigned_location_id() {
        let s = svc();
        let created = s.add(sample_new_spool("material-1")).await.unwrap();
        let before = s.view(created.id.clone()).await.unwrap();
        assert_eq!(before.location_id, None);

        let location = LocationId::new("warehouse-1");
        s.assign_location(created.id.clone(), Some(location.clone()))
            .await
            .unwrap();

        let detail = s.view(created.id.clone()).await.unwrap();
        assert_eq!(detail.location_id, Some(location.as_str().to_string()));
    }
}
