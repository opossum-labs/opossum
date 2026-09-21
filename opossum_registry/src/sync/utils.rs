use gix::{Repository, actor::Signature, date::Time, progress::Discard};
use opossum_core::error::{OpmResult, OpossumError};
use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

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
        .unwrap_or_else(|| ("Opossum User".to_string(), "user@opossum.local".to_string()));

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

/// Checks whether the directory contains any non-empty subdirectories other than `.git`.
pub fn directory_contains_assets(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path.file_name().is_some_and(|name| name != ".git")
                && fs::read_dir(&path).is_ok_and(|mut sub| sub.next().is_some())
            {
                return true;
            }
        }
    }
    false
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
        .map_err(|e| OpossumError::Registry(format!("Failed to checkout main worktree: {e}")))?;

    Ok(())
}

/// Checks out the specified commit into the worktree.
pub fn checkout_worktree(repo: &Repository, commit_id: impl Into<gix::ObjectId>) -> OpmResult<()> {
    let commit_old_id: gix::ObjectId = commit_id.into();
    let should_interrupt = AtomicBool::new(false);

    let worktree = repo.worktree().ok_or_else(|| {
        OpossumError::Registry("Cannot checkout files in a bare repository".into())
    })?;
    let worktree_dir = worktree.base();

    let tree_id = repo
        .find_object(commit_old_id)
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
