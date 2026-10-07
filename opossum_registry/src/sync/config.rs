use opossum_core::error::{OpmResult, OpossumError};
use std::{fmt::Write, fs, path::Path};

use super::{
    DEFAULT_BRANCH,
    utils::{DEFAULT_GIT_USER_EMAIL, DEFAULT_GIT_USER_NAME},
};

/// Ensures that a valid author/committer identity exists in .git/config.
///
/// Section-aware: guarantees that `name` and `email` are placed directly within
/// the `[user]` section, preventing configuration leakage into other sections.
pub fn ensure_committer_configured(local_path: &Path) -> OpmResult<()> {
    let config_path = local_path.join(".git").join("config");
    if !config_path.exists() {
        return Ok(());
    }

    // Check if gix can already resolve a committer from existing configuration
    if let Ok(repo) = gix::open(local_path)
        && repo.committer().and_then(Result::ok).is_some()
    {
        return Ok(());
    }

    let content = fs::read_to_string(&config_path).map_err(|e| {
        OpossumError::Registry(format!(
            "Failed to read git configuration {}: {e}",
            config_path.display()
        ))
    })?;

    let lines: Vec<&str> = content.lines().collect();
    let mut updated_lines = Vec::new();
    let mut inside_user_section = false;
    let mut user_section_found = false;
    let mut has_name = false;
    let mut has_email = false;
    let mut modified = false;

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            // When exiting the [user] section, insert any missing fields
            if inside_user_section {
                if !has_name {
                    updated_lines.push(format!("\tname = {DEFAULT_GIT_USER_NAME}"));
                    modified = true;
                }
                if !has_email {
                    updated_lines.push(format!("\temail = {DEFAULT_GIT_USER_EMAIL}"));
                    modified = true;
                }
                inside_user_section = false;
            }

            if trimmed.eq_ignore_ascii_case("[user]") {
                inside_user_section = true;
                user_section_found = true;
            }
        } else if inside_user_section {
            if trimmed.starts_with("name") && trimmed.contains('=') {
                has_name = true;
            } else if trimmed.starts_with("email") && trimmed.contains('=') {
                has_email = true;
            }
        }

        updated_lines.push((*line).to_string());
    }

    // Handle case where [user] was the last section in the file
    if inside_user_section {
        if !has_name {
            updated_lines.push(format!("\tname = {DEFAULT_GIT_USER_NAME}"));
            modified = true;
        }
        if !has_email {
            updated_lines.push(format!("\temail = {DEFAULT_GIT_USER_EMAIL}"));
            modified = true;
        }
    }

    // If [user] section was never found, append it cleanly at the end
    if !user_section_found {
        if !updated_lines.is_empty() && !updated_lines.last().is_some_and(String::is_empty) {
            updated_lines.push(String::new());
        }
        updated_lines.push("[user]".to_string());
        updated_lines.push(format!("\tname = {DEFAULT_GIT_USER_NAME}"));
        updated_lines.push(format!("\temail = {DEFAULT_GIT_USER_EMAIL}"));
        modified = true;
    }

    if modified {
        let mut updated_content = updated_lines.join("\n");
        updated_content.push('\n');
        fs::write(&config_path, updated_content).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to write user configuration to {}: {e}",
                config_path.display()
            ))
        })?;
    }

    Ok(())
}

/// Ensures that the remote 'origin' is properly configured in .git/config.
///
/// Only performs disk writes if the configuration actually changed.
pub fn ensure_remote_configured(local_path: &Path, remote_url: &str) -> OpmResult<()> {
    let trimmed_url = remote_url.trim();
    if trimmed_url.is_empty() {
        return Err(OpossumError::Registry(
            "Cannot configure remote: Remote URL is empty.".to_string(),
        ));
    }

    let config_path = local_path.join(".git").join("config");
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

    if content.contains("[remote \"origin\"]") {
        let lines: Vec<&str> = content.lines().collect();
        let mut updated_lines = Vec::new();
        let mut inside_origin = false;
        let mut url_found = false;
        let mut modified = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                if inside_origin && !url_found {
                    updated_lines.push(format!("\turl = {trimmed_url}"));
                    modified = true;
                }
                inside_origin = trimmed.eq_ignore_ascii_case("[remote \"origin\"]");
            }

            if inside_origin && trimmed.starts_with("url") && trimmed.contains('=') {
                url_found = true;
                let current_url = trimmed
                    .split_once('=')
                    .map(|(_, val)| val.trim())
                    .unwrap_or_default();

                if current_url == trimmed_url {
                    updated_lines.push(line.to_string());
                } else {
                    updated_lines.push(format!("\turl = {trimmed_url}"));
                    modified = true;
                }
            } else {
                updated_lines.push(line.to_string());
            }
        }

        if inside_origin && !url_found {
            updated_lines.push(format!("\turl = {trimmed_url}"));
            modified = true;
        }

        if modified {
            let mut updated_content = updated_lines.join("\n");
            updated_content.push('\n');
            fs::write(&config_path, updated_content).map_err(|e| {
                OpossumError::Registry(format!(
                    "Failed to update remote URL in {}: {e}",
                    config_path.display()
                ))
            })?;
        }
    } else {
        let mut updated = content;
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        let _ = write!(
            updated,
            "[remote \"origin\"]\n\turl = {trimmed_url}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"{DEFAULT_BRANCH}\"]\n\tremote = origin\n\tmerge = refs/heads/{DEFAULT_BRANCH}\n"
        );

        fs::write(&config_path, updated).map_err(|e| {
            OpossumError::Registry(format!(
                "Failed to write remote configuration to {}: {e}",
                config_path.display()
            ))
        })?;
    }

    Ok(())
}
