use super::{InstanceSnapshot, InstanceTransferRepository, TransferError};
use async_trait::async_trait;
use std::sync::Mutex;

pub struct StubInstanceTransferRepository {
    snapshot: Mutex<InstanceSnapshot>,
    replace_count: Mutex<usize>,
}

impl StubInstanceTransferRepository {
    pub fn with(snapshot: InstanceSnapshot) -> Self {
        Self {
            snapshot: Mutex::new(snapshot),
            replace_count: Mutex::new(0),
        }
    }

    pub fn replace_count(&self) -> usize {
        *self.replace_count.lock().unwrap()
    }
}

impl Default for StubInstanceTransferRepository {
    fn default() -> Self {
        Self::with(InstanceSnapshot {
            materials: vec![],
            manufacturers: vec![],
            locations: vec![],
            spools: vec![],
            printers: vec![],
            configuration: super::SnapshotConfiguration {
                low_stock_threshold: 15,
            },
        })
    }
}

#[async_trait]
impl InstanceTransferRepository for StubInstanceTransferRepository {
    async fn export_snapshot(&self) -> Result<InstanceSnapshot, TransferError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    async fn replace_snapshot(&self, snapshot: InstanceSnapshot) -> Result<(), TransferError> {
        *self.snapshot.lock().unwrap() = snapshot;
        *self.replace_count.lock().unwrap() += 1;
        Ok(())
    }
}
