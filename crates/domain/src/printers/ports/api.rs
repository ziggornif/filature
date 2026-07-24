use super::spi::RepositoryError;
use crate::printers::LoadableSpool;
use crate::printers::{AmsTray, MachineError, MachineLink, MachineStatus, NewPrinter, Printer};
use crate::shared::{PrinterId, SpoolId};
use async_trait::async_trait;

#[async_trait]
pub trait PrintersUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError>;
    async fn add(&self, printer: NewPrinter) -> Result<Printer, RepositoryError>;
    async fn edit(&self, printer: Printer) -> Result<Printer, RepositoryError>;
    async fn delete(&self, id: PrinterId) -> Result<(), RepositoryError>;
    async fn load_slot(
        &self,
        printer_id: PrinterId,
        slot_key: String,
        spool_id: SpoolId,
    ) -> Result<(), RepositoryError>;
    async fn unload_slot(
        &self,
        printer_id: PrinterId,
        slot_key: String,
    ) -> Result<(), RepositoryError>;
    async fn unload_spool(&self, spool_id: SpoolId) -> Result<(), RepositoryError>;
    async fn loadable_spools(
        &self,
        current: Option<SpoolId>,
    ) -> Result<Vec<LoadableSpool>, RepositoryError>;
}

#[async_trait]
pub trait MachineConnectivityUseCases: Send + Sync {
    async fn fetch_ams_trays(&self, printer_id: PrinterId) -> Result<Vec<AmsTray>, MachineError>;
    async fn get_printer_status(
        &self,
        printer_id: PrinterId,
    ) -> Result<MachineStatus, MachineError>;
    async fn test_machine_link(&self, link: MachineLink) -> Result<MachineStatus, MachineError>;
    async fn test_printer_machine_link(
        &self,
        printer_id: PrinterId,
        endpoint: String,
    ) -> Result<MachineStatus, MachineError>;
}
