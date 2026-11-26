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

    /// Prepares an editable draft (`version = 0`) targeting the user catalog:
    /// - If `id` belongs to the `Official` catalog, clones the asset with a brand-new UUID
    ///   so official UUIDs never collide with user catalog assets.
    /// - If `id` belongs to the `User` catalog, preserves the existing UUID and resets
    ///   `version = 0` so publishing creates the next incremental version (`v2`, `v3`, ...).
    ///
    /// # Errors
    /// Returns an error if `id` is not found in either catalog or cannot be loaded.
    pub fn prepare_user_draft(&self, id: Uuid, version: Option<u32>) -> OpmResult<T> {
        if self.is_official(id) {
            let mut forked: T = self.official_loader.load(id, version)?;
            forked.header_mut().id = Uuid::new_v4();
            forked.header_mut().version = 0;
            Ok(forked)
        } else if self.in_user_catalog(id) {
            let mut next_ver_draft: T = self.user_loader.load(id, version)?;
            next_ver_draft.header_mut().version = 0;
            Ok(next_ver_draft)
        } else {
            Err(OpossumError::Registry(format!(
                "Cannot prepare user draft: asset UUID {id} not found in official or user catalog"
            )))
        }
    }

    /// Publishes an asset draft into the user catalog (`<root>/user`), updates the user index,
    /// and commits locally to the user catalog's `main` branch.
    ///
    /// # Errors
    /// Returns an error if the asset UUID collides with an official catalog asset,
    /// or if writing, indexing, or local Git commit fails.
    pub fn publish(&mut self, asset: &mut T) -> OpmResult<PathBuf> {
        self.publish_to(CatalogSource::User, asset)
    }

    /// Publishes an asset draft to the target catalog (`User` or `Official`), updates its index,
    /// and commits to that repository's `main` branch if initialized.
    ///
    /// Enforces the invariant that an asset sharing a UUID with an `Official` catalog entry
    /// cannot be published into `CatalogSource::User`.
    ///
    /// # Errors
    /// Returns an error if invariant validation, writing, indexing, or Git commit fails.
    pub fn publish_to(&mut self, source: CatalogSource, asset: &mut T) -> OpmResult<PathBuf> {
        if source == CatalogSource::User && self.is_official(asset.id()) {
            return Err(OpossumError::Registry(format!(
                "Cannot publish asset '{}' ({}) into user catalog: UUID belongs to the official catalog. Fork it first via prepare_user_draft or copy_to_user_catalog.",
                asset.name(),
                asset.id()
            )));
        }

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
        let mut copied: T = self.loader_for(source).load(id, version)?;
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
    /// Deleting from `CatalogSource::Official` is forbidden because the official catalog
    /// is read-only/synchronized from upstream.
    ///
    /// # Errors
    /// Returns an error if attempting to delete from `CatalogSource::Official`,
    /// or if file deletion or index reversion fails.
    pub fn delete_latest_version_from(
        &mut self,
        source: CatalogSource,
        id: Uuid,
    ) -> OpmResult<Option<u32>> {
        if source == CatalogSource::Official {
            return Err(OpossumError::Registry(format!(
                "Cannot delete asset {id} from official catalog: official assets are read-only."
            )));
        }
        let remaining_latest = self.user_loader.delete_latest_version::<T>(id)?;
        self.user_index
            .remove_or_revert_entry(id, remaining_latest, &self.user_loader)?;
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod dual_catalog_tests {
    use super::*;
    use crate::{
        asset::RegisterableAsset,
        sync::{DEFAULT_BRANCH, RegistrySync},
    };
    use gix::objs::{
        Commit, Tree,
        tree::{Entry, EntryKind},
    };
    use opossum_core::{material::Material, refractive_index::RefrIndexConst};
    use std::fs;
    use tempfile::TempDir;

    /// Recursively writes a directory tree (excluding `.git`) into `repo` and returns the root tree OID.
    fn write_test_tree(repo: &gix::Repository, dir: &Path) -> OpmResult<gix::ObjectId> {
        let mut entries = Vec::new();
        let read_dir = fs::read_dir(dir).map_err(|e| OpossumError::Registry(e.to_string()))?;

        for entry_res in read_dir {
            let entry = entry_res.map_err(|e| OpossumError::Registry(e.to_string()))?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name == ".git" {
                continue;
            }
            if path.is_dir() {
                let child_oid = write_test_tree(repo, &path)?;
                entries.push(Entry {
                    mode: EntryKind::Tree.into(),
                    filename: file_name.into(),
                    oid: child_oid,
                });
            } else if path.is_file() {
                let bytes = fs::read(&path).map_err(|e| OpossumError::Registry(e.to_string()))?;
                let blob_oid = repo
                    .write_blob(&bytes)
                    .map_err(|e| OpossumError::Registry(e.to_string()))?
                    .detach();
                entries.push(Entry {
                    mode: EntryKind::Blob.into(),
                    filename: file_name.into(),
                    oid: blob_oid,
                });
            }
        }

        entries.sort();
        let tree = Tree { entries };
        let tree_oid = repo
            .write_object(&tree)
            .map_err(|e| OpossumError::Registry(e.to_string()))?
            .detach();
        Ok(tree_oid)
    }

    /// Initializes a local Git remote repository with two official materials (`N-BK7` and `SF11`) on `main`.
    fn init_test_remote(remote_path: &Path) -> OpmResult<(Uuid, Uuid)> {
        let repo = gix::init(remote_path)
            .map_err(|e| OpossumError::Registry(format!("Failed to init remote repo: {e}")))?;
        let head_file = remote_path.join(".git").join("HEAD");
        fs::write(&head_file, format!("ref: refs/heads/{DEFAULT_BRANCH}\n"))
            .map_err(|e| OpossumError::Registry(format!("Failed to write HEAD: {e}")))?;

        let loader = AssetLoader::new(remote_path);
        let mut mat_a = Material::new_draft(
            "N-BK7",
            Some("Schott".to_string()),
            Some("Standard borosilicate crown glass".to_string()),
            RefrIndexConst::new(1.5168)?.into(),
        );
        loader.publish(&mut mat_a)?;

        let mut mat_b = Material::new_draft(
            "SF11",
            Some("Schott".to_string()),
            Some("Dense flint glass".to_string()),
            RefrIndexConst::new(1.7847)?.into(),
        );
        loader.publish(&mut mat_b)?;

        let tree_id = write_test_tree(&repo, remote_path)?;
        let sig = gix::actor::Signature {
            name: "Test Maintainer".into(),
            email: "maintainer@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        };
        let commit = Commit {
            tree: tree_id,
            parents: Default::default(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            message: "Initial official catalog commit".into(),
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
            "test: initial commit",
        )
        .map_err(|e| OpossumError::Registry(format!("Failed to create branch ref: {e}")))?;

        let mut index = repo
            .index_from_tree(&tree_id)
            .map_err(|e| OpossumError::Registry(format!("Failed to build index: {e}")))?;
        index
            .write(gix::index::write::Options::default())
            .map_err(|e| OpossumError::Registry(format!("Failed to write index: {e}")))?;

        Ok((mat_a.id(), mat_b.id()))
    }

    #[test]
    fn test_dual_catalog_initialization_and_main_branch() -> OpmResult<()> {
        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(""))?;

        assert!(registry.official_path().join(".git").exists());
        assert!(registry.user_path().join(".git").exists());

        let official_head = fs::read_to_string(registry.official_path().join(".git/HEAD"))
            .map_err(|e| OpossumError::Registry(e.to_string()))?;
        let user_head = fs::read_to_string(registry.user_path().join(".git/HEAD"))
            .map_err(|e| OpossumError::Registry(e.to_string()))?;
        assert_eq!(
            official_head.trim(),
            format!("ref: refs/heads/{DEFAULT_BRANCH}")
        );
        assert_eq!(
            user_head.trim(),
            format!("ref: refs/heads/{DEFAULT_BRANCH}")
        );

        let mut custom = Material::new_draft(
            "Custom Glass",
            Some("Lab".to_string()),
            None,
            RefrIndexConst::new(1.6000)?.into(),
        );
        registry.publish(&mut custom)?;

        assert!(registry.in_user_catalog(custom.id()));
        assert!(!registry.is_official(custom.id()));
        assert_eq!(custom.version(), 1);
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

    #[test]
    fn test_prepare_user_draft_forks_official_and_increments_user_version() -> OpmResult<()> {
        let remote_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let (official_id, _) = init_test_remote(remote_dir.path())?;

        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let remote_url = remote_dir.path().to_string_lossy().to_string();
        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(&remote_url))?;

        // 1. Preparing a draft from an official material must generate a new UUID and version 0
        let mut forked_draft = registry.prepare_user_draft(official_id, None)?;
        assert_ne!(forked_draft.id(), official_id);
        assert_eq!(forked_draft.version(), 0);

        let user_id = forked_draft.id();
        registry.publish(&mut forked_draft)?;
        assert_eq!(forked_draft.version(), 1);
        assert!(registry.is_official(official_id));
        assert!(registry.in_user_catalog(user_id));

        // 2. Preparing a draft from a user material must keep the SAME UUID and reset version to 0
        let mut next_ver_draft = registry.prepare_user_draft(user_id, None)?;
        assert_eq!(next_ver_draft.id(), user_id);
        assert_eq!(next_ver_draft.version(), 0);

        next_ver_draft.optical.refractive_index = RefrIndexConst::new(1.5500)?.into();
        registry.publish(&mut next_ver_draft)?;
        assert_eq!(next_ver_draft.version(), 2);

        let user_entry = registry
            .user_index()
            .get(&user_id)
            .expect("user material indexed");
        assert_eq!(user_entry.common.latest_version, 2);
        assert_eq!(user_entry.common.available_versions, vec![1, 2]);
        Ok(())
    }

    #[test]
    fn test_publish_to_user_rejects_official_uuid() -> OpmResult<()> {
        let remote_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let (official_id, _) = init_test_remote(remote_dir.path())?;

        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let remote_url = remote_dir.path().to_string_lossy().to_string();
        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(&remote_url))?;

        let mut collision_draft = registry.load_from(CatalogSource::Official, official_id, None)?;
        collision_draft.header_mut().version = 0;

        let res = registry.publish_to(CatalogSource::User, &mut collision_draft);
        assert!(
            res.is_err(),
            "Publishing an official UUID into the user catalog must fail"
        );
        assert!(!registry.in_user_catalog(official_id));
        Ok(())
    }

    #[test]
    fn test_delete_from_official_catalog_is_forbidden() -> OpmResult<()> {
        let remote_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let (official_id, _) = init_test_remote(remote_dir.path())?;

        let reg_dir = TempDir::new().map_err(|e| OpossumError::Registry(e.to_string()))?;
        let remote_url = remote_dir.path().to_string_lossy().to_string();
        let mut registry =
            AssetRegistry::<Material>::with_official_remote(reg_dir.path(), Some(&remote_url))?;

        let del_res = registry.delete_latest_version_from(CatalogSource::Official, official_id);
        assert!(
            del_res.is_err(),
            "Deleting an asset from the official catalog must be forbidden"
        );
        assert!(registry.is_official(official_id));
        Ok(())
    }
}
