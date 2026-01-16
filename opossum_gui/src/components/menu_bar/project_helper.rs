use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use rfd::AsyncFileDialog;

/// Opens file dialog for selectin an existing OPM file.
/// Returns `Some(PathBuf)` or `None` otherwise.
pub async fn select_open_path() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = AsyncFileDialog::new()
            .set_directory("/")
            .set_title("Open OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .pick_file()
            .await;

        file.map(|handle| handle.path().to_path_buf())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Opens file dialog for saving an OPM file
pub async fn select_save_path() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = AsyncFileDialog::new()
            .set_directory("/")
            .set_title("Save OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .save_file()
            .await;

        file.map(|handle| handle.path().to_path_buf())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Opens file dialog for selction of a folder.
pub async fn select_folder_path() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let folder = AsyncFileDialog::new()
            .set_directory("./")
            .set_title("Select OPOSSUM report directory")
            .pick_folder()
            .await;

        folder.map(|handle| handle.path().to_path_buf())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}
