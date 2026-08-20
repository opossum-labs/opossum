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
    /// Returns an [`OpossumError`] if cloning fails or the repository cannot be opened.
    pub fn init_or_clone(&self) -> OpmResult<()> {
        if self.local_path.join(".git").exists() {
            // Ensure the existing repository can be opened
            gix::open(&self.local_path).map_err(|e| {
                OpossumError::Other(format!("Failed to open existing registry repository: {e}"))
            })?;
        } else {
            // Cancellation flag for long-running operations (false = continue execution)
            let should_interrupt = AtomicBool::new(false);

            // 1. Prepare repository directory and configure remote
            let mut prepare_clone = gix::prepare_clone(self.remote_url.as_str(), &self.local_path)
                .map_err(|e| {
                    OpossumError::Other(format!(
                        "Failed to prepare clone from {}: {e}",
                        self.remote_url
                    ))
                })?;

            // 2. Fetch packfiles and metadata from the remote
            let (mut prepare_checkout, _fetch_outcome) = prepare_clone
                .fetch_then_checkout(Discard, &should_interrupt)
                .map_err(|e| {
                    OpossumError::Other(format!(
                        "Failed to fetch repository from {}: {e}",
                        self.remote_url
                    ))
                })?;

            // 3. Check out the main worktree into the target folder
            let (_repo, _checkout_outcome) = prepare_checkout
                .main_worktree(Discard, &should_interrupt)
                .map_err(|e| {
                    OpossumError::Other(format!("Failed to checkout main worktree: {e}"))
                })?;
        }

        Ok(())
    }

    /// Pulls the latest changes from the remote repository via Fast-Forward.
    ///
    /// Fetches changes from remote and updates the local branch reference.
    /// If local diverged commits exist, the update is aborted to protect local modifications.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if fetching fails, branches have diverged, or checkout fails.
    pub fn pull_updates(&self) -> OpmResult<()> {
        let repo = gix::open(&self.local_path)
            .map_err(|e| OpossumError::Other(format!("Failed to open registry repository: {e}")))?;

        let should_interrupt = AtomicBool::new(false);

        // 1. Find remote 'origin' or the default configured remote
        let remote = match repo.find_default_remote(Direction::Fetch) {
            Some(Ok(r)) => r,
            _ => repo.find_remote("origin").map_err(|e| {
                OpossumError::Other(format!("Failed to find remote 'origin': {e}"))
            })?,
        };

        // 2. Connect and fetch remote refs and objects
        let connection = remote
            .connect(Direction::Fetch)
            .map_err(|e| OpossumError::Other(format!("Failed to connect to remote: {e}")))?;

        let _outcome = connection
            .prepare_fetch(Discard, Default::default())
            .map_err(|e| OpossumError::Other(format!("Failed to prepare fetch: {e}")))?
            .receive(Discard, &should_interrupt)
            .map_err(|e| OpossumError::Other(format!("Failed to receive remote pack: {e}")))?;

        // 3. Resolve local branch and remote-tracking branch references
        let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        let remote_ref_name = format!("refs/remotes/origin/{DEFAULT_BRANCH}");

        let local_ref = repo
            .find_reference(&local_ref_name)
            .map_err(|e| OpossumError::Other(format!("Local branch '{DEFAULT_BRANCH}' not found: {e}")))?;
        let local_commit_id = local_ref
            .into_fully_peeled_id()
            .map_err(|e| OpossumError::Other(format!("Failed to resolve local commit ID: {e}")))?;

        let remote_ref = repo
            .find_reference(&remote_ref_name)
            .map_err(|e| OpossumError::Other(format!("Remote tracking ref '{remote_ref_name}' not found: {e}")))?;
        let remote_commit_id = remote_ref
            .into_fully_peeled_id()
            .map_err(|e| OpossumError::Other(format!("Failed to resolve remote commit ID: {e}")))?;

        // Check if local branch is already up-to-date
        if local_commit_id == remote_commit_id {
            return Ok(());
        }

        // 4. Verify fast-forward compatibility (local commit must be an ancestor of remote commit)
        let is_fast_forward = repo
            .merge_base(local_commit_id, remote_commit_id)
            .map(|base| base == local_commit_id)
            .unwrap_or(false);

        if !is_fast_forward {
            return Err(OpossumError::Other(
                "Local branch has diverged from remote. Automated merge aborted to protect local changes.".to_string(),
            ));
        }

        // 5. Update local branch reference to match remote commit ID
        repo.reference(
            local_ref_name.as_str(),
            remote_commit_id.detach(),
            gix::refs::transaction::PreviousValue::MustExistAndMatch(local_commit_id.detach().into()),
            "registry: fast-forward update",
        )
        .map_err(|e| OpossumError::Other(format!("Failed to update local branch reference: {e}")))?;

        // 6. Check out new index and files into working directory
        let worktree = repo
            .worktree()
            .ok_or_else(|| OpossumError::Other("Cannot checkout files in a bare repository".into()))?;
        let worktree_dir = worktree.base();

        let tree_id = remote_commit_id
            .object()
            .map_err(|e| OpossumError::Other(format!("Failed to load commit object: {e}")))?
            .peel_to_tree()
            .map_err(|e| OpossumError::Other(format!("Failed to peel tree: {e}")))?
            .id;

        let mut index = repo
            .index_from_tree(&tree_id)
            .map_err(|e| OpossumError::Other(format!("Failed to build index from tree: {e}")))?;

        let opts = gix::worktree::state::checkout::Options {
            overwrite_existing: true,
            ..Default::default()
        };

        // Pass an owned clone of `repo.objects` so it satisfies `Send + Clone`
        gix::worktree::state::checkout(
            &mut index,
            worktree_dir,
            repo.objects.clone(),
            &mut Discard,
            &mut Discard,
            &should_interrupt,
            opts,
        )
        .map_err(|e| OpossumError::Other(format!("Failed to checkout updated working tree: {e}")))?;

        // 7. Persist the updated index file to disk
        index
            .write(Default::default())
            .map_err(|e| OpossumError::Other(format!("Failed to write index file: {e}")))?;

        Ok(())
    }
}