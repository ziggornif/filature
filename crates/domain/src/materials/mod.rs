pub mod model;
pub mod ports;

#[cfg(feature = "stubs")]
pub mod stubs;

pub use model::{
    Density, DryingParams, Material, MaterialId, NewMaterial, Sensitivity, Temperature,
};
pub use ports::api::MaterialsUseCases;
pub use ports::spi::{MaterialRepository, RepositoryError};
