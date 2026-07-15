use super::spi::RepositoryError;
use crate::printers::{NewPrinter, Printer};
use crate::shared::PrinterId;
use async_trait::async_trait;

#[async_trait]
pub trait PrintersUseCases: Send + Sync {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError>;
    async fn add(&self, printer: NewPrinter) -> Result<Printer, RepositoryError>;
    async fn edit(&self, printer: Printer) -> Result<Printer, RepositoryError>;
    async fn delete(&self, id: PrinterId) -> Result<(), RepositoryError>;
}
