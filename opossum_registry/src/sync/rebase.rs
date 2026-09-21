use gix::{Id, Repository, objs::Commit};
use opossum_core::error::{OpmResult, OpossumError};
use std::{fs, path::Path};

use super::{
    DEFAULT_BRANCH,
    commit::upsert_path_in_tree,
    utils::{checkout_worktree, collect_files_recursive, is_asset_file, resolve_signature},
};

/// Replays all local asset files on top of the remote tree and creates a rebased commit.
pub fn rebase_local_assets_onto_remote(
    local_path: &Path,
    repo: &Repository,
    remote_commit_id: Id<'_>,
) -> OpmResult<()> {
    let remote_commit_obj = repo
        .find_object(remote_commit_id)
        .map_err(|e| OpossumError::Registry(format!("Failed to find remote commit object: {e}")))?;
    let remote_tree_id = remote_commit_obj
        .peel_to_tree()
        .map_err(|e| OpossumError::Registry(format!("Failed to peel remote tree: {e}")))?
        .id;

    let files = collect_files_recursive(local_path)?;
    let mut current_tree_oid = remote_tree_id;
    let mut staged_asset_count = 0;

    for file_path in files {
        if let Ok(rel_path) = file_path.strip_prefix(local_path) {
            // Check dynamically if file is an asset rather than matching hardcoded strings
            if is_asset_file(rel_path) {
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
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

    let author = resolve_signature(repo);
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
    .map_err(|e| OpossumError::Registry(format!("Failed to update local branch reference: {e}")))?;

    checkout_worktree(repo, new_commit_id)?;

    Ok(())
}
