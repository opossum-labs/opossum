use opossum_core::error::{OpmResult, OpossumError};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

use gix::{
    objs::{
        Commit, Tree,
        tree::{Entry, EntryKind},
    },
    progress::Discard,
    remote::Direction,
};

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
    /// Ensures that the catalog directory is a properly initialized Git repository.
    ///
    /// - If `.git` already exists, verifies the repository can be opened.
    /// - If the directory is empty and a remote URL is present, attempts a direct clone.
    /// - If the directory contains existing assets without `.git` (or no remote is set),
    ///   initializes Git in-place, stages all existing assets into an initial commit,
    ///   and configures the remote tracking branch.
    ///
    /// Asset subdirectories (e.g. `materials/`, `coatings/`) are created on-demand
    /// by `AssetLoader::publish` only when assets of that category are first added.
    pub fn ensure_repository_initialized(&self) -> OpmResult<()> {
        if !self.local_path.exists() {
            fs::create_dir_all(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to create catalog root folder {}: {e}",
                    self.local_path.display()
                ))
            })?;
        }

        // 1. If already a git repository, verify integrity and return
        if self.local_path.join(".git").exists() {
            gix::open(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!("Failed to open existing repository: {e}"))
            })?;
            return Ok(());
        }

        // 2. Fresh installation: clone remote repository if directory is empty and remote URL is configured
        if Self::is_directory_empty(&self.local_path) && !self.remote_url.trim().is_empty() {
            return Self::clone_repository(&self.remote_url, &self.local_path);
        }

        // 3. In-Place Initialization
        let repo = gix::init(&self.local_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to initialize git repository at {}: {e}",
                self.local_path.display()
            ))
        })?;

        // 4. Only create an initial commit if actual assets already exist on disk
        if Self::directory_contains_assets(&self.local_path) {
            let files = Self::collect_files_recursive(&self.local_path)?;
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
                let author = gix::actor::Signature {
                    name: "Opossum Catalog".into(),
                    email: "catalog@opossum.local".into(),
                    time: gix::date::Time::now_local_or_utc(),
                };
                let committer = author.clone();

                let commit = Commit {
                    tree: tree_oid,
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

        // 5. Configure remote 'origin' in .git/config if remote URL is provided
        if !self.remote_url.trim().is_empty() {
            Self::configure_remote_origin(&self.local_path, &self.remote_url);
        }

        Ok(())
    }

    /// Ensures that the remote 'origin' is properly configured in .git/config.
    ///
    /// If the remote section is missing, it appends the origin remote and branch configuration.
    /// If the remote already exists but the URL changed in settings, it updates the URL line.
    pub fn ensure_remote_configured(&self) -> OpmResult<()> {
        let trimmed_url = self.remote_url.trim();
        if trimmed_url.is_empty() {
            return Err(OpossumError::Registry(
                "Cannot configure remote: Remote URL is empty.".to_string(),
            ));
        }

        let config_path = self.local_path.join(".git").join("config");
        if !config_path.exists() {
            return Err(OpossumError::Registry(format!(
                "Git configuration file not found at {}",
                config_path.display()
            )));
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to read git configuration {}: {e}",
                config_path.display()
            ))
        })?;

        if !content.contains("[remote \"origin\"]") {
            // Append origin remote and default tracking branch configuration
            let mut updated = content;
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&format!(
                "[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"{}\"]\n\tremote = origin\n\tmerge = refs/heads/{}\n",
                trimmed_url, DEFAULT_BRANCH, DEFAULT_BRANCH
            ));

            fs::write(&config_path, updated).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to write remote configuration to {}: {e}",
                    config_path.display()
                ))
            })?;
        } else {
            // Ensure the URL matches the latest configuration in case it was modified
            let lines: Vec<&str> = content.lines().collect();
            let mut updated_lines = Vec::new();
            let mut inside_origin = false;
            let mut url_updated = false;

            for line in lines {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    inside_origin = trimmed.eq_ignore_ascii_case("[remote \"origin\"]");
                }

                if inside_origin && trimmed.starts_with("url") && trimmed.contains('=') {
                    updated_lines.push(format!("\turl = {trimmed_url}"));
                    url_updated = true;
                } else {
                    updated_lines.push(line.to_string());
                }
            }

            if url_updated {
                let mut updated_content = updated_lines.join("\n");
                updated_content.push('\n');
                fs::write(&config_path, updated_content).map_err(|e| {
                    OpossumError::Registry(format!(
                        "Failed to update remote URL in {}: {e}",
                        config_path.display()
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Helper to recursively discover all files within a directory, ignoring `.git`.
    fn collect_files_recursive(dir: &Path) -> OpmResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !dir.exists() {
            return Ok(files);
        }

        let entries = fs::read_dir(dir).map_err(|e| {
            OpossumError::Registry(format!("Failed to read directory {}: {e}", dir.display()))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| OpossumError::Registry(e.to_string()))?;
            let path = entry.path();

            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                files.extend(Self::collect_files_recursive(&path)?);
            } else if path.is_file() {
                files.push(path);
            }
        }

        Ok(files)
    }

    /// Sets up the [remote "origin"] section in .git/config.
    fn configure_remote_origin(local_path: &Path, remote_url: &str) {
        let config_path = local_path.join(".git").join("config");
        if let Ok(mut content) = fs::read_to_string(&config_path) {
            if !content.contains("[remote \"origin\"]") {
                content.push_str(&format!(
                    "\n[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"{}\"]\n\tremote = origin\n\tmerge = refs/heads/{}\n",
                    remote_url, DEFAULT_BRANCH, DEFAULT_BRANCH
                ));
                let _ = fs::write(&config_path, content);
            }
        }
    }
    /// Ensures the local repository exists.
    ///
    /// If `.git` is already present, it opens the repository.
    /// If the target folder is empty or does not exist, it executes a direct clone.
    /// If the target folder contains files but lacks `.git`, it adopts the directory
    /// by cloning into a staging folder, transferring `.git`, and checking out missing files.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if cloning, moving metadata, or checkout fails.
    pub fn init_or_clone(&self) -> OpmResult<()> {
        if self.local_path.join(".git").exists() {
            gix::open(&self.local_path).map_err(|e| {
                OpossumError::Registry(format!("Failed to open existing registry repository: {e}"))
            })?;

            // Ensure the remote configuration is present in .git/config
            if !self.remote_url.trim().is_empty() {
                self.ensure_remote_configured()?;
            }
            return Ok(());
        }

        if Self::is_directory_empty(&self.local_path) {
            Self::clone_repository(&self.remote_url, &self.local_path)?;
        } else {
            self.adopt_existing_directory()?;
        }

        // Configure remote after adoption or setup
        if !self.remote_url.trim().is_empty() {
            self.ensure_remote_configured()?;
        }

        Ok(())
    }

    /// Adopts a non-empty directory by cloning to a staging directory, moving `.git`,
    /// and checking out remote tracked files without deleting local uncommitted assets.
    fn adopt_existing_directory(&self) -> OpmResult<()> {
        let parent_dir = self.local_path.parent().unwrap_or(&self.local_path);
        let staging_dir = parent_dir.join(format!(".clone_staging_{}", Uuid::new_v4()));

        // 1. Clone into isolated staging directory
        let clone_res = Self::clone_repository(&self.remote_url, &staging_dir);
        if let Err(e) = clone_res {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(e);
        }

        // 2. Relocate .git metadata directory into local_path
        let staging_git = staging_dir.join(".git");
        let target_git = self.local_path.join(".git");

        if let Err(e) = fs::rename(&staging_git, &target_git) {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(OpossumError::Registry(format!(
                "Failed to move .git metadata into target catalog folder: {e}"
            )));
        }

        // 3. Remove remainder of staging directory
        let _ = fs::remove_dir_all(&staging_dir);

        // 4. Open newly initialized repository at local_path
        let repo = gix::open(&self.local_path).map_err(|e| {
            OpossumError::Registry(format!("Failed to open adopted registry repository: {e}"))
        })?;

        // 5. Resolve HEAD commit to check out tracked catalog assets into the working directory
        let head_commit = repo
            .head_commit()
            .map_err(|e| OpossumError::Registry(format!("Failed to resolve HEAD commit: {e}")))?;

        Self::checkout_worktree(&repo, head_commit.id)?;

        Ok(())
    }

    /// Checks if a directory does not exist or contains no entries.
    fn is_directory_empty(path: &Path) -> bool {
        if !path.exists() {
            return true;
        }
        match fs::read_dir(path) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => false,
        }
    }

    /// Executes a clean clone into a specific directory path.
    fn clone_repository(remote_url: &str, target_path: &Path) -> OpmResult<()> {
        let should_interrupt = AtomicBool::new(false);

        let mut prepare_clone = gix::prepare_clone(remote_url, target_path).map_err(|e| {
            OpossumError::Registry(format!("Failed to prepare clone from {remote_url}: {e}"))
        })?;

        let (mut prepare_checkout, _fetch_outcome) = prepare_clone
            .fetch_then_checkout(Discard, &should_interrupt)
            .map_err(|e| {
                OpossumError::Registry(format!("Failed to fetch repository from {remote_url}: {e}"))
            })?;

        let (_repo, _checkout_outcome) = prepare_checkout
            .main_worktree(Discard, &should_interrupt)
            .map_err(|e| {
                OpossumError::Registry(format!("Failed to checkout main worktree: {e}"))
            })?;

        Ok(())
    }

    /// Checks out the specified commit into the worktree, merging files with the working tree.
    fn checkout_worktree(
        repo: &gix::Repository,
        commit_id: impl Into<gix::ObjectId>,
    ) -> OpmResult<()> {
        let commit_oid: gix::ObjectId = commit_id.into();
        let should_interrupt = AtomicBool::new(false);

        let worktree = repo.worktree().ok_or_else(|| {
            OpossumError::Registry("Cannot checkout files in a bare repository".into())
        })?;
        let worktree_dir = worktree.base();

        // Query the repository for the object corresponding to the commit ID
        let tree_id = repo
            .find_object(commit_oid)
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
        .map_err(|e| OpossumError::Registry(format!("Failed to checkout working tree: {e}")))?;

        index
            .write(gix::index::write::Options::default())
            .map_err(|e| OpossumError::Registry(format!("Failed to write index file: {e}")))?;

        Ok(())
    }

    /// Pulls the latest changes from the remote repository via Fast-Forward.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Registry`] if fetching fails, branches have diverged, or checkout fails.
    pub fn pull_updates(&self) -> OpmResult<()> {
        self.ensure_remote_configured()?;

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

        // Determine local branch head if present
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
                // Case A: Clean Fast-Forward
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

                Self::checkout_worktree(&repo, remote_commit_id)?;
                return Ok(());
            }

            // Case B: Divergence detected
            let has_local_assets = Self::directory_contains_assets(&self.local_path);

            if !has_local_assets {
                // Local only contains placeholder files: adopt remote repository state directly
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

                Self::checkout_worktree(&repo, remote_commit_id)?;
                return Ok(());
            }

            // Case C: Local contains genuine assets: rebase them on top of remote commit
            self.rebase_local_assets_onto_remote(&repo, remote_commit_id)?;
        } else {
            // Unborn branch: point directly to fetched remote commit
            repo.reference(
                local_ref_name.as_str(),
                remote_commit_id.detach(),
                gix::refs::transaction::PreviousValue::MustNotExist,
                "registry: initial checkout from remote",
            )
            .map_err(|e| {
                OpossumError::Registry(format!("Failed to initialize local branch: {e}"))
            })?;

            Self::checkout_worktree(&repo, remote_commit_id)?;
        }

        Ok(())
    }

    /// Checks whether the catalog directory contains any published assets (materials or coatings).
    fn directory_contains_assets(dir: &Path) -> bool {
        let check_folder = |name: &str| -> bool {
            let p = dir.join(name);
            if !p.exists() {
                return false;
            }
            if let Ok(mut read) = fs::read_dir(p) {
                read.next().is_some()
            } else {
                false
            }
        };

        check_folder("materials") || check_folder("coatings")
    }

    /// Replays all local asset files on top of the remote tree and creates a rebased commit.
    fn rebase_local_assets_onto_remote(
        &self,
        repo: &gix::Repository,
        remote_commit_id: gix::Id<'_>,
    ) -> OpmResult<()> {
        let remote_commit_obj = repo.find_object(remote_commit_id).map_err(|e| {
            OpossumError::Registry(format!("Failed to find remote commit object: {e}"))
        })?;
        let remote_tree_id = remote_commit_obj
            .peel_to_tree()
            .map_err(|e| OpossumError::Registry(format!("Failed to peel remote tree: {e}")))?
            .id;

        // Collect and stage all local asset files into the remote tree
        let files = Self::collect_files_recursive(&self.local_path)?;
        let mut current_tree_oid = remote_tree_id;
        let mut staged_asset_count = 0;

        for file_path in files {
            if let Ok(rel_path) = file_path.strip_prefix(&self.local_path) {
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                // Only migrate catalog asset domains
                if rel_str.starts_with("materials/") || rel_str.starts_with("coatings/") {
                    let segments: Vec<&str> = rel_str.split('/').collect();
                    if let Ok(file_bytes) = fs::read(&file_path)
                        && let Ok(blob) = repo.write_blob(&file_bytes)
                    {
                        current_tree_oid = upsert_path_in_tree(
                            repo,
                            Some(current_tree_oid),
                            &segments,
                            blob.detach(),
                        )?;
                        staged_asset_count += 1;
                    }
                }
            }
        }

        // Prepare commit author information
        let (author_name, author_email) = match repo.author() {
            Some(Ok(sig)) => (sig.name.to_string(), sig.email.to_string()),
            _ => ("Opossum User".to_string(), "user@opossum.local".to_string()),
        };
        let now = gix::date::Time::now_local_or_utc();
        let author = gix::actor::Signature {
            name: author_name.into(),
            email: author_email.into(),
            time: now,
        };
        let committer = author.clone();

        let commit = Commit {
            tree: current_tree_oid,
            parents: vec![remote_commit_id.detach()].into(),
            author,
            committer,
            encoding: None,
            message: format!(
                "Rebase: preserve {staged_asset_count} local asset(s) on top of remote catalog"
            )
            .into(),
            extra_headers: Vec::new(),
        };

        let new_commit_id = repo
            .write_object(&commit)
            .map_err(|e| OpossumError::Registry(format!("Failed to write rebase commit: {e}")))?
            .detach();

        let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
        repo.reference(
            local_ref_name.as_str(),
            new_commit_id,
            gix::refs::transaction::PreviousValue::Any,
            "registry: rebase local assets onto remote",
        )
        .map_err(|e| {
            OpossumError::Registry(format!("Failed to update local branch reference: {e}"))
        })?;

        Self::checkout_worktree(repo, new_commit_id)?;

        Ok(())
    }
}
/// Automatically commits a newly published or updated asset file to the local Git repository.
///
/// Reads author information from Git config (.gitconfig) with fallback to default values,
/// creates the Git tree hierarchy for the asset path, writes the commit object,
/// updates `refs/heads/main`, and synchronizes the Git index.
pub fn commit_asset_file(
    repo_root: &Path,
    file_path: &Path,
    commit_message: &str,
) -> OpmResult<()> {
    let repo = gix::open(repo_root).map_err(|e| {
        OpossumError::Registry(format!(
            "Failed to open registry repository for commit: {e}"
        ))
    })?;

    // Determine path relative to repository root and normalize directory separators for Git
    let rel_path = file_path.strip_prefix(repo_root).map_err(|e| {
        OpossumError::Registry(format!("File path is not within registry root: {e}"))
    })?;
    let rel_path_str = rel_path
        .to_str()
        .ok_or_else(|| OpossumError::Registry("Path contains invalid UTF-8 characters".into()))?
        .replace('\\', "/");

    let path_segments: Vec<&str> = rel_path_str.split('/').collect();

    // 1. Resolve author and committer signature from Git config with safe fallback
    let (author_name, author_email) = match repo.author() {
        Some(Ok(sig)) => (sig.name.to_string(), sig.email.to_string()),
        _ => match repo.committer() {
            Some(Ok(sig)) => (sig.name.to_string(), sig.email.to_string()),
            _ => ("Opossum User".to_string(), "user@opossum.local".to_string()),
        },
    };

    let now = gix::date::Time::now_local_or_utc();
    let author = gix::actor::Signature {
        name: author_name.into(),
        email: author_email.into(),
        time: now,
    };
    let committer = author.clone();

    // 2. Read raw file content and write blob into object database
    let file_bytes = fs::read(file_path).map_err(|e| {
        OpossumError::Registry(format!(
            "Failed to read asset file for staging {}: {e}",
            file_path.display()
        ))
    })?;

    let blob_id = repo
        .write_blob(&file_bytes)
        .map_err(|e| OpossumError::Registry(format!("Failed to write blob object: {e}")))?
        .detach();

    // 3. Resolve parent commit and current root tree
    let local_ref_name = format!("refs/heads/{DEFAULT_BRANCH}");
    let (parent_commit_id, base_tree_oid) = match repo.find_reference(&local_ref_name) {
        Ok(ref_head) => {
            let commit_id = ref_head.into_fully_peeled_id().map_err(|e| {
                OpossumError::Registry(format!("Failed to resolve HEAD commit: {e}"))
            })?;
            let commit_obj = repo.find_object(commit_id).map_err(|e| {
                OpossumError::Registry(format!("Failed to find commit object: {e}"))
            })?;
            let tree_id = commit_obj
                .peel_to_tree()
                .map_err(|e| OpossumError::Registry(format!("Failed to peel commit tree: {e}")))?
                .id;
            (Some(commit_id.detach()), Some(tree_id))
        }
        Err(_) => (None, None), // Initial commit when repository has no commits yet
    };

    // 4. Hierarchically insert or update asset in tree structure
    let new_root_tree_oid = upsert_path_in_tree(&repo, base_tree_oid, &path_segments, blob_id)?;

    // 5. Build and write commit object
    let commit = Commit {
        tree: new_root_tree_oid,
        parents: parent_commit_id.into_iter().collect(),
        author,
        committer,
        encoding: None,
        message: commit_message.into(),
        extra_headers: Vec::new(),
    };

    let commit_id = repo
        .write_object(&commit)
        .map_err(|e| OpossumError::Registry(format!("Failed to write commit object: {e}")))?
        .detach();

    // 6. Advance branch reference to new commit
    repo.reference(
        local_ref_name.as_str(),
        commit_id,
        match parent_commit_id {
            Some(prev_id) => {
                gix::refs::transaction::PreviousValue::MustExistAndMatch(prev_id.into())
            }
            None => gix::refs::transaction::PreviousValue::MustNotExist,
        },
        "registry: automated asset commit",
    )
    .map_err(|e| {
        OpossumError::Registry(format_error_chain(
            "Failed to update local branch reference",
            &e,
        ))
    })?;

    // 7. Synchronize in-memory index with disk to keep working directory clean
    let mut index = repo.index_from_tree(&new_root_tree_oid).map_err(|e| {
        OpossumError::Registry(format_error_chain("Failed to build index from tree", &e))
    })?;

    index
        .write(gix::index::write::Options::default())
        .map_err(|e| {
            OpossumError::Registry(format_error_chain("Failed to write index file", &e))
        })?;

    Ok(())
}

/// Recursively updates or inserts a path entry into a Git tree structure.
fn upsert_path_in_tree(
    repo: &gix::Repository,
    tree_oid: Option<gix::ObjectId>,
    path_segments: &[&str],
    blob_oid: gix::ObjectId,
) -> OpmResult<gix::ObjectId> {
    if path_segments.is_empty() {
        return Err(OpossumError::Registry(
            "Path segments cannot be empty".into(),
        ));
    }

    // Load existing entries of the current tree if present
    let mut entries = if let Some(oid) = tree_oid {
        let tree_obj = repo
            .find_object(oid)
            .map_err(|e| OpossumError::Registry(format!("Failed to find tree object: {e}")))?
            .peel_to_tree()
            .map_err(|e| OpossumError::Registry(format!("Failed to peel tree: {e}")))?;

        tree_obj
            .iter()
            .map(|e_res| {
                let e = e_res.map_err(|err| {
                    OpossumError::Registry(format!("Failed to parse tree entry: {err}"))
                })?;
                Ok(Entry {
                    mode: e.mode(),
                    filename: e.filename().to_owned(),
                    oid: e.id().detach(),
                })
            })
            .collect::<OpmResult<Vec<Entry>>>()?
    } else {
        Vec::new()
    };

    if path_segments.len() == 1 {
        // Leaf segment: add or replace the file blob using EntryKind::Blob
        let leaf_name = path_segments[0];
        entries.retain(|e| e.filename != leaf_name);
        entries.push(Entry {
            mode: EntryKind::Blob.into(),
            filename: leaf_name.into(),
            oid: blob_oid,
        });
    } else {
        // Intermediate segment: find child tree, recursively update, then re-insert using EntryKind::Tree
        let dir_name = path_segments[0];
        let existing_child_oid = entries
            .iter()
            .find(|e| e.filename == dir_name)
            .map(|e| e.oid);

        let child_tree_oid =
            upsert_path_in_tree(repo, existing_child_oid, &path_segments[1..], blob_oid)?;

        entries.retain(|e| e.filename != dir_name);
        entries.push(Entry {
            mode: EntryKind::Tree.into(),
            filename: dir_name.into(),
            oid: child_tree_oid,
        });
    }

    // Sort entries canonical to Git tree specifications (handled by Entry::cmp)
    entries.sort();

    let tree = Tree { entries };
    let new_tree_oid = repo
        .write_object(&tree)
        .map_err(|e| OpossumError::Registry(format!("Failed to write tree object: {e}")))?
        .detach();

    Ok(new_tree_oid)
}
/// Formats a root error and all of its underlying sources into a readable diagnostic string.
fn format_error_chain(context: &str, err: &dyn std::error::Error) -> String {
    let mut message = format!("{context}: {err}");
    let mut current_source = err.source();

    while let Some(source) = current_source {
        message.push_str(&format!(" -> Caused by: {source}"));
        current_source = source.source();
    }

    message
}
