use crate::printers::{NewPrinter, Printer};
use crate::shared::{DomainError, PrinterId};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
    #[error("no printer with id '{}'", .0.as_str())]
    NotFound(PrinterId),
    #[error("{0}")]
    Domain(#[from] DomainError),
}

#[async_trait]
pub trait PrinterRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Printer>, RepositoryError>;
    async fn insert(&self, printer: NewPrinter) -> Result<Printer, RepositoryError>;
    async fn update(&self, printer: Printer) -> Result<Printer, RepositoryError>;
    async fn delete(&self, id: &PrinterId) -> Result<(), RepositoryError>;
}
