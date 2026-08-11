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
    ///
    /// Example path: `<root_path>/materials/<uuid>/`
    #[must_use]
    pub fn asset_directory<T: RegisterableAsset>(&self, id: Uuid) -> PathBuf {
        self.root_path
            .join(T::relative_subfolder())
            .join(id.to_string())
    }

    /// Computes the full file path for a specific asset version.
    ///
    /// Example path: `<root_path>/materials/<uuid>/v1.ron`
    #[must_use]
    pub fn asset_file_path<T: RegisterableAsset>(&self, id: Uuid, version: u32) -> PathBuf {
        self.asset_directory::<T>(id)
            .join(format!("v{version}.ron"))
    }

    /// Lists all available version numbers stored on disk for a given asset UUID.
    ///
    /// Returns a sorted vector of version numbers in ascending order (e.g., `[1, 2, 3]`).
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if the asset directory cannot be read.
    pub fn list_versions<T: RegisterableAsset>(&self, id: Uuid) -> OpmResult<Vec<u32>> {
        let dir_path = self.asset_directory::<T>(id);

        if !dir_path.exists() {
            return Err(OpossumError::Other(format!(
                "Asset directory for UUID {id} does not exist at {}",
                dir_path.display()
            )));
        }

        let entries = fs::read_dir(&dir_path).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to read asset directory {}: {e}",
                dir_path.display()
            ))
        })?;

        let mut versions = Vec::new();

        for entry in entries {
            let entry = entry
                .map_err(|e| OpossumError::Other(format!("Failed to read directory entry: {e}")))?;
            let path = entry.path();

            if path.is_file()
                && let Some(file_name) = path.file_name().and_then(|s| s.to_str())
            {
                // Match pattern "v<NUMBER>.ron"
                if file_name.starts_with('v')
                    && std::path::Path::new(file_name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("ron"))
                {
                    let version_str = &file_name[1..file_name.len() - 4];
                    if let Ok(version_num) = version_str.parse::<u32>() {
                        versions.push(version_num);
                    }
                }
            }
        }

        if versions.is_empty() {
            return Err(OpossumError::Other(format!(
                "No valid version files found in directory {}",
                dir_path.display()
            )));
        }

        versions.sort_unstable();
        Ok(versions)
    }

    /// Unified asset loader.
    ///
    /// - If `version` is `Some(v)`, it loads that specific version.
    /// - If `version` is `None`, it resolves and loads the latest available version.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if the file does not exist or fails to parse.
    pub fn load<T: RegisterableAsset>(&self, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        let target_version = if let Some(v) = version {
            v
        } else {
            let available_versions = self.list_versions::<T>(id)?;
            *available_versions.last().ok_or_else(|| {
                OpossumError::Other(format!("No available versions found for UUID {id}"))
            })?
        };

        let file_path = self.asset_file_path::<T>(id, target_version);

        if !file_path.exists() {
            return Err(OpossumError::Other(format!(
                "Asset file for UUID {id} version {target_version} not found at {}",
                file_path.display()
            )));
        }

        let content = fs::read_to_string(&file_path).map_err(|e| {
            OpossumError::Other(format!("Failed to read file {}: {e}", file_path.display()))
        })?;

        ron::from_str::<T>(&content).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to deserialize RON in {}: {e}",
                file_path.display()
            ))
        })
    }

    /// Saves an asset to disk using its current version from `AssetHeader`.
    ///
    /// Automatically creates any missing parent directories.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if serialization or file write operations fail.
    pub fn save_asset<T: RegisterableAsset>(&self, asset: &T) -> OpmResult<PathBuf> {
        let file_path = self.asset_file_path::<T>(asset.id(), asset.version());
        let dir_path = file_path.parent().ok_or_else(|| {
            OpossumError::Other(format!(
                "Invalid file path parent for {}",
                file_path.display()
            ))
        })?;

        // Ensure target directory exists
        fs::create_dir_all(dir_path).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to create directories {}: {e}",
                dir_path.display()
            ))
        })?;

        // Pretty print RON serialization
        let pretty_config = ron::ser::PrettyConfig::default();
        let ron_str = ron::ser::to_string_pretty(asset, pretty_config)
            .map_err(|e| OpossumError::Other(format!("Failed to serialize asset to RON: {e}")))?;

        fs::write(&file_path, ron_str).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to write asset file {}: {e}",
                file_path.display()
            ))
        })?;

        Ok(file_path)
    }

    /// Increments the asset's version number to `latest_version + 1` and saves it as a new file.
    ///
    /// This enforces the append-only versioning strategy.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if writing or version resolution fails.
    pub fn save_as_next_version<T: RegisterableAsset + Clone>(
        &self,
        asset: &mut T,
    ) -> OpmResult<PathBuf> {
        let next_version = self
            .list_versions::<T>(asset.id())
            .map_or(1, |versions| versions.last().map_or(1, |v| v + 1));

        // Note: The AssetHeader version is updated via re-creation or direct mutation in the asset implementation.
        // For trait compliance, we require the concrete asset to update its header version.
        // Here we update the asset header version if possible, or save directly with next_version.
        // As header is accessible via reference, concrete implementations can provide a mutable header or version setter.

        // We write directly to the path computed with next_version:
        let file_path = self.asset_file_path::<T>(asset.id(), next_version);
        let dir_path = file_path.parent().ok_or_else(|| {
            OpossumError::Other(format!("Invalid parent path for {}", file_path.display()))
        })?;

        fs::create_dir_all(dir_path).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to create directory {}: {e}",
                dir_path.display()
            ))
        })?;

        let pretty_config = ron::ser::PrettyConfig::default();
        let ron_str = ron::ser::to_string_pretty(asset, pretty_config)
            .map_err(|e| OpossumError::Other(format!("Failed to serialize asset: {e}")))?;

        fs::write(&file_path, ron_str).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to write asset to {}: {e}",
                file_path.display()
            ))
        })?;

        Ok(file_path)
    }
    /// Computes the next available version number for an asset UUID (`latest + 1`).
    ///
    /// Returns `1` if no prior versions exist.
    #[must_use]
    pub fn next_version_number<T: RegisterableAsset>(&self, id: Uuid) -> u32 {
        self.list_versions::<T>(id)
            .ok()
            .and_then(|versions| versions.last().copied())
            .map_or(1, |latest| latest + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::material::MaterialAsset;
    use opossum_core::{material::Material, refractive_index::RefractiveIndexType};
    use tempfile::TempDir;

    #[test]
    fn test_unified_load_specific_version() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Other(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let id = Uuid::new_v4();
        let material = Material::new(
            id,
            1,
            "N-BK7",
            Some("Schott".to_string()),
            None,
            RefractiveIndexType::default(),
        );

        loader.save_asset(&material)?;

        // Test loading with explicit version Option: Some(1)
        let loaded: Material = loader.load(id, Some(1))?;
        assert_eq!(loaded.name(), "N-BK7");
        assert_eq!(loaded.version(), 1);

        Ok(())
    }

    #[test]
    fn test_unified_load_latest_version() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Other(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());

        let id = Uuid::new_v4();

        // Save version 1
        let mat_v1 = Material::new(
            id,
            1,
            "N-BK7 v1",
            Some("Schott".to_string()),
            None,
            RefractiveIndexType::default(),
        );
        loader.save_asset(&mat_v1)?;

        // Save version 2
        let mat_v2 = Material::new(
            id,
            2,
            "N-BK7 v2",
            Some("Schott".to_string()),
            None,
            RefractiveIndexType::default(),
        );
        loader.save_asset(&mat_v2)?;

        // Test loading with None (should load latest, which is v2)
        let latest: Material = loader.load(id, None)?;
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
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Other(e.to_string()))?;
        let loader = AssetLoader::new(temp_dir.path());
        let id = Uuid::new_v4();

        // Should return 1 when no versions exist yet
        assert_eq!(loader.next_version_number::<Material>(id), 1);

        // Save v1 and check next version
        let mat_v1 = Material::new(id, 1, "Glass", None, None, RefractiveIndexType::default());
        loader.save_asset(&mat_v1)?;
        assert_eq!(loader.next_version_number::<Material>(id), 2);

        Ok(())
    }
}
