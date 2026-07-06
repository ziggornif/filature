pub mod model;
pub mod ports;
pub mod usecases;

#[cfg(any(test, feature = "stubs"))]
pub mod stubs;

pub use model::{Colour, Diameter, NewSpool, Spool, SpoolId, SpoolStatus, remaining_length_m};
pub use ports::api::SpoolsUseCases;
pub use ports::spi::{RepositoryError, SpoolFilter, SpoolRepository, SpoolSort};
pub use usecases::SpoolsService;
