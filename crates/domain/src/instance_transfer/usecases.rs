use super::{
    FORMAT, InstanceDocument, InstanceTransferRepository, InstanceTransferUseCases, TransferError,
    VERSION,
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct InstanceTransferService {
    repository: Arc<dyn InstanceTransferRepository>,
}

impl InstanceTransferService {
    pub fn new(repository: Arc<dyn InstanceTransferRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl InstanceTransferUseCases for InstanceTransferService {
    async fn export(&self) -> Result<InstanceDocument, TransferError> {
        let document = InstanceDocument {
            format: FORMAT.to_string(),
            version: VERSION,
            content: self.repository.export_snapshot().await?,
        };
        document.validate()?;
        Ok(document)
    }

    async fn import(&self, document: InstanceDocument) -> Result<(), TransferError> {
        document.validate()?;
        self.repository.replace_snapshot(document.content).await
    }
}

#[cfg(all(test, feature = "stubs"))]
mod tests {
    use super::*;
    use crate::instance_transfer::stubs::StubInstanceTransferRepository;
    use crate::instance_transfer::{InstanceSnapshot, SnapshotConfiguration};

    fn snapshot() -> InstanceSnapshot {
        InstanceSnapshot {
            materials: vec![],
            manufacturers: vec![],
            locations: vec![],
            spools: vec![],
            printers: vec![],
            configuration: SnapshotConfiguration {
                low_stock_threshold: 15,
            },
        }
    }

    #[tokio::test]
    async fn export_wraps_snapshot_in_current_format() {
        let service = InstanceTransferService::new(Arc::new(StubInstanceTransferRepository::with(
            snapshot(),
        )));
        let document = service.export().await.unwrap();
        assert_eq!(document.format, FORMAT);
        assert_eq!(document.version, VERSION);
    }

    #[tokio::test]
    async fn incompatible_version_is_rejected_without_replacing() {
        let repository = Arc::new(StubInstanceTransferRepository::with(snapshot()));
        let service = InstanceTransferService::new(repository.clone());
        let error = service
            .import(InstanceDocument {
                format: FORMAT.to_string(),
                version: VERSION + 1,
                content: snapshot(),
            })
            .await
            .unwrap_err();

        assert_eq!(error, TransferError::UnsupportedVersion(VERSION + 1));
        assert_eq!(repository.replace_count(), 0);
    }
}
