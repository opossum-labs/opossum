pub mod asset;
pub mod coating;
pub mod index;
pub mod loader;
pub mod material;
pub mod registry;

#[cfg(not(target_arch = "wasm32"))]
pub mod sync;

pub use coating::CoatingAsset;
pub use registry::AssetRegistry;

#[cfg(not(target_arch = "wasm32"))]
pub use sync::RegistrySync;
