use crate::{
    create_f_path,
    error::{OpmResult, OpossumError},
};
use std::{fs::File, path::Path};

/// Creates a new file at a path constructed from directory, filename, and extension,
/// and logs the operation with a user-defined description.
///
/// # Parameters
///
/// * `path` - Base directory for the file.
/// * `f_name` - Name of the file without extension.
/// * `f_ext` - File extension (e.g., `"ron"`, `"dot"`).
/// * `print_str` - A descriptive string used for logging (e.g., `"analysis report"`).
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
    let f_path = create_f_path(path, f_name, f_ext);
    File::create(f_path)
        .map_err(|e| OpossumError::Other(format!("{f_name} file creation failed: {e}")))
}
