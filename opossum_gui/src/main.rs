#![allow(clippy::volatile_composites)]

use dioxus::prelude::*;
use opossum_gui::App;

// --- Desktop-specific imports (only compiled when the "desktop" feature is active) ---
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
use {
    dioxus::desktop::{tao::window::Icon, WindowBuilder},
    directories::ProjectDirs,
    std::io::Cursor,
};

// --- General non-WASM imports ---
#[cfg(not(target_arch = "wasm32"))]
use opossum_gui::ProcessHandle;

const MAIN_CSS: Asset = asset!("/assets/main.css");
const DX_COMPONENT_CSS: Asset = asset!("/assets/dx-components-theme.css");
const MDB_CSS: Asset = asset!("/assets/mdb.min.css");
const MDB_JS: Asset = asset!("/assets/mdb.umd.min.js");
const MDB_SUB_CSS: Asset = asset!("/assets/mdb_submenu.css");
const MDB_ACC_CSS: Asset = asset!("/assets/mdb_accordion.css");

// --- Desktop-only helper functions (Wry/Tao window icons) ---
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
fn read_icon() -> Option<Icon> {
    let icon_bytes: &[u8] = include_bytes!("../../opossum_core/logo/Logo_square.ico");
    let mut reader = Cursor::new(icon_bytes);
    let icon_dir = ico::IconDir::read(&mut reader).ok()?;
    let entry = icon_dir.entries().first()?;
    let width = entry.width();
    let height = entry.height();
    let image = entry.decode().ok()?;
    let data = image.rgba_data();
    Icon::from_rgba(data.into(), width, height).ok()
}

// --- Non-WASM backend process handler (runs on both desktop and native renderers) ---
#[cfg(all(not(debug_assertions), not(target_arch = "wasm32")))]
fn start_backend() -> Result<ProcessHandle, String> {
    use std::env;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    // Safely get the executable path
    let gui_exe_path =
        env::current_exe().map_err(|e| format!("Could not get current executable path: {e}"))?;

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

    // Pipe standard error output to read potential startup issues
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
            // 3. Wait briefly to give the backend time to report immediate failure
            thread::sleep(Duration::from_millis(200));

            // Check if the process has already exited
            if let Ok(Some(status)) = child_process.try_wait() {
                // 4. Extract error details from stderr
                let mut error_details = String::new();
                if let Some(mut stderr) = child_process.stderr.take() {
                    let _ = stderr.read_to_string(&mut error_details);
                }

                let error_msg = if error_details.trim().is_empty() {
                    format!(
                        "The backend server crashed with Exit Status: {status}.\nNo further details were provided."
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
        Err(e) => Err(format!("Failed to execute the backend server: {e}")),
    }
}

// --- Desktop Launch implementation (Wry / Tao backend with custom window settings) ---
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
fn launch_app(backend_handle: ProcessHandle) {
    println!("Launching GUI (Desktop Mode)...");
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

// --- Native Renderer Launch implementation (Experimental Blitz / Winit backend) ---
#[cfg(all(not(target_arch = "wasm32"), not(feature = "desktop")))]
fn launch_app(backend_handle: ProcessHandle) {
    println!("Launching GUI (Native Renderer Mode)...");
    dioxus::LaunchBuilder::new()
        .with_context(backend_handle)
        .launch(MainApp);
}

// --- Native Main Entry Point ---
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Release build: start backend process and handle launch errors
    #[cfg(not(debug_assertions))]
    {
        match start_backend() {
            Ok(backend_handle) => {
                launch_app(backend_handle);
            }
            Err(error_message) => {
                // Display native error dialog using rfd
                rfd::MessageDialog::new()
                    .set_title("OPOSSUM - Startup Error")
                    .set_description(&error_message)
                    .set_level(rfd::MessageLevel::Error)
                    .show();

                std::process::exit(1);
            }
        }
    }

    // Debug build: pass default dummy handle
    #[cfg(debug_assertions)]
    {
        launch_app(ProcessHandle::default());
    }
}

// --- WASM Main Entry Point ---
#[cfg(target_arch = "wasm32")]
fn main() {
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
            println!("Stopping app...");
        });
    }
    rsx! {
        document::Stylesheet { href: DX_COMPONENT_CSS }
        document::Stylesheet { href: MAIN_CSS }
        document::Stylesheet { href: MDB_CSS }
        document::Stylesheet { href: MDB_SUB_CSS }
        document::Stylesheet { href: MDB_ACC_CSS }
        document::Script { src: MDB_JS }

        // Disable browser context menu in release builds
        if !cfg!(debug_assertions) {
            script { "document.addEventListener('contextmenu', event => event.preventDefault());" }
        }

        App {}
    }
}