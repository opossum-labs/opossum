pub mod asset;
pub mod coating;
pub mod index;
pub mod loader;
pub mod registry;
pub mod material;

#[cfg(not(target_arch = "wasm32"))]
pub mod sync;

pub use asset::RegisterableAsset;
pub use coating::CoatingAsset;
pub use index::{AssetIndex, IndexEntry};
pub use loader::AssetLoader;
pub use registry::AssetRegistry;

#[cfg(not(target_arch = "wasm32"))]
pub use sync::RegistrySync;
