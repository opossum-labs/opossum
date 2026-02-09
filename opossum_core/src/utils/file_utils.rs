//! Various functions for file handling.
use crate::error::{OpmResult, OpossumError};
use log::info;
use std::{
    fs::{File, create_dir, remove_dir_all},
    path::{Path, PathBuf},
};

/// Creates a new file at a path constructed from directory, filename, and extension,
/// and logs the operation with a user-defined description.
///
/// # Parameters
///
/// * `path` - Base directory for the file.
/// * `f_name` - Name of the file without extension.
/// * `f_ext` - File extension (e.g., `"ron"`, `"dot"`).
///
/// # Returns
///
/// * `Ok(File)` if the file is successfully created.
/// * `Err(OpossumError)` if file creation fails.
///
/// # Errors
///
/// Returns an error if the file cannot be created (e.g., due to permissions, invalid path, or I/O issues).
pub fn create_file_instance(path: &Path, f_name: &str, f_ext: &str) -> OpmResult<File> {
    let f_path = path.join(f_name).with_extension(f_ext);
    File::create(&f_path).map_err(|e| {
        OpossumError::Other(format!(
            "File creation failed for '{}': {}",
            f_path.display(),
            e
        ))
    })
}
/// Creates a fresh `data/` subdirectory in the given report directory.
///
/// If a `data/` folder already exists, it is deleted first.
///
/// # Parameters
///
/// * `report_directory` - Path to the root directory where the `data/` folder should be created.
///
/// # Returns
///
/// * `Ok(())` if the directory is successfully created.
/// * `Err(OpossumError)` if removing or creating the directory fails.
///
/// # Errors
///
/// * Returns an error if the existing `data/` directory cannot be deleted.
/// * Returns an error if the `data/` directory cannot be created.
pub fn recreate_data_dir<P: AsRef<Path>>(report_directory: P) -> OpmResult<()> {
    let data_dir = report_directory.as_ref().join("data"); // Use .as_ref()
    if data_dir.exists() {
        info!("Delete old report data dir");
        remove_dir_all(&data_dir)
            .map_err(|e| OpossumError::Other(format!("removing old data directory failed: {e}")))?;
    }
    create_dir(&data_dir)
        .map_err(|e| OpossumError::Other(format!("creating data directory failed: {e}")))
}

/// Constructs a `PathBuf` from a directory, a filename (without extension), and a file extension.
///
/// # Parameters
///
/// * `path` - Base directory path.
/// * `f_name` - Filename without extension.
/// * `f_ext` - File extension to be appended (e.g., `"ron"`, `"html"`).
///
/// # Returns
///
/// A `PathBuf` representing the full file path with the specified extension.
#[must_use]
pub fn create_f_path<P: AsRef<Path>>(path: P, f_name: &str, f_ext: &str) -> PathBuf {
    path.as_ref().join(f_name).with_extension(f_ext)
}
/// Sanitizes a filename by replacing invalid characters with underscores.
///
/// Invalid characters include `/`, `\`, null bytes, and control characters.
/// Leading `.` and `..` are also replaced to prevent path traversal.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    // Replace invalid characters
    let processed = name
        .replace(['/', '\\', '\0'], "_")
        .replace(|c: char| c.is_control(), "_");

    // Handle special cases for . and ..
    if processed == "." {
        "_".to_string()
    } else if processed == ".." {
        "__".to_string()
    } else {
        processed
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("valid_name"), "valid_name");
        assert_eq!(sanitize_filename("valid_name.txt"), "valid_name.txt");
        assert_eq!(sanitize_filename("invalid/name"), "invalid_name");
        assert_eq!(sanitize_filename("invalid\\name"), "invalid_name");
        assert_eq!(sanitize_filename("../invalid"), ".._invalid");
        assert_eq!(sanitize_filename("..\\invalid"), ".._invalid");
        assert_eq!(sanitize_filename("invalid\0name"), "invalid_name");
        assert_eq!(sanitize_filename("."), "_");
        assert_eq!(sanitize_filename(".."), "__");
    }
}
