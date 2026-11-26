//! High-level asset registry facade managing both the shared official catalog (`official/`)
//! and the local user catalog (`user/`).
use crate::{
    index::{AssetIndex, IndexEntry, IndexableAsset},
    loader::AssetLoader,
};
use opossum_core::error::{OpmResult, OpossumError};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Subdirectory name for the shared official catalog under the registry root.
pub const OFFICIAL_CATALOG_SUBDIR: &str = "official";
/// Subdirectory name for the local user catalog under the registry root.
pub const USER_CATALOG_SUBDIR: &str = "user";

/// Indicates which catalog source an asset belongs to or targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    /// Shared official catalog (`<root>/official`).
    Official,
    /// Local user catalog (`<root>/user`).
    User,
}

/// Combined registry providing `official` (read/sync from remote) and `user` (local edits/custom assets)
/// catalog sources with separate in-memory search indices.
#[derive(Debug, Clone)]
pub struct AssetRegistry<T: IndexableAsset> {
    /// Root directory containing `official/` and `user/`.
    root_path: PathBuf,
    /// Filesystem loader for `<root>/official`.
    official_loader: AssetLoader,
    /// In-memory index for `<root>/official`.
    official_index: AssetIndex<T>,
    /// Filesystem loader for `<root>/user`.
    user_loader: AssetLoader,
    /// In-memory index for `<root>/user`.
    user_index: AssetIndex<T>,
}

impl<T: IndexableAsset> AssetRegistry<T> {
    /// Creates a new `AssetRegistry` with `official/` and `user/` subdirectories,
    /// initializes Git repositories on `main`, and builds both in-memory indices.
    ///
    /// # Errors
    /// Returns an error if directory creation, repository initialization, or index building fails.
    pub fn new(root_path: impl Into<PathBuf>) -> OpmResult<Self> {
        Self::with_official_remote(root_path, None)
    }

    /// Creates a new `AssetRegistry` with an explicit remote URL for the official catalog
    /// (falling back to `DEFAULT_CATALOG_REMOTE_URL` when `None`).
    ///
    /// # Errors
    /// Returns an error if directory creation, repository initialization, or index building fails.
    pub fn with_official_remote(
        root_path: impl Into<PathBuf>,
        official_remote_url: Option<&str>,
    ) -> OpmResult<Self> {
        let root = root_path.into();
        let official_path = root.join(OFFICIAL_CATALOG_SUBDIR);
        let user_path = root.join(USER_CATALOG_SUBDIR);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let official_sync =
                crate::sync::RegistrySync::new_official(&official_path, official_remote_url);
            official_sync.ensure_repository_initialized()?;

            let user_sync = crate::sync::RegistrySync::new_user(&user_path);
            user_sync.ensure_repository_initialized()?;
        }

        let official_loader = AssetLoader::new(&official_path);
        let mut official_index = AssetIndex::new();
        official_index.build_from_loader(&official_loader)?;

        let user_loader = AssetLoader::new(&user_path);
        let mut user_index = AssetIndex::new();
        user_index.build_from_loader(&user_loader)?;

        Ok(Self {
            root_path: root,
            official_loader,
            official_index,
            user_loader,
            user_index,
        })
    }

    /// Returns the combined registry root directory path.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Returns the filesystem path of the official catalog (`<root>/official`).
    #[must_use]
    pub fn official_path(&self) -> PathBuf {
        self.root_path.join(OFFICIAL_CATALOG_SUBDIR)
    }

    /// Returns the filesystem path of the user catalog (`<root>/user`).
    #[must_use]
    pub fn user_path(&self) -> PathBuf {
        self.root_path.join(USER_CATALOG_SUBDIR)
    }

    /// Returns a reference to the official catalog index.
    #[must_use]
    pub const fn official_index(&self) -> &AssetIndex<T> {
        &self.official_index
    }

    /// Returns a reference to the user catalog index.
    #[must_use]
    pub const fn user_index(&self) -> &AssetIndex<T> {
        &self.user_index
    }

    /// Returns a reference to the primary index (defaults to `official_index` for backward compatibility).
    #[must_use]
    pub const fn index(&self) -> &AssetIndex<T> {
        &self.official_index
    }

    /// Returns a reference to the loader for the specified catalog source.
    #[must_use]
    pub const fn loader_for(&self, source: CatalogSource) -> &AssetLoader {
        match source {
            CatalogSource::Official => &self.official_loader,
            CatalogSource::User => &self.user_loader,
        }
    }

    /// Returns a reference to the primary loader (`official_loader`).
    #[must_use]
    pub const fn loader(&self) -> &AssetLoader {
        &self.official_loader
    }

    /// Returns `true` if `id` exists in the official catalog (UUID is single point of truth).
    #[must_use]
    pub fn is_official(&self, id: Uuid) -> bool {
        self.official_index.get(&id).is_some()
    }

    /// Returns `true` if `id` exists in the user catalog.
    #[must_use]
    pub fn in_user_catalog(&self, id: Uuid) -> bool {
        self.user_index.get(&id).is_some()
    }

    /// Rebuilds both `official` and `user` in-memory indices from disk.
    ///
    /// # Errors
    /// Returns an error if reading either catalog directory fails.
    pub fn rebuild_index(&mut self) -> OpmResult<usize> {
        let official_count = self
            .official_index
            .build_from_loader(&self.official_loader)?;
        let user_count = self.user_index.build_from_loader(&self.user_loader)?;
        Ok(official_count + user_count)
    }

    /// Publishes an asset draft into the user catalog (`<root>/user`), updates the user index,
    /// and commits locally to the user catalog's `main` branch.
    ///
    /// # Errors
    /// Returns an error if writing, indexing, or local Git commit fails.
    pub fn publish(&mut self, asset: &mut T) -> OpmResult<PathBuf> {
        self.publish_to(CatalogSource::User, asset)
    }

    /// Publishes an asset draft to the target catalog (`User` or `Official`), updates its index,
    /// and commits to that repository's `main` branch if initialized.
    ///
    /// # Errors
    /// Returns an error if writing, indexing, or Git commit fails.
    pub fn publish_to(&mut self, source: CatalogSource, asset: &mut T) -> OpmResult<PathBuf> {
        let (loader, index) = match source {
            CatalogSource::Official => (&self.official_loader, &mut self.official_index),
            CatalogSource::User => (&self.user_loader, &mut self.user_index),
        };

        let file_path = loader.publish(asset)?;
        let versions = loader.list_versions::<T>(asset.id())?;
        index.update_entry(asset, versions);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let repo_root = loader.root_path();
            if repo_root.join(".git").exists() {
                let msg = format!(
                    "Publish {} '{}' (v{})",
                    T::relative_subfolder(),
                    asset.name(),
                    asset.version()
                );
                crate::sync::commit_asset_file(repo_root, &file_path, &msg)?;
            }
        }
        Ok(file_path)
    }

    /// Copies an existing asset from `source` into the user catalog (`<root>/user`)
    /// with a brand-new UUID so there are no ID or version collisions with the official catalog.
    ///
    /// # Errors
    /// Returns an error if loading the source asset or publishing into the user catalog fails.
    pub fn copy_to_user_catalog(
        &mut self,
        source: CatalogSource,
        id: Uuid,
        version: Option<u32>,
        new_name: Option<String>,
    ) -> OpmResult<T> {
        let source_loader = self.loader_for(source);
        let mut copied: T = source_loader.load(id, version)?;
        copied.header_mut().id = Uuid::new_v4();
        copied.header_mut().version = 0;
        if let Some(name) = new_name {
            copied.header_mut().name = name;
        }
        self.publish_to(CatalogSource::User, &mut copied)?;
        Ok(copied)
    }

    /// Loads an asset by UUID, checking `official` first and falling back to `user`.
    ///
    /// # Errors
    /// Returns an error if the asset is not found in either catalog or fails to deserialize.
    pub fn load(&self, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        if self.is_official(id) {
            self.official_loader.load(id, version)
        } else if self.in_user_catalog(id) {
            self.user_loader.load(id, version)
        } else {
            Err(OpossumError::Registry(format!(
                "Asset with UUID {id} not found in official or user catalog"
            )))
        }
    }

    /// Loads an asset explicitly from the given catalog source.
    ///
    /// # Errors
    /// Returns an error if the asset file does not exist or fails to deserialize.
    pub fn load_from(&self, source: CatalogSource, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        self.loader_for(source).load(id, version)
    }

    /// Deletes the latest version of an asset from the specified catalog source and updates its index.
    ///
    /// # Errors
    /// Returns an error if file deletion or index reversion fails.
    pub fn delete_latest_version_from(
        &mut self,
        source: CatalogSource,
        id: Uuid,
    ) -> OpmResult<Option<u32>> {
        let (loader, index) = match source {
            CatalogSource::Official => (&self.official_loader, &mut self.official_index),
            CatalogSource::User => (&self.user_loader, &mut self.user_index),
        };
        let remaining_latest = loader.delete_latest_version::<T>(id)?;
        index.remove_or_revert_entry(id, remaining_latest, loader)?;
        Ok(remaining_latest)
    }

    /// Deletes the latest version from the user catalog (default write target).
    ///
    /// # Errors
    /// Returns an error if file deletion or index reversion fails.
    pub fn delete_latest_version(&mut self, id: Uuid) -> OpmResult<Option<u32>> {
        self.delete_latest_version_from(CatalogSource::User, id)
    }

    /// Searches text across both `official` and `user` indices.
    #[must_use]
    pub fn search_text(&self, query: &str) -> Vec<&IndexEntry<T::IndexData>> {
        let mut results = self.official_index.search_text(query);
        results.extend(self.user_index.search_text(query));
        results
    }

    /// Returns all indexed entries across both `official` and `user` catalogs.
    #[must_use]
    pub fn all_entries(&self) -> Vec<&IndexEntry<T::IndexData>> {
        let mut entries = self.official_index.all_entries();
        entries.extend(self.user_index.all_entries());
        entries
    }
}
#[cfg(test)]
mod dual_catalog_tests {
    use super::*;
    use crate::sync::{DEFAULT_BRANCH, DEFAULT_CATALOG_REMOTE_URL, RegistrySync};
    use opossum_core::error::{OpmResult, OpossumError};
    use opossum_core::material::Material;
    use opossum_core::refractive_index::RefrIndexConst;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// Helper to write a Git tree recursively for a temporary remote repository.
    fn write_test_tree(repo: &gix::Repository, dir: &Path) -> OpmResult<gix::ObjectId> {
        let mut entries = Vec::new();
        let dir_entries = fs::read_dir(dir)
            .map_err(|e| OpossumError::Registry(format!("Failed to read test dir: {e}")))?;

        for entry_res in dir_entries {
            let entry = entry_res
                .map_err(|e| OpossumError::Registry(format!("Failed to read entry: {e}")))?;
            if entry.file_name() == ".git" {
                continue;
            }
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                let subtree_id = write_test_tree(repo, &path)?;
                entries.push(gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryKind::Tree.into(),
                    filename: file_name.into(),
                    oid: subtree_id,
                });
            } else if path.is_file() {
                let bytes = fs::read(&path)
                    .map_err(|e| OpossumError::Registry(format!("Failed to read file: {e}")))?;
                let blob_id = repo
                    .write_blob(&bytes)
                    .map_err(|e| OpossumError::Registry(format!("Failed to write blob: {e}")))?
                    .detach();
                entries.push(gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryKind::Blob.into(),
                    filename: file_name.into(),
                    oid: blob_id,
                });
            }
        }

        entries.sort_by(|a, b| {
            let a_name = if a.mode == gix::objs::tree::EntryKind::Tree.into() {
                format!("{}/", a.filename)
            } else {
                a.filename.to_string()
            };
            let b_name = if b.mode == gix::objs::tree::EntryKind::Tree.into() {
                format!("{}/", b.filename)
            } else {
                b.filename.to_string()
            };
            a_name.cmp(&b_name)
        });

        let tree = gix::objs::Tree { entries };
        let tree_id = repo
            .write_object(&tree)
            .map_err(|e| OpossumError::Registry(format!("Failed to write tree: {e}")))?
            .detach();
        Ok(tree_id)
    }

    /// Creates a simulated remote repository with an initial commit on `refs/heads/main`.
    fn init_test_remote(remote_dir: &Path) -> OpmResult<(Uuid, Uuid)> {
        let repo = gix::init(remote_dir)
            .map_err(|e| OpossumError::Registry(format!("Failed to init remote repo: {e}")))?;
        fs::write(
            repo.git_dir().join("HEAD"),
            format!("ref: refs/heads/{DEFAULT_BRANCH}\n"),
        )
        .map_err(|e| OpossumError::Registry(format!("Failed to set remote HEAD: {e}")))?;

        let remote_loader = AssetLoader::new(remote_dir);
        let mut mat_a = Material::new_draft(
            "N-BK7",
            Some("Schott".to_string()),
            Some("Official crown glass".to_string()),
            RefrIndexConst::new(1.5168)?.into(),
        );
        let id_a = mat_a.id();
        remote_loader.publish(&mut mat_a)?;

        let mut mat_b = Material::new_draft(
            "B270",
            Some("Schott".to_string()),
            Some("Official plate glass".to_string()),
            RefrIndexConst::new(1.5230)?.into(),
        );
        let id_b = mat_b.id();
        remote_loader.publish(&mut mat_b)?;

        let tree_id = write_test_tree(&repo, remote_dir)?;
        let sig = gix::actor::Signature {
            name: "Test Maintainer".into(),
            email: "test@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        };
        let commit = gix::objs::Commit {
            tree: tree_id,
            parents: Default::default(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            message: "Initial official catalog".into(),
            extra_headers: Vec::new(),
        };
        let commit_id = repo
            .write_object(&commit)
            .map_err(|e| OpossumError::Registry(format!("Failed to write commit: {e}")))?
            .detach();

        let ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        repo.reference(
            ref_name.as_str(),
            commit_id,
            gix::refs::transaction::PreviousValue::Any,
            "test: initial main commit",
        )
        .map_err(|e| OpossumError::Registry(format!("Failed to create main ref: {e}")))?;

        Ok((id_a, id_b))
    }

    #[test]
    fn test_dual_catalog_initialization_and_main_branch() -> OpmResult<()> {
        let temp_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let registry = AssetRegistry::<Material>::new(temp_dir.path())?;

        // Both official and user subdirectories must exist
        assert!(registry.official_path().exists());
        assert!(registry.user_path().exists());

        // Both repositories must point HEAD to refs/heads/main
        let official_head = fs::read_to_string(registry.official_path().join(".git/HEAD"))
            .map_err(|e| OpossumError::Registry(e.to_string()))?;
        let user_head = fs::read_to_string(registry.user_path().join(".git/HEAD"))
            .map_err(|e| OpossumError::Registry(e.to_string()))?;
        assert_eq!(official_head.trim(), "ref: refs/heads/main");
        assert_eq!(user_head.trim(), "ref: refs/heads/main");

        // Official catalog must default to DEFAULT_CATALOG_REMOTE_URL
        let official_cfg = fs::read_to_string(registry.official_path().join(".git/config"))
            .map_err(|e| OpossumError::Registry(e.to_string()))?;
        assert!(official_cfg.contains(DEFAULT_CATALOG_REMOTE_URL));
        Ok(())
    }

    #[test]
    fn test_copy_official_to_user_generates_new_uuid() -> OpmResult<()> {
        let remote_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let (official_id, _) = init_test_remote(remote_dir.path())?;

        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let official_path = reg_dir.path().join(OFFICIAL_CATALOG_SUBDIR);
        let remote_url = remote_dir.path().to_string_lossy().to_string();
        let sync = RegistrySync::new_official(&official_path, Some(&remote_url));
        sync.ensure_repository_initialized()?;
        sync.pull_updates()?;

        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(&remote_url))?;
        assert!(registry.is_official(official_id));
        assert!(!registry.in_user_catalog(official_id));

        let user_copy = registry.copy_to_user_catalog(
            CatalogSource::Official,
            official_id,
            None,
            Some("N-BK7 (Custom)".to_string()),
        )?;

        // Must have a distinct UUID and start at version 1 in the user catalog
        assert_ne!(user_copy.id(), official_id);
        assert_eq!(user_copy.version(), 1);
        assert!(registry.is_official(official_id));
        assert!(registry.in_user_catalog(user_copy.id()));
        assert!(!registry.is_official(user_copy.id()));
        Ok(())
    }

    #[test]
    fn test_official_pull_adopts_remote_and_prunes_removed_assets() -> OpmResult<()> {
        let remote_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let (id_a, id_b) = init_test_remote(remote_dir.path())?;

        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let remote_url = remote_dir.path().to_string_lossy().to_string();
        let official_path = reg_dir.path().join(OFFICIAL_CATALOG_SUBDIR);

        let sync = RegistrySync::new_official(&official_path, Some(&remote_url));
        sync.ensure_repository_initialized()?;
        sync.pull_updates()?;

        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(&remote_url))?;
        assert_eq!(registry.official_index().len(), 2);
        assert!(registry.official_index().get(&id_a).is_some());
        assert!(registry.official_index().get(&id_b).is_some());

        // Simulate a non-fast-forward remote update (e.g. format migration) that removes `id_b`
        // and updates `id_a` with a new refractive index
        let remote_repo = gix::open(remote_dir.path())
            .map_err(|e| OpossumError::Registry(format!("Failed to open remote repo: {e}")))?;
        let b_dir = remote_dir.path().join("materials").join(id_b.to_string());
        if b_dir.exists() {
            fs::remove_dir_all(&b_dir)
                .map_err(|e| OpossumError::Registry(format!("Failed to remove mat_b: {e}")))?;
        }
        let remote_loader = AssetLoader::new(remote_dir.path());
        let mut updated_a: Material = remote_loader.load(id_a, None)?;
        updated_a.optical.refractive_index = RefrIndexConst::new(1.5200)?.into();
        let a_file = remote_loader.asset_file_path::<Material>(id_a, updated_a.version());
        let pretty = ron::ser::PrettyConfig::default();
        let ron_str = ron::ser::to_string_pretty(&updated_a, pretty)
            .map_err(|e| OpossumError::Registry(format!("Failed to serialize updated_a: {e}")))?;
        fs::write(&a_file, ron_str)
            .map_err(|e| OpossumError::Registry(format!("Failed to write updated_a: {e}")))?;

        let new_tree_id = write_test_tree(&remote_repo, remote_dir.path())?;
        let sig = gix::actor::Signature {
            name: "Test Maintainer".into(),
            email: "test@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        };
        // Orphan/divergent commit to force non-fast-forward alignment
        let migrated_commit = gix::objs::Commit {
            tree: new_tree_id,
            parents: Default::default(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            message: "Format migration on remote main".into(),
            extra_headers: Vec::new(),
        };
        let migrated_commit_id = remote_repo
            .write_object(&migrated_commit)
            .map_err(|e| OpossumError::Registry(format!("Failed to write migrated commit: {e}")))?
            .detach();
        let ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        remote_repo
            .reference(
                ref_name.as_str(),
                migrated_commit_id,
                gix::refs::transaction::PreviousValue::Any,
                "test: force non-fast-forward update",
            )
            .map_err(|e| OpossumError::Registry(format!("Failed to update remote ref: {e}")))?;

        // Pull updates and rebuild index
        sync.pull_updates()?;
        registry.rebuild_index()?;

        // `id_b` must be pruned from disk and index, `id_a` must have updated nd = 1.5200
        assert!(
            !official_path
                .join("materials")
                .join(id_b.to_string())
                .exists()
        );
        assert_eq!(registry.official_index().len(), 1);
        let entry_a = registry.official_index().get(&id_a).expect("id_a present");
        assert_eq!(entry_a.specific.nd, Some(1.5200));
        Ok(())
    }
}
