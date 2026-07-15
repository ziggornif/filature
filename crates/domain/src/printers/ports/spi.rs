use crate::printers::{LoadableSpool, NewPrinter, Printer};
use crate::shared::{DomainError, PrinterId, SpoolId};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("no printer with id '{}'", .0.as_str())]
    NotFound(PrinterId),
    #[error("no slot '{slot_key}' on printer '{}'", printer_id.as_str())]
    SlotNotFound {
        printer_id: PrinterId,
        slot_key: String,
    },
    #[error("no spool with id '{}'", .0.as_str())]
    UnknownSpool(SpoolId),
    #[error("spool '{}' is already loaded", .0.as_str())]
    AlreadyLoaded(SpoolId),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

#[async_trait]
pub trait PrinterRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError>;
    async fn insert(&self, printer: NewPrinter) -> Result<Printer, RepositoryError>;
    async fn update(&self, printer: Printer) -> Result<Printer, RepositoryError>;
    async fn delete(&self, id: &PrinterId) -> Result<(), RepositoryError>;
    async fn spool_is_loadable(&self, id: &SpoolId) -> Result<Option<bool>, RepositoryError>;
    async fn set_slot_spool(
        &self,
        printer_id: &PrinterId,
        slot_key: &str,
        spool_id: Option<&SpoolId>,
    ) -> Result<(), RepositoryError>;
    async fn clear_spool(&self, spool_id: &SpoolId) -> Result<(), RepositoryError>;
    async fn loadable_spools(
        &self,
        current: Option<&SpoolId>,
    ) -> Result<Vec<LoadableSpool>, RepositoryError>;
}
