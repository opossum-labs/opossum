//! Git synchronization module for the OPOSSUM asset registry.
mod commit;
mod config;
mod utils;

use self::{
    commit::upsert_path_in_tree,
    config::{ensure_committer_configured, ensure_head_points_to_main, ensure_remote_configured},
    utils::{
        checkout_worktree, clone_repository, collect_files_recursive, directory_contains_assets,
        format_error_chain, is_asset_file, is_directory_empty, open_registry_repo,
        resolve_signature,
    },
};
pub use commit::commit_asset_file;
use gix::{Repository, objs::Commit, progress::Discard, remote::Direction};
use opossum_core::error::{OpmResult, OpossumError};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

/// Default branch name assumed for registry repositories (`main` avoids legacy `master` defaults).
pub const DEFAULT_BRANCH: &str = "main";

/// Default remote Git repository URL for the shared official OPOSSUM catalog.
pub const DEFAULT_CATALOG_REMOTE_URL: &str = "https://github.com/opossum-labs/opossum_catalog.git";

/// Distinguishes between the shared official catalog and the local user catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogRole {
    /// Synchronized from the shared remote repository (`origin/main`).
    Official,
    /// Local user catalog tracked on a local `main` branch without remote pull/rebase.
    User,
}

/// Handles synchronization of a catalog directory using pure Rust `gix`.
pub struct RegistrySync {
    local_path: PathBuf,
    remote_url: String,
    role: CatalogRole,
}

impl RegistrySync {
    /// Creates a synchronization handler for the official catalog with the default remote URL.
    #[must_use]
    pub fn new_official(local_path: impl Into<PathBuf>, remote_url: Option<&str>) -> Self {
        let resolved_url = remote_url
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CATALOG_REMOTE_URL)
            .to_string();
        Self {
            local_path: local_path.into(),
            remote_url: resolved_url,
            role: CatalogRole::Official,
        }
    }

    /// Creates a synchronization handler for a local user catalog (no remote sync).
    #[must_use]
    pub fn new_user(local_path: impl Into<PathBuf>) -> Self {
        Self {
            local_path: local_path.into(),
            remote_url: String::new(),
            role: CatalogRole::User,
        }
    }

    /// Creates a synchronization handler (defaults to `CatalogRole::Official`).
    pub fn new(local_path: impl Into<PathBuf>, remote_url: impl Into<String>) -> Self {
        let url_str = remote_url.into();
        let trimmed = url_str.trim();
        let resolved_url = if trimmed.is_empty() {
            DEFAULT_CATALOG_REMOTE_URL.to_string()
        } else {
            trimmed.to_string()
        };
        Self {
            local_path: local_path.into(),
            remote_url: resolved_url,
            role: CatalogRole::Official,
        }
    }

    /// Ensures that the catalog directory is initialized as a Git repository on branch `main`.
    ///
    /// # Errors
    ///
    /// This function returns an error if an underlying git operation fails.
    pub fn ensure_repository_initialized(&self) -> OpmResult<()> {
        if !self.local_path.exists() {
            fs::create_dir_all(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to create catalog root folder {}: {e}",
                    self.local_path.display()
                ))
            })?;
        }

        // 1. Existing repository: ensure HEAD -> main and user/remote configuration
        if self.local_path.join(".git").exists() {
            let _repo = open_registry_repo(&self.local_path)?;
            ensure_head_points_to_main(&self.local_path)?;
            if self.role == CatalogRole::Official && !self.remote_url.trim().is_empty() {
                ensure_remote_configured(&self.local_path, &self.remote_url)?;
            }
            ensure_committer_configured(&self.local_path)?;
            return Ok(());
        }

        // 2. Fresh official catalog on an empty directory: clone from remote URL
        if self.role == CatalogRole::Official
            && is_directory_empty(&self.local_path)
            && !self.remote_url.trim().is_empty()
        {
            clone_repository(&self.remote_url, &self.local_path)?;
            ensure_head_points_to_main(&self.local_path)?;
            ensure_committer_configured(&self.local_path)?;
            return Ok(());
        }

        // 3. In-place initialization explicitly targeting `main`
        let repo = gix::init(&self.local_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to initialize git repository at {}: {e}",
                self.local_path.display()
            ))
        })?;
        ensure_head_points_to_main(&self.local_path)?;
        ensure_committer_configured(&self.local_path)?;

        // 4. Create initial commit on `main` if existing assets are present on disk
        if directory_contains_assets(&self.local_path) {
            self.commit_existing_assets(&repo)?;
        }

        // 5. Ensure `origin` remote is configured for official catalogs
        if self.role == CatalogRole::Official && !self.remote_url.trim().is_empty() {
            ensure_remote_configured(&self.local_path, &self.remote_url)?;
        }
        Ok(())
    }

    fn commit_existing_assets(&self, repo: &Repository) -> OpmResult<()> {
        let files = collect_files_recursive(&self.local_path)?;
        let mut root_tree_oid = None;
        for file_path in files {
            if let Ok(rel_path) = file_path.strip_prefix(&self.local_path)
                && is_asset_file(rel_path)
            {
                let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
                let segments: Vec<&str> = rel_path_str
                    .split('/')
                    .filter(|segment| !segment.is_empty())
                    .collect();
                if let Ok(file_bytes) = fs::read(&file_path)
                    && let Ok(blob) = repo.write_blob(&file_bytes)
                {
                    root_tree_oid = Some(upsert_path_in_tree(
                        repo,
                        root_tree_oid,
                        &segments,
                        blob.detach(),
                    )?);
                }
            }
        }
        let Some(tree_oid) = root_tree_oid else {
            return Ok(());
        };
        let author = resolve_signature(repo);
        let committer = author.clone();
        let commit = Commit {
            tree: tree_oid,
            #[allow(clippy::default_trait_access)]
            parents: Default::default(),
            author,
            committer,
            encoding: None,
            message: "Initial catalog commit: import existing assets".into(),
            extra_headers: Vec::new(),
        };
        let commit_id = repo
            .write_object(&commit)
            .map_err(|e| OpossumError::Registry(format!("Failed to write initial commit: {e}")))?
            .detach();
        let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        repo.reference(
            local_ref_name.as_str(),
            commit_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "registry: initial main branch setup",
        )
        .map_err(|e| {
            OpossumError::Registry(format!("Failed to create branch '{DEFAULT_BRANCH}': {e}"))
        })?;
        let mut index = repo
            .index_from_tree(&tree_oid)
            .map_err(|e| OpossumError::Registry(format!("Failed to build initial index: {e}")))?;
        index
            .write(gix::index::write::Options::default())
            .map_err(|e| OpossumError::Registry(format!("Failed to write initial index: {e}")))?;
        Ok(())
    }

    /// Pulls the latest changes from `origin/main` for the official catalog.
    ///
    /// For `CatalogRole::User`, this is a no-op because user catalogs are strictly local.
    /// For `CatalogRole::Official`, if history has diverged (e.g. after a format migration or
    /// force-push on `origin/main`), local `refs/heads/main` is aligned to `remote_commit_id`
    /// and the worktree is pruned/checked out so stale/old-format files do not linger.
    ///
    /// # Errors
    ///
    /// This function returns an error if an underlying git operation fails.
    pub fn pull_updates(&self) -> OpmResult<()> {
        if self.role == CatalogRole::User {
            ensure_head_points_to_main(&self.local_path)?;
            ensure_committer_configured(&self.local_path)?;
            return Ok(());
        }

        ensure_remote_configured(&self.local_path, &self.remote_url)?;
        ensure_head_points_to_main(&self.local_path)?;
        ensure_committer_configured(&self.local_path)?;

        let repo = open_registry_repo(&self.local_path)?;
        let should_interrupt = AtomicBool::new(false);
        let remote = match repo.find_default_remote(Direction::Fetch) {
            Some(Ok(r)) => r,
            _ => repo.find_remote("origin").map_err(|e| {
                OpossumError::Registry(format!("Failed to find remote 'origin': {e}"))
            })?,
        };
        let connection = remote.connect(Direction::Fetch).map_err(|e| {
            OpossumError::Registry(format_error_chain("Failed to connect to remote", &e))
        })?;
        let _outcome = connection
            .prepare_fetch(Discard, gix::remote::ref_map::Options::default())
            .map_err(|e| OpossumError::Registry(format_error_chain("Failed to prepare fetch", &e)))?
            .receive(Discard, &should_interrupt)
            .map_err(|e| {
                OpossumError::Registry(format_error_chain("Failed to receive remote pack", &e))
            })?;

        let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        let remote_ref_name = format!("refs/remotes/origin/{DEFAULT_BRANCH}");
        let remote_ref = repo.find_reference(&remote_ref_name).map_err(|e| {
            OpossumError::Registry(format!(
                "Remote tracking branch '{remote_ref_name}' not found: {e}. Ensure remote repository contains commits on '{DEFAULT_BRANCH}'."
            ))
        })?;
        let remote_commit_id = remote_ref.into_fully_peeled_id().map_err(|e| {
            OpossumError::Registry(format!("Failed to resolve remote commit ID: {e}"))
        })?;
        let local_commit_id = match repo.find_reference(&local_ref_name) {
            Ok(local_ref) => Some(local_ref.into_fully_peeled_id().map_err(|e| {
                OpossumError::Registry(format!("Failed to resolve local commit ID: {e}"))
            })?),
            Err(_) => None,
        };

        if let Some(local_cid) = local_commit_id {
            if local_cid == remote_commit_id {
                checkout_worktree(&repo, remote_commit_id)?;
                return Ok(());
            }

            let is_fast_forward = repo
                .merge_base(local_cid, remote_commit_id)
                .is_ok_and(|base| base == local_cid);
            let reason = if is_fast_forward {
                "registry: fast-forward update from origin/main"
            } else {
                "registry: align official catalog with origin/main"
            };

            repo.reference(
                local_ref_name.as_str(),
                remote_commit_id.detach(),
                gix::refs::transaction::PreviousValue::MustExistAndMatch(local_cid.detach().into()),
                reason,
            )
            .map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to update local '{DEFAULT_BRANCH}' branch: {e}"
                ))
            })?;
        } else {
            repo.reference(
                local_ref_name.as_str(),
                remote_commit_id.detach(),
                gix::refs::transaction::PreviousValue::MustNotExist,
                "registry: initial checkout from origin/main",
            )
            .map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to initialize '{DEFAULT_BRANCH}' branch: {e}"
                ))
            })?;
        }
        checkout_worktree(&repo, remote_commit_id)?;
        Ok(())
    }
}
