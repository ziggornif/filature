mod support;

use domain::instance_configuration::{
    InstanceConfigurationRepository, InstanceConfigurationService, InstanceConfigurationUseCases,
};
use domain::shared::LowStockThreshold;
use filature::persistence::connect_and_migrate;
use filature::persistence::instance_configuration::SqlxInstanceConfigurationRepository;
use std::sync::Arc;

#[tokio::test]
async fn default_then_persisted_threshold_survives_service_recreation() {
    let url = support::postgres_url().await;
    let pool = connect_and_migrate(&url).await.unwrap();
    let repo: Arc<dyn InstanceConfigurationRepository> =
        Arc::new(SqlxInstanceConfigurationRepository::new(pool.clone()));
    let service = InstanceConfigurationService::new(repo);

    assert_eq!(
        service.get().await.unwrap().low_stock_threshold,
        LowStockThreshold::default()
    );
    service
        .update_low_stock_threshold(LowStockThreshold::new(31).unwrap())
        .await
        .unwrap();

    let reloaded = InstanceConfigurationService::new(Arc::new(
        SqlxInstanceConfigurationRepository::new(pool.clone()),
    ));
    assert_eq!(
        reloaded.get().await.unwrap().low_stock_threshold.percent(),
        31
    );

    let invalid = sqlx::query(
        "UPDATE instance_configuration SET low_stock_threshold = 101 WHERE singleton = TRUE",
    )
    .execute(&pool)
    .await;
    assert!(
        invalid.is_err(),
        "database constraint must reject 101 percent"
    );
}
