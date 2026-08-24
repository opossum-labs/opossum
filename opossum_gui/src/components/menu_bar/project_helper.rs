use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use rfd::AsyncFileDialog;

/// Opens a file dialog for selecting an existing OPM file.
/// Returns `Some(PathBuf)` if a file was selected, or `None` otherwise.
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
        // Path selection is handled differently in WASM (via direct file inputs),
        // so this specific dialog helper returns None.
        None
    }
}

/// Opens a file dialog for saving an OPM file on desktop targets.
/// Returns `Some(PathBuf)` if a destination was chosen, or `None` otherwise.
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

/// Opens a folder selection dialog.
/// Returns `Some(PathBuf)` if a folder was selected, or `None` otherwise.
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

/// Abstracted method to save OPM content to disk or trigger a browser download
/// based on the compilation target (Desktop vs. WASM).
#[allow(clippy::unused_async)] // false positive since it only considers the non-WASM config.
pub async fn save_opm_data(path: &Path, content: &str) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // On desktop platforms, write the string directly to the local file system.
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    #[cfg(target_arch = "wasm32")]
    {
        use dioxus::document::eval;

        // Extract the filename from the provided path, or use a default name.
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project.opm");

        // JavaScript to handle the client-side download in the browser.
        let script = r#"
            // Explicitly receive the JSON payload sent from Rust (Dioxus 0.5+ requirement)
            let msg = await dioxus.recv();
            const filename = msg.filename;
            const content = msg.content;

            async function triggerSave() {
                // 1. Try the modern File System Access API first (Chrome, Edge, Opera)
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
                            // The user explicitly cancelled the file picker dialog
                            return false;
                        }
                        // Fall through to the standard download method on other execution errors
                    }
                }

                // 2. Fallback mechanism for browsers without File System Access API (e.g., Firefox)
                // We use application/octet-stream to force a file download instead of opening it in a tab.
                const blob = new Blob([content], { type: 'application/octet-stream' });
                const url = URL.createObjectURL(blob);
                
                const a = document.createElement('a');
                a.style.display = 'none';
                a.href = url;
                a.download = filename;

                document.body.appendChild(a);
                
                // Synchronously trigger the download
                a.click();
                
                // It is safe to immediately remove the anchor element from the DOM
                document.body.removeChild(a);

                // Defer URL revocation to the very end of the current JavaScript event loop.
                // This ensures the browser's download manager has securely picked up the blob reference
                // without relying on arbitrary timeouts.
                setTimeout(() => {
                    URL.revokeObjectURL(url);
                }, 0);

                return true;
            }

            return await triggerSave();
        "#;

        let mut eval_handle = eval(script);

        // Send the required data (filename and content) to the JavaScript context
        eval_handle
            .send(serde_json::json!({
                "filename": filename,
                "content": content,
            }))
            .map_err(|e| e.to_string())?;

        // Await the boolean result from the JavaScript execution
        match eval_handle.recv::<bool>().await {
            Ok(true) => Ok(()),
            Ok(false) => Err("Save operation was cancelled by user".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}
