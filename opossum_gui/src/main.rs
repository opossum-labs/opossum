#![windows_subsystem = "windows"]
#![allow(clippy::items_after_statements)]
use dioxus::{desktop::tao::window::Icon, prelude::*};
use opossum_gui::App;
use std::{
    io::Cursor,
    process::Child,
    sync::{Arc, Mutex},
};

const MAIN_CSS: Asset = asset!("./assets/main.css");
// const PLOTLY_JS: Asset = asset!("./assets/plotly.js");
// const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
// const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
const MDB_CSS: Asset = asset!("./assets/mdb.min.css");
const MDB_JS: Asset = asset!("./assets/mdb.umd.min.js");
const MDB_SUB_CSS: Asset = asset!("./assets/mdb_submenu.css");
const MDB_ACC_CSS: Asset = asset!("./assets/mdb_accordion.css");

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
#[cfg(not(debug_assertions))]
fn start_backend() -> ProcessHandle {
    use std::env;
    let gui_exe_path = env::current_exe().expect("could not get current executable path: {e}");
    let gui_exe_dir = gui_exe_path.parent().expect("could not get executable dir");
    use std::process::Command;
    #[cfg(target_os = "windows")]
    let backend_path = gui_exe_dir.join("opossum_backend.exe");
    #[cfg(target_os = "linux")]
    let backend_path = gui_exe_dir.join("../lib/OpossumGui/opossum_backend");
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
#[derive(Clone, Default)]
struct ProcessHandle {
    #[allow(dead_code)]
    inner: Option<Arc<Mutex<Child>>>,
}
#[cfg(not(debug_assertions))]
impl ProcessHandle {
    pub fn new(child: Child) -> Self {
        Self {
            inner: Some(Arc::new(Mutex::new(child))),
        }
    }
    pub fn kill(&self) {
        println!("Attempting to terminate backend server...");
        if let Some(child) = &self.inner {
            let mut handle = child.lock().unwrap();
            match handle.kill() {
                Ok(_) => {
                    // Wait for the process to ensure it's fully cleaned up
                    let _ = handle.wait();
                    println!("Backend server terminated successfully.");
                }
                Err(e) => eprintln!("Error terminating backend server: {}", e),
            }
        }
    }
}
fn main() {
    #[cfg(feature = "desktop")]
    fn launch_app(backend_handle: ProcessHandle) {
        println!("Launching GUI...");
        use directories::ProjectDirs;
        let data_dir = ProjectDirs::from("org", "OpossumLabs", "OpossumGui").map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            |proj_dirs| proj_dirs.data_local_dir().to_path_buf(),
        );
        let window = dioxus::desktop::WindowBuilder::new()
            .with_decorations(false)
            .with_window_icon(read_icon())
            .with_title("Opossum");
        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new()
                    .with_window(window)
                    .with_data_directory(data_dir)
            )
            .with_context(backend_handle)
            .launch(MainApp);
    }
    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        dioxus::launch(MainApp);
    }
    #[cfg(not(debug_assertions))]
    {
        let backend_handle = start_backend();
        launch_app(backend_handle);
    }
    #[cfg(debug_assertions)]
    {
        launch_app(ProcessHandle::default());
    }
}

#[component]
fn MainApp() -> Element {
    #[cfg(not(debug_assertions))]
    {
        let backend_handle = use_context::<ProcessHandle>();
        use_drop(move || {
            backend_handle.kill();
            println!("Stopping app...")
        });
    }
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Stylesheet { href: MDB_CSS }
        document::Stylesheet { href: MDB_SUB_CSS }
        document::Stylesheet { href: MDB_ACC_CSS }
        document::Script { src: MDB_JS }
        App {}
    }
}
