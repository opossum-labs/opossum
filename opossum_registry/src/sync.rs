use opossum_core::error::{OpmResult, OpossumError};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use gix::progress::Discard;
use gix::remote::Direction;

/// Default branch name assumed for the registry repository.
const DEFAULT_BRANCH: &str = "main";

/// Handles synchronization of the local asset registry with a remote Git repository using pure Rust `gix`.
pub struct RegistrySync {
    /// The local path where the registry is stored.
    local_path: PathBuf,
    /// The URL of the remote Git repository.
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

    /// Ensures the local repository exists. If it does not exist, it clones the repository
    /// from the remote URL.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if cloning fails or the repository cannot be opened.
    pub fn init_or_clone(&self) -> OpmResult<()> {
        if self.local_path.join(".git").exists() {
            gix::open(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!("Failed to open existing registry repository: {e}"))
            })?;
        } else {
            let should_interrupt = AtomicBool::new(false);

            let mut prepare_clone = gix::prepare_clone(self.remote_url.as_str(), &self.local_path)
                .map_err(|e| {
                    OpossumError::Registry(format!(
                        "Failed to prepare clone from {}: {e}",
                        self.remote_url
                    ))
                })?;

            let (mut prepare_checkout, _fetch_outcome) = prepare_clone
                .fetch_then_checkout(Discard, &should_interrupt)
                .map_err(|e| {
                    OpossumError::Registry(format!(
                        "Failed to fetch repository from {}: {e}",
                        self.remote_url
                    ))
                })?;

            let (_repo, _checkout_outcome) = prepare_checkout
                .main_worktree(Discard, &should_interrupt)
                .map_err(|e| {
                    OpossumError::Registry(format!("Failed to checkout main worktree: {e}"))
                })?;
        }

        Ok(())
    }

    /// Pulls the latest changes from the remote repository via Fast-Forward.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if fetching fails, branches have diverged, or checkout fails.
    pub fn pull_updates(&self) -> OpmResult<()> {
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

        let connection = remote
            .connect(Direction::Fetch)
            .map_err(|e| OpossumError::Registry(format!("Failed to connect to remote: {e}")))?;

        let _outcome = connection
            .prepare_fetch(Discard, gix::remote::ref_map::Options::default())
            .map_err(|e| OpossumError::Registry(format!("Failed to prepare fetch: {e}")))?
            .receive(Discard, &should_interrupt)
            .map_err(|e| OpossumError::Registry(format!("Failed to receive remote pack: {e}")))?;

        let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        let remote_ref_name = format!("refs/remotes/origin/{DEFAULT_BRANCH}");

        let local_ref = repo.find_reference(&local_ref_name).map_err(|e| {
            OpossumError::Registry(format!("Local branch '{DEFAULT_BRANCH}' not found: {e}"))
        })?;
        let local_commit_id = local_ref.into_fully_peeled_id().map_err(|e| {
            OpossumError::Registry(format!("Failed to resolve local commit ID: {e}"))
        })?;

        let remote_ref = repo.find_reference(&remote_ref_name).map_err(|e| {
            OpossumError::Registry(format!(
                "Remote tracking ref '{remote_ref_name}' not found: {e}"
            ))
        })?;
        let remote_commit_id = remote_ref.into_fully_peeled_id().map_err(|e| {
            OpossumError::Registry(format!("Failed to resolve remote commit ID: {e}"))
        })?;

        if local_commit_id == remote_commit_id {
            return Ok(());
        }

        let is_fast_forward = repo
            .merge_base(local_commit_id, remote_commit_id)
            .is_ok_and(|base| base == local_commit_id);

        if !is_fast_forward {
            return Err(OpossumError::Registry(
                "Local branch has diverged from remote. Automated merge aborted to protect local changes.".to_string(),
            ));
        }

        repo.reference(
            local_ref_name.as_str(),
            remote_commit_id.detach(),
            gix::refs::transaction::PreviousValue::MustExistAndMatch(
                local_commit_id.detach().into(),
            ),
            "registry: fast-forward update",
        )
        .map_err(|e| {
            OpossumError::Registry(format!("Failed to update local branch reference: {e}"))
        })?;

        let worktree = repo.worktree().ok_or_else(|| {
            OpossumError::Registry("Cannot checkout files in a bare repository".into())
        })?;
        let worktree_dir = worktree.base();

        let tree_id = remote_commit_id
            .object()
            .map_err(|e| OpossumError::Registry(format!("Failed to load commit object: {e}")))?
            .peel_to_tree()
            .map_err(|e| OpossumError::Registry(format!("Failed to peel tree: {e}")))?
            .id;

        let mut index = repo
            .index_from_tree(&tree_id)
            .map_err(|e| OpossumError::Registry(format!("Failed to build index from tree: {e}")))?;

        let opts = gix::worktree::state::checkout::Options {
            overwrite_existing: true,
            ..Default::default()
        };

        gix::worktree::state::checkout(
            &mut index,
            worktree_dir,
            repo.objects.clone(),
            &Discard,
            &Discard,
            &should_interrupt,
            opts,
        )
        .map_err(|e| {
            OpossumError::Registry(format!("Failed to checkout updated working tree: {e}"))
        })?;

        index
            .write(gix::index::write::Options::default())
            .map_err(|e| OpossumError::Registry(format!("Failed to write index file: {e}")))?;

        Ok(())
    }
}
