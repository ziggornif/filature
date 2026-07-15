use crate::printers::LoadableSpool;
use crate::printers::{NewPrinter, Printer, PrinterRepository, PrintersUseCases, RepositoryError};
use crate::shared::{DomainError, PrinterId, SpoolId};
use async_trait::async_trait;
use std::sync::Arc;

pub struct PrintersService {
    repo: Arc<dyn PrinterRepository>,
}

impl PrintersService {
    pub fn new(repo: Arc<dyn PrinterRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl PrintersUseCases for PrintersService {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError> {
        self.repo.list().await
    }
    async fn add(&self, printer: NewPrinter) -> Result<Printer, RepositoryError> {
        self.repo.insert(printer).await
    }
    async fn edit(&self, printer: Printer) -> Result<Printer, RepositoryError> {
        self.repo.update(printer).await
    }
    async fn delete(&self, id: PrinterId) -> Result<(), RepositoryError> {
        self.repo.delete(&id).await
    }
    async fn load_slot(
        &self,
        printer_id: PrinterId,
        slot_key: String,
        spool_id: SpoolId,
    ) -> Result<(), RepositoryError> {
        match self.repo.spool_is_loadable(&spool_id).await? {
            None => Err(RepositoryError::UnknownSpool(spool_id)),
            Some(false) => Err(RepositoryError::Domain(DomainError::SpoolNotLoadable)),
            Some(true) => {
                self.repo
                    .set_slot_spool(&printer_id, &slot_key, Some(&spool_id))
                    .await
            }
        }
    }
    async fn unload_slot(
        &self,
        printer_id: PrinterId,
        slot_key: String,
    ) -> Result<(), RepositoryError> {
        self.repo.set_slot_spool(&printer_id, &slot_key, None).await
    }
    async fn unload_spool(&self, spool_id: SpoolId) -> Result<(), RepositoryError> {
        self.repo.clear_spool(&spool_id).await
    }
    async fn loadable_spools(
        &self,
        current: Option<SpoolId>,
    ) -> Result<Vec<LoadableSpool>, RepositoryError> {
        self.repo.loadable_spools(current.as_ref()).await
    }
}
