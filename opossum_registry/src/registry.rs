//! High-level asset registry facade combining filesystem storage and in-memory caching.

use opossum_core::error::OpmResult;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::index::{AssetIndex, IndexEntry, IndexableAsset};
use crate::loader::AssetLoader;

/// Combined registry providing atomic file persistence and high-performance in-memory indexing.
#[derive(Debug, Clone)]
pub struct AssetRegistry<T: IndexableAsset> {
    /// Underlying filesystem loader.
    loader: AssetLoader,
    /// In-memory search index cache.
    index: AssetIndex<T>,
}

impl<T: IndexableAsset> AssetRegistry<T> {
    /// Creates a new `AssetRegistry` and immediately builds its in-memory index from disk.
    ///
    /// # Errors
    /// Returns an error if directory traversal or index population fails.
    pub fn new(root_path: impl Into<PathBuf>) -> OpmResult<Self> {
        let loader = AssetLoader::new(root_path);
        let mut index = AssetIndex::new();
        index.build_from_loader(&loader)?;

        Ok(Self { loader, index })
    }

    /// Returns a reference to the registry root path.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        self.loader.root_path()
    }

    /// Returns a reference to the read-only in-memory index.
    #[must_use]
    pub const fn index(&self) -> &AssetIndex<T> {
        &self.index
    }

    /// Returns a reference to the low-level loader.
    #[must_use]
    pub const fn loader(&self) -> &AssetLoader {
        &self.loader
    }

    /// Rebuilds the complete index from disk.
    ///
    /// Useful after external repository synchronizations (e.g. Git pull).
    ///
    /// # Errors
    /// Returns an error if directory reading fails.
    pub fn rebuild_index(&mut self) -> OpmResult<usize> {
        self.index.build_from_loader(&self.loader)
    }

    /// Publishes an asset draft to disk and updates the in-memory cache in O(1).
    ///
    /// # Errors
    /// Returns an error if disk serialization or writing fails.
    pub fn publish(&mut self, asset: &mut T) -> OpmResult<PathBuf> {
        // 1. Persist to disk using loader (updates asset version in-place)
        let file_path = self.loader.publish(asset)?;

        // 2. Fetch updated version history for this asset
        let versions = self.loader.list_versions::<T>(asset.id())?;

        // 3. Immediately update the in-memory index cache (O(1))
        self.index.update_entry(asset, versions);

        Ok(file_path)
    }

    /// Unified asset loader forwarding to `AssetLoader::load`.
    ///
    /// # Errors
    /// Returns an error if the asset file cannot be read or parsed.
    pub fn load(&self, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        self.loader.load(id, version)
    }

    /// Deletes the latest version of an asset and automatically updates the in-memory cache.
    ///
    /// # Errors
    /// Returns an error if file deletion or index cache reversion fails.
    pub fn delete_latest_version(&mut self, id: Uuid) -> OpmResult<Option<u32>> {
        // 1. Delete version on disk
        let remaining_latest = self.loader.delete_latest_version::<T>(id)?;

        // 2. Synchronize in-memory index
        self.index
            .remove_or_revert_entry(id, remaining_latest, &self.loader)?;

        Ok(remaining_latest)
    }

    /// Direct text search proxy delegating to the indexed entries.
    #[must_use]
    pub fn search_text(&self, query: &str) -> Vec<&IndexEntry<T::IndexData>> {
        self.index.search_text(query)
    }

    /// Direct all-entries proxy.
    #[must_use]
    pub fn all_entries(&self) -> Vec<&IndexEntry<T::IndexData>> {
        self.index.all_entries()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::error::OpossumError;
    use opossum_core::material::Material;
    use opossum_core::refractive_index::RefrIndexConst;
    use tempfile::TempDir;

    #[test]
    fn test_registry_atomic_publish_and_index_sync() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let mut registry = AssetRegistry::<Material>::new(temp_dir.path())?;

        assert_eq!(registry.index().len(), 0);

        // Publish new material draft
        let mut mat = Material::new_draft(
            "N-BK7",
            Some("Schott".to_string()),
            None,
            RefrIndexConst::new(1.5168)?.into(),
        );
        let id = mat.id();
        registry.publish(&mut mat)?;

        // Index must be instantly updated in memory without manual rebuild
        assert_eq!(registry.index().len(), 1);
        let entry = registry.index().get(&id).expect("Asset should be indexed");
        assert_eq!(entry.common.name, "N-BK7");
        assert_eq!(entry.common.latest_version, 1);
        assert_eq!(entry.specific.nd, Some(1.5168));

        // Delete latest version
        let remaining = registry.delete_latest_version(id)?;
        assert_eq!(remaining, None);
        assert_eq!(registry.index().len(), 0);

        Ok(())
    }
}
