pub mod model;
pub mod ports;
pub mod read_models;
pub mod usecases;

#[cfg(feature = "stubs")]
pub mod stubs;

pub use model::{
    Colour, Diameter, EditSpool, NewSpool, Spool, SpoolCondition, SpoolId, SpoolStatus, SpoolType,
    normalize_ams_tag_uid, remaining_length_m,
};
pub use ports::api::SpoolsUseCases;
pub use ports::spi::{ReconcilableSpool, RepositoryError, SpoolFilter, SpoolRepository, SpoolSort};
pub use read_models::{SpoolDetail, SpoolListItem};
pub use usecases::SpoolsService;
