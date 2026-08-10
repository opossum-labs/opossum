use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use rfd::AsyncFileDialog;

/// Opens file dialog for selecting an existing OPM file.
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

/// Opens file dialog for saving an OPM file on desktop targets.
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

/// Opens file dialog for selection of a folder.
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

/// Abstracted method to save OPM content to disk or trigger browser download based on the compile target.
pub async fn save_opm_data(path: &Path, content: &str) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    #[cfg(target_arch = "wasm32")]
    {
        use dioxus::document::eval;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project.opm");

        let script = r#"
            const filename = msg.filename;
            const content = msg.content;

            async function triggerSave() {
                // Try modern File System Access API first (shows 'Save As' dialog)
                if ('showSaveFilePicker' in window) {
                    try {
                        const handle = await window.showSaveFilePicker({
                            suggestedName: filename,
                            types: [{
                                description: 'OPOSSUM setup file',
                                accept: { 'text/plain': ['.opm'] },
                            }],
                        });
                        const writable = await handle.createWritable();
                        await writable.write(content);
                        await writable.close();
                        return true;
                    } catch (err) {
                        if (err.name === 'AbortError') {
                            // User cancelled the save dialog
                            return false;
                        }
                    }
                }

                // Fallback for standard browser file download
                const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = filename;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                URL.revokeObjectURL(url);
                return true;
            }

            return await triggerSave();
        "#;

        let mut eval_handle = eval(script);
        eval_handle
            .send(serde_json::json!({
                "filename": filename,
                "content": content,
            }))
            .map_err(|e| e.to_string())?;

        match eval_handle.recv::<bool>().await {
            Ok(true) => Ok(()),
            Ok(false) => Err("Save operation cancelled by user".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}