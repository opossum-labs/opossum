use gix::{Repository, actor::Signature, date::Time, open::Options, progress::Discard};
use opossum_core::error::{OpmResult, OpossumError};
use std::{
    collections::HashSet,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// Default Git committer and author name used when no user identity is configured.
pub const DEFAULT_GIT_USER_NAME: &str = "Opossum User";
/// Default Git committer and author email used when no user identity is configured.
pub const DEFAULT_GIT_USER_EMAIL: &str = "user@opossum.local";

/// Normalizes local filesystem paths to use forward slashes for Git config compatibility.
#[must_use]
pub fn normalize_git_url_or_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") || trimmed.starts_with("git@") {
        trimmed.to_string()
    } else {
        trimmed.replace('\\', "/")
    }
}

/// Opens a Git repository for a catalog in a failsafe manner.
///
/// Tries standard `gix::open` first. If loading git configuration fails,
/// retries with `gix::open_opts` using isolated options and permissions.
pub fn open_registry_repo(local_path: &Path) -> OpmResult<Repository> {
    match gix::open(local_path) {
        Ok(repo) => Ok(repo),
        Err(primary_err) => {
            let opts = Options::isolated().permissions(gix::open::Permissions::isolated());
            gix::open_opts(local_path, opts).map_err(|fallback_err| {
                let chained_primary =
                    format_error_chain("Standard gix::open failed", &primary_err);
                let chained_fallback =
                    format_error_chain("Isolated gix::open_opts fallback failed", &fallback_err);
                OpossumError::Registry(format!(
                    "Failed to open registry repository at {}: {chained_primary} | {chained_fallback}",
                    local_path.display()
                ))
            })
        }
    }
}

/// Resolves author/committer signature from git configuration with sensible fallbacks.
pub fn resolve_signature(repo: &Repository) -> Signature {
    let (name, email) = repo
        .author()
        .and_then(Result::ok)
        .map(|sig| (sig.name.to_string(), sig.email.to_string()))
        .or_else(|| {
            repo.committer()
                .and_then(Result::ok)
                .map(|sig| (sig.name.to_string(), sig.email.to_string()))
        })
        .unwrap_or_else(|| {
            (
                DEFAULT_GIT_USER_NAME.to_string(),
                DEFAULT_GIT_USER_EMAIL.to_string(),
            )
        });
    Signature {
        name: name.into(),
        email: email.into(),
        time: Time::now_local_or_utc(),
    }
}

/// Checks whether a relative path points to a registry asset file.
pub fn is_asset_file(rel_path: &Path) -> bool {
    let is_ron = rel_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ron"));
    let not_git = !rel_path.starts_with(".git");
    is_ron && not_git
}

/// Checks whether the directory contains any valid asset files.
pub fn directory_contains_assets(dir: &Path) -> bool {
    let Ok(files) = collect_files_recursive(dir) else {
        return false;
    };
    files
        .iter()
        .filter_map(|p| p.strip_prefix(dir).ok())
        .any(is_asset_file)
}

/// Checks if a directory does not exist or contains no entries.
pub fn is_directory_empty(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

/// Recursively discovers all files within a directory, ignoring `.git`.
pub fn collect_files_recursive(dir: &Path) -> OpmResult<Vec<PathBuf>> {
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
            files.extend(collect_files_recursive(&path)?);
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

/// Executes a clean repository clone into a specific directory path.
pub fn clone_repository(remote_url: &str, target_path: &Path) -> OpmResult<()> {
    let normalized_url = normalize_git_url_or_path(remote_url);
    let should_interrupt = AtomicBool::new(false);
    let mut prepare_clone =
        gix::prepare_clone(normalized_url.as_str(), target_path).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to prepare clone from {normalized_url}: {e}"
            ))
        })?;
    let (mut prepare_checkout, _fetch_outcome) = prepare_clone
        .fetch_then_checkout(Discard, &should_interrupt)
        .map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to fetch repository from {normalized_url}: {e}"
            ))
        })?;
    let (_repo, _checkout_outcome) = prepare_checkout
        .main_worktree(Discard, &should_interrupt)
        .map_err(|e| OpossumError::Registry(format!("Failed to checkout main worktree: {e}")))?;
    Ok(())
}

/// Recursively collects all relative file paths present in a Git tree.
fn collect_tree_paths(
    repo: &Repository,
    tree_id: gix::ObjectId,
    prefix: &Path,
    out: &mut HashSet<PathBuf>,
) -> OpmResult<()> {
    let tree_obj = repo
        .find_object(tree_id)
        .map_err(|e| OpossumError::Registry(format!("Failed to find tree object {tree_id}: {e}")))?
        .peel_to_tree()
        .map_err(|e| OpossumError::Registry(format!("Failed to peel tree {tree_id}: {e}")))?;
    for entry_res in tree_obj.iter() {
        let entry = entry_res
            .map_err(|e| OpossumError::Registry(format!("Failed to read tree entry: {e}")))?;
        let name_str = entry.filename().to_string();
        let rel = prefix.join(&name_str);
        if entry.mode().is_tree() {
            collect_tree_paths(repo, entry.id().detach(), &rel, out)?;
        } else {
            out.insert(rel);
        }
    }
    Ok(())
}

/// Removes worktree files and empty subdirectories (excluding `.git`) that do not exist in `tree_id`.
pub fn prune_worktree_to_tree(
    repo: &Repository,
    worktree_dir: &Path,
    tree_id: gix::ObjectId,
) -> OpmResult<()> {
    let mut expected_files = HashSet::new();
    collect_tree_paths(repo, tree_id, Path::new(""), &mut expected_files)?;

    let existing_files = collect_files_recursive(worktree_dir)?;
    for file_path in existing_files {
        if let Ok(rel_path) = file_path.strip_prefix(worktree_dir)
            && !expected_files.contains(rel_path)
        {
            fs::remove_file(&file_path).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to remove stale worktree file {}: {e}",
                    file_path.display()
                ))
            })?;
        }
    }

    remove_empty_dirs_recursive(worktree_dir, worktree_dir)?;
    Ok(())
}

fn remove_empty_dirs_recursive(root_dir: &Path, current_dir: &Path) -> OpmResult<()> {
    if !current_dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(current_dir).map_err(|e| {
        OpossumError::Registry(format!(
            "Failed to read directory {}: {e}",
            current_dir.display()
        ))
    })?;
    for entry_res in entries {
        let entry = entry_res.map_err(|e| OpossumError::Registry(e.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            remove_empty_dirs_recursive(root_dir, &path)?;
            let is_empty = fs::read_dir(&path).is_ok_and(|mut it| it.next().is_none());
            if is_empty && path != root_dir {
                fs::remove_dir(&path).map_err(|e| {
                    OpossumError::Registry(format!(
                        "Failed to remove empty directory {}: {e}",
                        path.display()
                    ))
                })?;
            }
        }
    }
    Ok(())
}

/// Checks out the specified commit into the worktree and removes stale files not in that commit.
pub fn checkout_worktree(repo: &Repository, commit_id: impl Into<gix::ObjectId>) -> OpmResult<()> {
    let commit_old_id: gix::ObjectId = commit_id.into();
    let should_interrupt = AtomicBool::new(false);
    let worktree = repo.worktree().ok_or_else(|| {
        OpossumError::Registry("Cannot checkout files in a bare repository".into())
    })?;
    let worktree_dir = worktree.base().to_path_buf();
    let tree_id = repo
        .find_object(commit_old_id)
        .map_err(|e| OpossumError::Registry(format!("Failed to load commit object: {e}")))?
        .peel_to_tree()
        .map_err(|e| OpossumError::Registry(format!("Failed to peel tree: {e}")))?
        .id;
    prune_worktree_to_tree(repo, &worktree_dir, tree_id)?;
    let mut index = repo
        .index_from_tree(&tree_id)
        .map_err(|e| OpossumError::Registry(format!("Failed to build index from tree: {e}")))?;
    let opts = gix::worktree::state::checkout::Options {
        overwrite_existing: true,
        ..Default::default()
    };
    gix::worktree::state::checkout(
        &mut index,
        &worktree_dir,
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

/// Formats a root error and all of its underlying sources into a readable diagnostic string.
pub fn format_error_chain(context: &str, err: &dyn std::error::Error) -> String {
    let mut message = format!("{context}: {err}");
    let mut current_source = err.source();
    while let Some(source) = current_source {
        let _ = write!(message, " -> Caused by: {source}");
        current_source = source.source();
    }
    message
}
