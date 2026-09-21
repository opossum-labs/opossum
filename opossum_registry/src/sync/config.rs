use opossum_core::error::{OpmResult, OpossumError};
use std::{fmt::Write, fs, path::Path};

use super::DEFAULT_BRANCH;

/// Ensures that the remote 'origin' is properly configured in .git/config.
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
