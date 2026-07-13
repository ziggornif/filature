pub mod model;
pub mod ports;
pub mod read_models;
pub mod usecases;

#[cfg(feature = "stubs")]
pub mod stubs;

pub use model::{
    Colour, Diameter, NewSpool, Spool, SpoolCondition, SpoolId, SpoolStatus, SpoolType,
    remaining_length_m,
};
pub use ports::api::SpoolsUseCases;
pub use ports::spi::{RepositoryError, SpoolFilter, SpoolRepository, SpoolSort};
pub use read_models::{SpoolDetail, SpoolListItem};
pub use usecases::SpoolsService;
