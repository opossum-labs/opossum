use opossum_core::error::{OpmResult, OpossumError};
use opossum_core::material::Material;
use std::collections::HashMap;
use std::fs;
use std::marker::PhantomData;
use uuid::Uuid;

use crate::asset::RegisterableAsset;
use crate::coating::CoatingAsset;
use crate::loader::AssetLoader;
// use crate::material::MaterialAsset;

/// Standard d-line wavelength (587.56 nm) used for nominal refractive index anchor (`n_d`).
pub const WAVELENGTH_D_LINE_NM: f64 = 587.56;

/// Common metadata shared by all indexed assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonIndex {
    /// Unique identifier of the asset.
    pub id: Uuid,

    /// Highest version number available on disk.
    pub latest_version: u32,

    /// List of all available version numbers found on disk (sorted ascending).
    pub available_versions: Vec<u32>,

    /// Display name of the asset.
    pub name: String,

    /// Optional manufacturer name.
    pub manufacturer: Option<String>,

    /// Optional description text.
    pub description: Option<String>,
}

/// A complete index entry combining common metadata and type-specific data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry<D> {
    /// The universal metadata (ID, name, manufacturer).
    pub common: CommonIndex,

    /// The type-specific precomputed data (e.g., `MaterialIndexData`).
    pub specific: D,
}

/// In-memory search index for a specific asset type `T`.
#[derive(Debug, Clone)]
pub struct AssetIndex<T: IndexableAsset> {
    /// Maps asset UUIDs to their complete index entry.
    entries: HashMap<Uuid, IndexEntry<T::IndexData>>,

    /// `PhantomData` marker to bind this index instance to asset type `T`.
    _marker: PhantomData<T>,
}

impl<T: IndexableAsset> Default for AssetIndex<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for assets that can provide type-specific data for RAM caching.
pub trait IndexableAsset: RegisterableAsset {
    /// The specific data type stored in the index alongside the common metadata.
    type IndexData;

    /// Computes and returns the type-specific index data for this asset.
    fn create_index_data(&self) -> Self::IndexData;
}

/// Type-specific index payload for `MaterialAssets`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialIndexData {
    /// Pre-computed nominal refractive index at d-line (587.56 nm), if applicable.
    pub nd: Option<f64>,
}

/// Type-specific index payload for `CoatingAssets`.
/// Currently empty, but prepared for future extensions (e.g., laser damage threshold).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoatingIndexData {}

impl IndexableAsset for CoatingAsset {
    type IndexData = CoatingIndexData;

    fn create_index_data(&self) -> Self::IndexData {
        CoatingIndexData {}
    }
}

impl<T: IndexableAsset> AssetIndex<T> {
    /// Creates a new, empty `AssetIndex`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            _marker: PhantomData,
        }
    }

    /// Returns the number of unique assets indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index contains no assets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves an index entry by its UUID.
    #[must_use]
    pub fn get(&self, id: &Uuid) -> Option<&IndexEntry<T::IndexData>> {
        self.entries.get(id)
    }

    /// Returns a vector containing references to all indexed entries.
    #[must_use]
    pub fn all_entries(&self) -> Vec<&IndexEntry<T::IndexData>> {
        self.entries.values().collect()
    }

    /// Text-based search filtering by name and manufacturer (case-insensitive) on the common metadata.
    #[must_use]
    pub fn search(
        &self,
        name_query: Option<&str>,
        manufacturer_query: Option<&str>,
    ) -> Vec<&IndexEntry<T::IndexData>> {
        self.entries
            .values()
            .filter(|entry| {
                // Filter by manufacturer if requested
                if let Some(mfg) = manufacturer_query {
                    let mfg_lower = mfg.to_lowercase();
                    match &entry.common.manufacturer {
                        Some(entry_mfg) if entry_mfg.to_lowercase().contains(&mfg_lower) => {}
                        _ => return false,
                    }
                }

                // Filter by name or description text query
                if let Some(q) = name_query {
                    let q_lower = q.to_lowercase();
                    let name_matches = entry.common.name.to_lowercase().contains(&q_lower);
                    let desc_matches = entry
                        .common
                        .description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&q_lower));

                    if !name_matches && !desc_matches {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Performs a dynamic evaluation search by loading full assets and applying a custom predicate function.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if loading any underlying asset fails.
    pub fn filter_by<F>(&self, loader: &AssetLoader, predicate: F) -> OpmResult<Vec<T>>
    where
        F: Fn(&T) -> bool,
    {
        let mut matching_assets = Vec::new();

        for entry in self.entries.values() {
            // Load the latest version of the asset for evaluation
            let asset: T = loader.load(entry.common.id, Some(entry.common.latest_version))?;
            if predicate(&asset) {
                matching_assets.push(asset);
            }
        }

        Ok(matching_assets)
    }

    /// Scans the local filesystem via `AssetLoader` and populates the in-memory index for type `T`.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if directory reading fails.
    pub fn build_from_loader(&mut self, loader: &AssetLoader) -> OpmResult<usize> {
        self.entries.clear();

        let subfolder_path = loader.root_path().join(T::relative_subfolder());

        if !subfolder_path.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(&subfolder_path).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to read subfolder directory {}: {e}",
                subfolder_path.display()
            ))
        })?;

        for entry in entries {
            let entry = entry
                .map_err(|e| OpossumError::Other(format!("Failed to read directory entry: {e}")))?;
            let path = entry.path();

            if path.is_dir()
                && let Some(folder_name) = path.file_name().and_then(|s| s.to_str())
                && let Ok(id) = Uuid::parse_str(folder_name)
                && let Ok(versions) = loader.list_versions::<T>(id)
                && let Some(&latest_version) = versions.last()
                && let Ok(asset) = loader.load::<T>(id, Some(latest_version))
            {
                let common = CommonIndex {
                    id,
                    latest_version,
                    available_versions: versions,
                    name: asset.name().to_string(),
                    manufacturer: asset.manufacturer().map(String::from),
                    description: asset.header().description.clone(),
                };

                let index_entry = IndexEntry {
                    common,
                    specific: asset.create_index_data(),
                };

                self.entries.insert(id, index_entry);
            }
        }

        Ok(self.entries.len())
    }
}

// -----------------------------------------------------------------------------
// Type-Specific Index Queries
// -----------------------------------------------------------------------------

impl AssetIndex<Material> {
    /// Searches specifically for material assets whose nominal refractive index `nd` falls within `[min_n, max_n]`.
    #[must_use]
    pub fn search_by_nd_range(
        &self,
        min_n: f64,
        max_n: f64,
    ) -> Vec<&IndexEntry<MaterialIndexData>> {
        self.entries
            .values()
            .filter(|entry: &&IndexEntry<MaterialIndexData>| {
                entry
                    .specific
                    .nd
                    .is_some_and(|nd_val| nd_val >= min_n && nd_val <= max_n)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::coatings::CoatingType;
    use opossum_core::refractive_index::{RefrIndexConst, RefractiveIndexType};
    use tempfile::TempDir;

    #[test]
    fn test_material_index_build_and_nd_search() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Other(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        // Create Material 1: n = 1.5168
        let id1 = Uuid::new_v4();
        let const_1_51 = RefractiveIndexType::Const(RefrIndexConst::new(1.5168)?);
        let mat1 = Material::new(
            id1,
            1,
            "N-BK7",
            Some("Schott".to_string()),
            None,
            const_1_51,
        );
        loader.save_asset(&mat1)?;

        // Build Material Index
        let mut index = AssetIndex::<Material>::new();
        index.build_from_loader(&loader)?;

        assert_eq!(index.len(), 1);

        // search_by_nd_range is available on AssetIndex<MaterialAsset>
        let bk7_matches = index.search_by_nd_range(1.50, 1.55);
        assert_eq!(bk7_matches.len(), 1);
        assert_eq!(bk7_matches[0].common.id, id1);
        assert_eq!(bk7_matches[0].specific.nd, Some(1.5168));

        Ok(())
    }

    #[test]
    fn test_coating_index_build_generic() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Other(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        // Create Coating
        let id = Uuid::new_v4();
        let coating = CoatingAsset::new(
            id,
            1,
            "Ideal AR",
            Some("Thorlabs".to_string()),
            None,
            CoatingType::IdealAR,
        );
        loader.save_asset(&coating)?;

        // Build Coating Index using generic build_from_loader
        let mut index = AssetIndex::<CoatingAsset>::new();
        let count = index.build_from_loader(&loader)?;

        assert_eq!(count, 1);
        let entry = index.get(&id).expect("Coating should be indexed");
        assert_eq!(entry.common.name, "Ideal AR");

        // entry.specific is of type CoatingIndexData {}
        assert_eq!(entry.specific, CoatingIndexData {});

        Ok(())
    }
}
