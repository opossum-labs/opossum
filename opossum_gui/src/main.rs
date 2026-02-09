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
    let mut reader = Cursor::new(icon_bytes);
    let icon_dir = ico::IconDir::read(&mut reader).unwrap();
    icon_dir.entries().iter().next().map_or_else(
        || None,
        |entry| {
            let width = entry.width();
            let height = entry.height();
            entry.decode().map_or_else(
                |_| None,
                |image| {
                    let data = image.rgba_data();
                    Icon::from_rgba(data.into(), width, height).ok()
                },
            )
        },
    )
}

#[cfg(all(not(debug_assertions), not(target_arch = "wasm32")))]
fn start_backend() -> ProcessHandle {
    use std::env;
    let gui_exe_path = env::current_exe().expect("could not get current executable path: {e}");
    let gui_exe_dir = gui_exe_path.parent().expect("could not get executable dir");
    use std::process::Command;
    #[cfg(target_os = "windows")]
    let backend_path = gui_exe_dir.join("opossum_backend.exe");
    #[cfg(target_os = "linux")]
    let backend_path = gui_exe_dir.join("opossum_backend");
    println!("Starting backend server... at {}", backend_path.display());
    let mut command = Command::new(backend_path);
    // On Windows, you might need to prevent a new console window from opening.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child_process = command.spawn().expect("Failed to backend server.");
    println!("Backend server started with PID: {}", child_process.id());
    ProcessHandle::new(child_process)
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

    // Release-Build: start backend an return handle
    #[cfg(not(debug_assertions))]
    {
        let backend_handle = start_backend();
        launch_app(backend_handle);
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
        App {}
    }
}
