pub mod asset;
pub mod coating;
pub mod material;

pub use asset::{AssetHeader, CURRENT_SCHEMA_VERSION, RegisterableAsset};
pub use coating::CoatingAsset;
pub use material::MaterialAsset;
