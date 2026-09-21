//! Git synchronization module for the OPOSSUM asset registry.

mod commit;
mod config;
mod rebase;
mod utils;

pub use commit::commit_asset_file;

use gix::{objs::Commit, progress::Discard, remote::Direction};
use opossum_core::error::{OpmResult, OpossumError};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

use self::{
    commit::upsert_path_in_tree,
    config::ensure_remote_configured,
    rebase::rebase_local_assets_onto_remote,
    utils::{
        checkout_worktree, clone_repository, collect_files_recursive, directory_contains_assets,
        format_error_chain, is_directory_empty, resolve_signature,
    },
};

/// Default branch name assumed for the registry repository.
pub const DEFAULT_BRANCH: &str = "main";

/// Handles synchronization of the local asset registry with a remote Git repository using pure Rust `gix`.
pub struct RegistrySync {
    local_path: PathBuf,
    remote_url: String,
}

impl RegistrySync {
    /// Creates a new synchronization handler.
    pub fn new(local_path: impl Into<PathBuf>, remote_url: impl Into<String>) -> Self {
        Self {
            local_path: local_path.into(),
            remote_url: remote_url.into(),
        }
    }

    /// Ensures that the catalog directory is a properly initialized Git repository.
    ///
    /// # Errors
    ///
    /// This function returns an error if an underlying git opreatin fails
    pub fn ensure_repository_initialized(&self) -> OpmResult<()> {
        if !self.local_path.exists() {
            fs::create_dir_all(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to create catalog root folder {}: {e}",
                    self.local_path.display()
                ))
            })?;
        }

        // 1. If already a git repository, verify integrity and ensure remote config is up to date
        if self.local_path.join(".git").exists() {
            gix::open(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!("Failed to open existing repository: {e}"))
            })?;

            // Keep remote configuration in sync even if the repository already existed
            if !self.remote_url.trim().is_empty() {
                ensure_remote_configured(&self.local_path, &self.remote_url)?;
            }
            return Ok(());
        }

        // 2. Fresh installation: clone if empty and remote URL provided
        if is_directory_empty(&self.local_path) && !self.remote_url.trim().is_empty() {
            return clone_repository(&self.remote_url, &self.local_path);
        }

        // 3. In-place initialization
        let repo = gix::init(&self.local_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to initialize git repository at {}: {e}",
                self.local_path.display()
            ))
        })?;
        // 4. Create initial commit if existing assets are found on disk
        if directory_contains_assets(&self.local_path) {
            let files = collect_files_recursive(&self.local_path)?;
            let mut root_tree_oid = None;

            for file_path in files {
                if let Ok(rel_path) = file_path.strip_prefix(&self.local_path) {
                    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
                    let segments: Vec<&str> = rel_path_str.split('/').collect();

                    if let Ok(file_bytes) = fs::read(&file_path)
                        && let Ok(blob) = repo.write_blob(&file_bytes)
                    {
                        root_tree_oid = Some(upsert_path_in_tree(
                            &repo,
                            root_tree_oid,
                            &segments,
                            blob.detach(),
                        )?);
                    }
                }
            }

            if let Some(tree_oid) = root_tree_oid {
                let author = resolve_signature(&repo);
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
                    .map_err(|e| {
                        OpossumError::Registry(format!("Failed to write initial commit: {e}"))
                    })?
                    .detach();

                let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
                repo.reference(
                    local_ref_name.as_str(),
                    commit_id,
                    gix::refs::transaction::PreviousValue::MustNotExist,
                    "registry: initial repository setup",
                )
                .map_err(|e| {
                    OpossumError::Registry(format!("Failed to create default branch: {e}"))
                })?;

                let mut index = repo.index_from_tree(&tree_oid).map_err(|e| {
                    OpossumError::Registry(format!("Failed to build initial index: {e}"))
                })?;

                index
                    .write(gix::index::write::Options::default())
                    .map_err(|e| {
                        OpossumError::Registry(format!("Failed to write initial index: {e}"))
                    })?;
            }
        }

        // 5. Ensure remote tracking branch is configured
        if !self.remote_url.trim().is_empty() {
            ensure_remote_configured(&self.local_path, &self.remote_url)?;
        }
        Ok(())
    }

    /// Pulls the latest changes from the remote repository via Fast-Forward or local Rebase.
    ///
    /// # Errors
    ///
    /// This functions retursn an error if an underlying git operation fails
    pub fn pull_updates(&self) -> OpmResult<()> {
        ensure_remote_configured(&self.local_path, &self.remote_url)?;

        let repo = gix::open(&self.local_path).map_err(|e| {
            OpossumError::Registry(format!("Failed to open registry repository: {e}"))
        })?;

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
                return Ok(());
            }

            let is_fast_forward = repo
                .merge_base(local_cid, remote_commit_id)
                .is_ok_and(|base| base == local_cid);

            if is_fast_forward {
                repo.reference(
                    local_ref_name.as_str(),
                    remote_commit_id.detach(),
                    gix::refs::transaction::PreviousValue::MustExistAndMatch(
                        local_cid.detach().into(),
                    ),
                    "registry: fast-forward update",
                )
                .map_err(|e| {
                    OpossumError::Registry(format!("Failed to update local branch reference: {e}"))
                })?;

                checkout_worktree(&repo, remote_commit_id)?;
                return Ok(());
            }

            let has_local_assets = directory_contains_assets(&self.local_path);

            if !has_local_assets {
                repo.reference(
                    local_ref_name.as_str(),
                    remote_commit_id.detach(),
                    gix::refs::transaction::PreviousValue::MustExistAndMatch(
                        local_cid.detach().into(),
                    ),
                    "registry: adopt remote catalog (no local assets)",
                )
                .map_err(|e| {
                    OpossumError::Registry(format!("Failed to align branch with remote: {e}"))
                })?;

                checkout_worktree(&repo, remote_commit_id)?;
                return Ok(());
            }

            rebase_local_assets_onto_remote(&self.local_path, &repo, remote_commit_id)?;
        } else {
            repo.reference(
                local_ref_name.as_str(),
                remote_commit_id.detach(),
                gix::refs::transaction::PreviousValue::MustNotExist,
                "registry: initial checkout from remote",
            )
            .map_err(|e| {
                OpossumError::Registry(format!("Failed to initialize local branch: {e}"))
            })?;

            checkout_worktree(&repo, remote_commit_id)?;
        }

        Ok(())
    }
}
