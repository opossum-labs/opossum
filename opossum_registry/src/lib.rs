pub mod asset;
pub mod coating;
pub mod index;
pub mod loader;
pub mod material;
pub mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod sync;

pub use coating::CoatingAsset;
pub use registry::{AssetRegistry, CatalogSource, OFFICIAL_CATALOG_SUBDIR, USER_CATALOG_SUBDIR};
#[cfg(not(target_arch = "wasm32"))]
pub use sync::{CatalogRole, DEFAULT_BRANCH, DEFAULT_CATALOG_REMOTE_URL, RegistrySync};
