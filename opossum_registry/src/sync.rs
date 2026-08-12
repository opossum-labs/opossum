use opossum_core::error::{OpmResult, OpossumError};
use std::path::PathBuf;

use git2::{Repository, Signature, build::CheckoutBuilder};

/// Default branch name assumed for the registry repository.
const DEFAULT_BRANCH: &str = "main";

/// Handles synchronization of the local asset registry with a remote Git repository.
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
    /// Returns an [`OpossumError`] if cloning fails (e.g., due to network issues or invalid URL).
    pub fn init_or_clone(&self) -> OpmResult<()> {
        if self.local_path.join(".git").exists() {
            // Repository already exists, basic check to ensure it's openable
            Repository::open(&self.local_path).map_err(|e| {
                OpossumError::Other(format!("Failed to open existing registry repository: {e}"))
            })?;
        } else {
            // Clone the repository
            Repository::clone(&self.remote_url, &self.local_path).map_err(|e| {
                OpossumError::Other(format!(
                    "Failed to clone registry from {}: {}",
                    self.remote_url, e
                ))
            })?;
        }
        Ok(())
    }

    /// Pulls the latest changes from the remote repository.
    ///
    /// This method fetches data from the `origin` remote. It attempts a fast-forward merge first.
    /// If local changes exist, it attempts a normal merge. If there are merge conflicts
    /// (e.g. same UUID/version modified locally and remotely), the operation is aborted to protect data.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if fetching, merging fails, or if merge conflicts are detected.
    ///
    /// # Panics
    ///
    /// Might theoretically panic if the commit signature for a possible merge cannot be set.
    pub fn pull_updates(&self) -> OpmResult<()> {
        let repo = Repository::open(&self.local_path)
            .map_err(|e| OpossumError::Other(format!("Failed to open registry repository: {e}")))?;

        // 1. Fetch from origin
        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| OpossumError::Other(format!("Failed to find remote 'origin': {e}")))?;

        let mut fetch_options = git2::FetchOptions::new();
        remote
            .fetch(&[DEFAULT_BRANCH], Some(&mut fetch_options), None)
            .map_err(|e| OpossumError::Other(format!("Failed to fetch from remote: {e}")))?;

        // 2. Find the commit we just fetched
        let fetch_head = repo
            .find_reference("FETCH_HEAD")
            .map_err(|e| OpossumError::Other(format!("Failed to find FETCH_HEAD: {e}")))?;
        let fetch_commit = repo
            .reference_to_annotated_commit(&fetch_head)
            .map_err(|e| {
                OpossumError::Other(format!("Failed to resolve FETCH_HEAD commit: {e}"))
            })?;

        // 3. Analyze if we can fast-forward or need a merge
        let (analysis, _) = repo
            .merge_analysis(&[&fetch_commit])
            .map_err(|e| OpossumError::Other(format!("Failed to analyze merge: {e}")))?;

        if analysis.is_up_to_date() {
            // Nothing to do, local is already up to date
            return Ok(());
        }

        if analysis.is_fast_forward() {
            // 4a. Perform fast-forward merge
            let refname = format!("refs/heads/{DEFAULT_BRANCH}");

            if let Ok(mut reference) = repo.find_reference(&refname) {
                reference
                    .set_target(fetch_commit.id(), "Fast-Forward Merge")
                    .map_err(|e| {
                        OpossumError::Other(format!("Failed to fast-forward reference: {e}"))
                    })?;
                repo.set_head(&refname)
                    .map_err(|e| OpossumError::Other(format!("Failed to set HEAD: {e}")))?;

                let mut checkout_builder = CheckoutBuilder::new();
                repo.checkout_head(Some(checkout_builder.force()))
                    .map_err(|e| OpossumError::Other(format!("Failed to checkout HEAD: {e}")))?;
            } else {
                return Err(OpossumError::Other(format!(
                    "Local branch {DEFAULT_BRANCH} does not exist."
                )));
            }
        } else if analysis.is_normal() {
            // 4b. Perform a normal merge (handling local changes)
            let head_commit = repo
                .head()
                .map_err(|e| OpossumError::Other(format!("Failed to get HEAD: {e}")))?
                .peel_to_commit()
                .map_err(|e| OpossumError::Other(format!("Failed to peel HEAD to commit: {e}")))?;

            let mut index = repo
                .merge_commits(
                    &head_commit,
                    &repo
                        .find_commit(fetch_commit.id())
                        .map_err(|e| OpossumError::Other(format!("Error merging commit: {e}")))?,
                    None,
                )
                .map_err(|e| OpossumError::Other(format!("Failed to find commit: {e}")))?;

            if index.has_conflicts() {
                // Abort on conflicts to protect local user data
                repo.cleanup_state().ok(); // Try to clean up
                return Err(OpossumError::Other(
                    "Merge conflicts detected (e.g. same material version created locally and remotely). Update aborted.".to_string(),
                ));
            }

            // Write the merged tree
            let oid = index
                .write_tree_to(&repo)
                .map_err(|e| OpossumError::Other(format!("Failed to write merge tree: {e}")))?;
            let result_tree = repo
                .find_tree(oid)
                .map_err(|e| OpossumError::Other(format!("Failed to find merge tree: {e}")))?;

            // Use system signature or fallback for the merge commit
            let signature = repo.signature().unwrap_or_else(|_| {
                Signature::now("OPOSSUM Auto-Sync", "sync@opossum.local")
                    .map_err(|e| OpossumError::Other(format!("Failed to set signature: {e}")))
                    .unwrap()
            });

            let msg = format!("Merge remote-tracking branch '{DEFAULT_BRANCH}'");
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                &msg,
                &result_tree,
                &[
                    &head_commit,
                    &repo
                        .find_commit(fetch_commit.id())
                        .map_err(|e| OpossumError::Other(format!("Failed to find commit: {e}")))?,
                ],
            )
            .map_err(|e| OpossumError::Other(format!("Failed to create merge commit: {e}")))?;

            repo.cleanup_state()
                .map_err(|e| OpossumError::Other(format!("Failed to clean up repo state: {e}")))?;

            // Force checkout to update the working directory with the merge result
            let mut checkout_builder = CheckoutBuilder::new();
            repo.checkout_head(Some(checkout_builder.force()))
                .map_err(|e| OpossumError::Other(format!("Failed to checkout merged HEAD: {e}")))?;
        } else {
            return Err(OpossumError::Other(
                "Unable to pull updates. Repository is in an unsupported state.".to_string(),
            ));
        }

        Ok(())
    }
}
