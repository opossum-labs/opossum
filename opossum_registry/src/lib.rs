pub mod asset;
pub mod coating;
pub mod index;
pub mod loader;
pub mod material;

pub use asset::RegisterableAsset;
pub use coating::CoatingAsset;
pub use index::{AssetIndex, IndexEntry};
pub use loader::AssetLoader;
