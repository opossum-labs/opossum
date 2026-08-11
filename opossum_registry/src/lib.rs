pub mod asset;
pub mod coating;
pub mod index;
pub mod loader;
pub mod material;

pub use asset::{AssetHeader, CURRENT_SCHEMA_VERSION, RegisterableAsset};
pub use coating::CoatingAsset;
pub use index::{AssetIndex, IndexEntry};
pub use loader::AssetLoader;
pub use material::{MaterialAsset, MechanicalProperties, OpticalProperties, ThermalProperties};
