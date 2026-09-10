#![allow(missing_docs)]
use std::path::Path;
use std::process::Command;

pub fn main() {
    // Re-run this script if build.rs itself changes
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-env=OPM_FILE_VERSION=0");

    // Inform Cargo when to re-run the build script based on Git repository changes
    track_git_changes();

    // 1. Retrieve Git describe output (tag, commit hash, and dirty flag)
    if let Some(describe) = run_git(&["describe", "--tags", "--always", "--dirty"]) {
        println!("cargo::rustc-env=OPM_GIT_DESCRIBE={describe}");
    }

    // 2. Retrieve the latest commit timestamp formatted in strict ISO 8601 / RFC 3339
    if let Some(timestamp) = run_git(&["log", "-1", "--format=%cI"]) {
        println!("cargo::rustc-env=OPM_GIT_COMMIT_TIMESTAMP={timestamp}");
    }
}

/// Execute a Git command and return stdout as a trimmed String on success.
fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).ok()?;
        Some(stdout.trim().to_string())
    } else {
        None
    }
}

/// Monitor `.git/HEAD` and the current branch ref so Cargo invalidates the cache
/// whenever a new commit is created or branches are switched.
fn track_git_changes() {
    let git_dir = Path::new(".git");
    if !git_dir.exists() {
        return;
    }

    // Support standard repositories as well as Git worktrees and submodules
    let git_base = if git_dir.is_file() {
        std::fs::read_to_string(git_dir)
            .ok()
            .and_then(|content| {
                content
                    .strip_prefix("gitdir: ")
                    .map(|path| Path::new(path.trim()).to_path_buf())
            })
            .unwrap_or_else(|| git_dir.to_path_buf())
    } else {
        git_dir.to_path_buf()
    };

    let head_path = git_base.join("HEAD");
    if head_path.exists() {
        println!("cargo::rerun-if-changed={}", head_path.display());
        if let Ok(head_contents) = std::fs::read_to_string(&head_path) {
            // If HEAD points to a ref (e.g. "ref: refs/heads/main"), watch that ref file too
            if let Some(ref_path) = head_contents.strip_prefix("ref: ") {
                let ref_full_path = git_base.join(ref_path.trim());
                if ref_full_path.exists() {
                    println!("cargo::rerun-if-changed={}", ref_full_path.display());
                }
            }
        }
    }
}
