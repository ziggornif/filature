pub mod model;
pub mod ports;
pub mod seed;
pub mod usecases;

#[cfg(feature = "stubs")]
pub mod stubs;

pub use model::{
    Density, DryingParams, Material, MaterialId, NewMaterial, Sensitivity, Temperature,
};
pub use ports::api::MaterialsUseCases;
pub use ports::spi::{MaterialRepository, RepositoryError};
pub use usecases::MaterialsService;
