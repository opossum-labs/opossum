#![allow(clippy::volatile_composites)]

use dioxus::prelude::*;
use opossum_gui::App;

// --- dektop specific imports ---
#[cfg(not(target_arch = "wasm32"))]
use {
    dioxus::desktop::{WindowBuilder, tao::window::Icon},
    directories::ProjectDirs,
    opossum_gui::ProcessHandle,
    std::io::Cursor,
};

const MAIN_CSS: Asset = asset!("/assets/main.css");
const DX_COMPONENT_CSS: Asset = asset!("/assets/dx-components-theme.css");
// const PLOTLY_JS: Asset = asset!("/assets/plotly.js");
// const THREE_MOD_JS: Asset = asset!("/assets/three_mod.js");
// const ORBIT_CTRLS: Asset = asset!("/assets/orbitControls.js");
const MDB_CSS: Asset = asset!("/assets/mdb.min.css");
const MDB_JS: Asset = asset!("/assets/mdb.umd.min.js");
const MDB_SUB_CSS: Asset = asset!("/assets/mdb_submenu.css");
const MDB_ACC_CSS: Asset = asset!("/assets/mdb_accordion.css");

// --- desktop only functions ---
#[cfg(not(target_arch = "wasm32"))]
fn read_icon() -> Option<Icon> {
    let icon_bytes: &[u8] = include_bytes!("../../opossum_core/logo/Logo_square.ico");
    let img = image::load_from_memory(icon_bytes).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

#[cfg(all(not(debug_assertions), not(target_arch = "wasm32")))]
fn start_backend() -> Result<ProcessHandle, String> {
    use std::env;
    use std::io::Read;
    use std::process::{Command, Stdio}; // Added Stdio for piping
    use std::thread;
    use std::time::Duration; // Added Read to extract the string from stderr

    // Safely get the executable path
    let gui_exe_path =
        env::current_exe().map_err(|e| format!("Could not get current executable path: {}", e))?;

    let gui_exe_dir = gui_exe_path
        .parent()
        .ok_or("Could not get executable directory.")?;

    #[cfg(target_os = "windows")]
    let backend_path = gui_exe_dir.join("opossum_backend.exe");
    #[cfg(target_os = "linux")]
    let backend_path = gui_exe_dir.join("opossum_backend");

    println!("Starting backend server... at {}", backend_path.display());

    // 1. Check if the executable exists
    if !backend_path.exists() {
        return Err(format!(
            "The backend executable was not found.\nPath: {}",
            backend_path.display()
        ));
    }

    let mut command = Command::new(&backend_path);

    // Pipe the standard error output so the frontend can read it!
    command.stderr(Stdio::piped());

    // Prevent a new console window from opening on Windows
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    // 2. Attempt to start the process
    match command.spawn() {
        Ok(mut child_process) => {
            // 3. Wait briefly to give the backend time to fail if the port is blocked
            thread::sleep(Duration::from_millis(200));

            // Check if the process has already exited
            if let Ok(Some(status)) = child_process.try_wait() {
                // 4. Extract the exact error message from stderr
                let mut error_details = String::new();
                if let Some(mut stderr) = child_process.stderr.take() {
                    // Ignore read errors here, we just want the string if it exists
                    let _ = stderr.read_to_string(&mut error_details);
                }

                // 5. Format the message for the user depending on whether we got a string
                let error_msg = if error_details.trim().is_empty() {
                    format!(
                        "The backend server crashed with Exit Status: {}.\nNo further details were provided.",
                        status
                    )
                } else {
                    format!(
                        "The backend server failed to start.\n\nBackend Error:\n{}",
                        error_details.trim()
                    )
                };

                return Err(error_msg);
            }

            println!("Backend server started with PID: {}", child_process.id());
            Ok(ProcessHandle::new(child_process))
        }
        Err(e) => Err(format!("Failed to execute the backend server: {}", e)),
    }
}

// --- Desktop Main ---
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    fn launch_app(backend_handle: ProcessHandle) {
        println!("Launching GUI...");
        let data_dir = ProjectDirs::from("org", "OpossumLabs", "OpossumGui").map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            |proj_dirs| proj_dirs.data_local_dir().to_path_buf(),
        );
        let window = WindowBuilder::new()
            .with_decorations(false)
            .with_window_icon(read_icon())
            .with_title("Opossum");
        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new()
                    .with_window(window)
                    .with_data_directory(data_dir),
            )
            .with_context(backend_handle)
            .launch(MainApp);
    }

    // Release-Build: start backend and handle potential errors
    #[cfg(not(debug_assertions))]
    {
        match start_backend() {
            Ok(backend_handle) => {
                launch_app(backend_handle);
            }
            Err(error_message) => {
                // Show native error dialog using rfd
                rfd::MessageDialog::new()
                    .set_title("OPOSSUM - Startup Error")
                    .set_description(&error_message)
                    .set_level(rfd::MessageLevel::Error)
                    .show();

                // Exit the application gracefully with an error code
                std::process::exit(1);
            }
        }
    }

    // Debug-Build: return dummy handle
    #[cfg(debug_assertions)]
    {
        launch_app(ProcessHandle::default());
    }
}

// --- WASM Main ---
#[cfg(target_arch = "wasm32")]
fn main() {
    // simple start for WASM builds (no backend)
    dioxus::launch(MainApp);
}

#[component]
fn MainApp() -> Element {
    #[cfg(all(not(target_arch = "wasm32"), not(debug_assertions)))]
    {
        use crate::dioxus_core::use_drop;
        let backend_handle = use_context::<ProcessHandle>();
        use_drop(move || {
            backend_handle.kill();
            println!("Stopping app...")
        });
    }
    rsx! {
        document::Stylesheet { href: DX_COMPONENT_CSS }
        document::Stylesheet { href: MAIN_CSS }
        document::Stylesheet { href: MDB_CSS }
        document::Stylesheet { href: MDB_SUB_CSS }
        document::Stylesheet { href: MDB_ACC_CSS }
        document::Script { src: MDB_JS }

        // Disable the default browser context menu globally, but only in release builds.
        // Custom Dioxus context menus (e.g., in the Graph Editor) will still work
        // because preventDefault() only stops the browser's native menu.
        if !cfg!(debug_assertions) {
            script { "document.addEventListener('contextmenu', event => event.preventDefault());" }
        }

        App {}
    }
}
