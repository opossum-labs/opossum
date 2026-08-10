pub mod asset;
pub mod coating;
pub mod loader;
pub mod material;

pub use asset::{AssetHeader, CURRENT_SCHEMA_VERSION, RegisterableAsset};
pub use coating::CoatingAsset;
pub use loader::AssetLoader;
pub use material::{MaterialAsset, MechanicalProperties, OpticalProperties, ThermalProperties};
