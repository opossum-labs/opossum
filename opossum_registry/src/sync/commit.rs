use gix::{
    ObjectId, Repository,
    objs::{
        Commit, Tree,
        tree::{Entry, EntryKind},
    },
};
use opossum_core::error::{OpmResult, OpossumError};
use std::{fs, path::Path};

use super::{
    DEFAULT_BRANCH,
    utils::{format_error_chain, resolve_signature},
};

/// Automatically commits a newly published or updated asset file to the local Git repository.
///
/// # Errors
///
/// This function returns an error if the underlying git operation fails.
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

    let rel_path = file_path.strip_prefix(repo_root).map_err(|e| {
        OpossumError::Registry(format!("File path is not within registry root: {e}"))
    })?;
    let rel_path_str = rel_path
        .to_str()
        .ok_or_else(|| OpossumError::Registry("Path contains invalid UTF-8 characters".into()))?
        .replace('\\', "/");

    let path_segments: Vec<&str> = rel_path_str.split('/').collect();

    // 1. Resolve author and committer signatures
    let author = resolve_signature(&repo);
    let committer = author.clone();

    // 2. Read raw file content and write blob
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
        Err(_) => (None, None),
    };

    // 4. Hierarchically insert or update asset in tree
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

    // 6. Advance branch reference
    repo.reference(
        local_ref_name.as_str(),
        commit_id,
        parent_commit_id.map_or(
            gix::refs::transaction::PreviousValue::MustNotExist,
            |prev_id| gix::refs::transaction::PreviousValue::MustExistAndMatch(prev_id.into()),
        ),
        "registry: automated asset commit",
    )
    .map_err(|e| {
        OpossumError::Registry(format_error_chain(
            "Failed to update local branch reference",
            &e,
        ))
    })?;

    // 7. Synchronize index to disk
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
pub fn upsert_path_in_tree(
    repo: &Repository,
    tree_oid: Option<ObjectId>,
    path_segments: &[&str],
    blob_oid: ObjectId,
) -> OpmResult<ObjectId> {
    if path_segments.is_empty() {
        return Err(OpossumError::Registry(
            "Path segments cannot be empty".into(),
        ));
    }

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
        let leaf_name = path_segments[0];
        entries.retain(|e| e.filename != leaf_name);
        entries.push(Entry {
            mode: EntryKind::Blob.into(),
            filename: leaf_name.into(),
            oid: blob_oid,
        });
    } else {
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

    entries.sort();

    let tree = Tree { entries };
    let new_tree_oid = repo
        .write_object(&tree)
        .map_err(|e| OpossumError::Registry(format!("Failed to write tree object: {e}")))?
        .detach();

    Ok(new_tree_oid)
}
