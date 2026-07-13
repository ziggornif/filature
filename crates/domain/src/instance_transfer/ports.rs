use super::{InstanceDocument, InstanceSnapshot, TransferError};
use async_trait::async_trait;

/// Driving port for complete, versioned instance backups.
#[async_trait]
pub trait InstanceTransferUseCases: Send + Sync {
    async fn export(&self) -> Result<InstanceDocument, TransferError>;
    async fn import(&self, document: InstanceDocument) -> Result<(), TransferError>;
}

/// Driven port that owns the all-table read and atomic replacement boundary.
#[async_trait]
pub trait InstanceTransferRepository: Send + Sync {
    async fn export_snapshot(&self) -> Result<InstanceSnapshot, TransferError>;
    async fn replace_snapshot(&self, snapshot: InstanceSnapshot) -> Result<(), TransferError>;
}
