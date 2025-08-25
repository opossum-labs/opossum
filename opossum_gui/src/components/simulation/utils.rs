use opossum_backend::{OpmResult, OpossumError};
use std::{env, path::PathBuf};

pub fn find_cli_executable() -> OpmResult<PathBuf> {
    let possible_cli_paths = vec![
        "opossum",
        "opossum.exe",
        "../../../../../debug/opossum",
        "../../../../../debug/opossum.exe",
    ];
    let gui_exe_path = env::current_exe()
        .map_err(|e| OpossumError::Other(format!("could not get gui executable path: {e}")))?;
    let gui_exe_dir = gui_exe_path.parent().ok_or(OpossumError::Other(
        "could not get parent dir of gui executable.".into(),
    ))?;
    for possible_path in possible_cli_paths {
        let test_path = gui_exe_dir.join(possible_path);
        if test_path.is_file() {
            return Ok(test_path);
        }
    }
    Err(OpossumError::Other(format!("no cli excutable found.")))
}
