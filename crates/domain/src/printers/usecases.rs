use crate::printers::{NewPrinter, Printer, PrinterRepository, PrintersUseCases, RepositoryError};
use crate::shared::PrinterId;
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
}
