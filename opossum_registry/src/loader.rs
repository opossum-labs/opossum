use opossum_core::error::{OpmResult, OpossumError};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::asset::RegisterableAsset;

/// Handles loading, saving, and version resolution of registry assets on disk.
#[derive(Debug, Clone)]
pub struct AssetLoader {
    /// Root directory of the local registry repository (e.g., ~/.opossum/data).
    root_path: PathBuf,
}

impl AssetLoader {
    /// Creates a new `AssetLoader` instance pointing to the specified registry root directory.
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    /// Returns a reference to the root directory path.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Computes the directory path for a specific asset UUID.
    #[must_use]
    pub fn asset_directory<T: RegisterableAsset>(&self, id: Uuid) -> PathBuf {
        self.root_path
            .join(T::relative_subfolder())
            .join(id.to_string())
    }

    /// Computes the full file path for a specific asset version.
    #[must_use]
    pub fn asset_file_path<T: RegisterableAsset>(&self, id: Uuid, version: u32) -> PathBuf {
        self.asset_directory::<T>(id)
            .join(format!("v{version}.ron"))
    }

    /// Lists all available version numbers stored on disk for a given asset UUID.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if the asset directory cannot be read.
    pub fn list_versions<T: RegisterableAsset>(&self, id: Uuid) -> OpmResult<Vec<u32>> {
        let dir_path = self.asset_directory::<T>(id);

        if !dir_path.exists() {
            return Err(OpossumError::Registry(format!(
                "Asset directory for UUID {id} does not exist at {}",
                dir_path.display()
            )));
        }

        let entries = fs::read_dir(&dir_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to read asset directory {}: {e}",
                dir_path.display()
            ))
        })?;

        let mut versions = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| {
                OpossumError::Registry(format!("Failed to read directory entry: {e}"))
            })?;
            let path = entry.path();

            // Extract version numbers from filenames (e.g., "v1.ron" -> 1)
            // Using standard pattern matching for stable Rust compatibility
            if path.is_file()
                && let (Some(ext), Some(stem)) = (path.extension(), path.file_stem())
                && ext.eq_ignore_ascii_case("ron")
                && let Some(stem_str) = stem.to_str()
                && let Some(version_str) = stem_str.strip_prefix('v')
                && let Ok(version_num) = version_str.parse::<u32>()
            {
                versions.push(version_num);
            }
        }

        if versions.is_empty() {
            return Err(OpossumError::Registry(format!(
                "No valid version files found in directory {}",
                dir_path.display()
            )));
        }

        versions.sort_unstable();
        Ok(versions)
    }

    /// Unified asset loader.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if the file does not exist or fails to parse.
    pub fn load<T: RegisterableAsset>(&self, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        let target_version = if let Some(v) = version {
            v
        } else {
            let available_versions = self.list_versions::<T>(id)?;
            *available_versions.last().ok_or_else(|| {
                OpossumError::Registry(format!("No available versions found for UUID {id}"))
            })?
        };

        let file_path = self.asset_file_path::<T>(id, target_version);

        if !file_path.exists() {
            return Err(OpossumError::Registry(format!(
                "Asset file for UUID {id} version {target_version} not found at {}",
                file_path.display()
            )));
        }

        let content = fs::read_to_string(&file_path).map_err(|e| {
            OpossumError::Registry(format!("Failed to read file {}: {e}", file_path.display()))
        })?;

        ron::from_str::<T>(&content).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to deserialize RON in {}: {e}",
                file_path.display()
            ))
        })
    }

    /// Publishes a material draft to the local registry.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if directory creation, serialization, or writing fails.
    pub fn publish<T: RegisterableAsset>(&self, asset: &mut T) -> OpmResult<PathBuf> {
        if asset.version() == 0 {
            let next_version = self.next_version_number::<T>(asset.id());
            asset.header_mut().version = next_version;
        }

        let file_path = self.asset_file_path::<T>(asset.id(), asset.version());
        let dir_path = file_path.parent().ok_or_else(|| {
            OpossumError::Registry(format!(
                "Invalid file path parent for {}",
                file_path.display()
            ))
        })?;

        fs::create_dir_all(dir_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to create directories {}: {e}",
                dir_path.display()
            ))
        })?;

        let pretty_config = ron::ser::PrettyConfig::default();
        let ron_str = ron::ser::to_string_pretty(asset, pretty_config).map_err(|e| {
            OpossumError::Registry(format!("Failed to serialize asset to RON: {e}"))
        })?;

        fs::write(&file_path, ron_str).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to write asset file {}: {e}",
                file_path.display()
            ))
        })?;

        Ok(file_path)
    }

    /// Computes the next available version number for an asset UUID (`latest + 1`).
    #[must_use]
    pub fn next_version_number<T: RegisterableAsset>(&self, id: Uuid) -> u32 {
        self.list_versions::<T>(id)
            .ok()
            .and_then(|versions| versions.last().copied())
            .map_or(1, |latest| latest + 1)
    }

    /// Deletes the latest version of an asset from disk.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if reading directory contents or deleting files fails.
    pub fn delete_latest_version<T: RegisterableAsset>(&self, id: Uuid) -> OpmResult<Option<u32>> {
        let versions = self.list_versions::<T>(id)?;
        let latest_version = *versions
            .last()
            .ok_or_else(|| OpossumError::Registry(format!("No versions found for UUID {id}")))?;

        let file_path = self.asset_file_path::<T>(id, latest_version);
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to delete version file {}: {e}",
                    file_path.display()
                ))
            })?;
        }

        if versions.len() <= 1 {
            let dir_path = self.asset_directory::<T>(id);
            if dir_path.exists() {
                fs::remove_dir_all(&dir_path).map_err(|e| {
                    OpossumError::Registry(format!(
                        "Failed to remove empty asset directory {}: {e}",
                        dir_path.display()
                    ))
                })?;
            }
            Ok(None)
        } else {
            Ok(Some(versions[versions.len() - 2]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::{material::Material, refractive_index::RefractiveIndexType};
    use tempfile::TempDir;

    #[test]
    fn test_unified_load_specific_version() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let mut material = Material::new_draft(
            "N-BK7",
            Some("Schott".to_string()),
            None,
            RefractiveIndexType::default(),
        );

        loader.publish(&mut material)?;

        let loaded: Material = loader.load(material.id(), Some(1))?;
        assert_eq!(loaded.name(), "N-BK7");
        assert_eq!(loaded.version(), 1);

        Ok(())
    }

    #[test]
    fn test_unified_load_latest_version() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let mut mat_v1 = Material::new_draft(
            "N-BK7 v1",
            Some("Schott".to_string()),
            None,
            RefractiveIndexType::default(),
        );
        loader.publish(&mut mat_v1)?;

        let mut mat_v2 = Material::new_draft_from(&mat_v1);
        mat_v2.header.name = "N-BK7 v2".to_string();
        loader.publish(&mut mat_v2)?;

        let latest: Material = loader.load(mat_v1.id(), None)?;
        assert_eq!(latest.version(), 2);
        assert_eq!(latest.name(), "N-BK7 v2");

        Ok(())
    }

    #[test]
    fn test_load_nonexistent_asset_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let loader = AssetLoader::new(temp_dir.path());
        let id = Uuid::new_v4();

        let result: OpmResult<Material> = loader.load(id, Some(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_next_version_number_calculation() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let mut mat_v1 = Material::new_draft("Glass", None, None, RefractiveIndexType::default());
        let mat_id = mat_v1.id();

        assert_eq!(loader.next_version_number::<Material>(mat_id), 1);

        loader.publish(&mut mat_v1)?;
        assert_eq!(loader.next_version_number::<Material>(mat_id), 2);

        Ok(())
    }

    #[test]
    fn test_delete_latest_version_stepwise_and_full_removal() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let mut mat_v1 = Material::new_draft("Glass", None, None, RefractiveIndexType::default());
        let id = mat_v1.id();
        loader.publish(&mut mat_v1)?;

        let mut mat_v2 = mat_v1.new_draft_from();
        loader.publish(&mut mat_v2)?;
        assert_eq!(loader.list_versions::<Material>(id)?, vec![1, 2]);

        let remaining_latest = loader.delete_latest_version::<Material>(id)?;
        assert_eq!(remaining_latest, Some(1));
        assert_eq!(loader.list_versions::<Material>(id)?, vec![1]);

        let remaining_latest = loader.delete_latest_version::<Material>(id)?;
        assert_eq!(remaining_latest, None);
        assert!(!loader.asset_directory::<Material>(id).exists());

        Ok(())
    }
}
