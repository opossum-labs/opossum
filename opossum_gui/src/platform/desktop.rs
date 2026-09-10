// src/platform/desktop.rs

use std::process::Child;
use std::sync::{Arc, Mutex};

use dioxus::desktop::{WindowBuilder, tao::window::Icon};
use dioxus::prelude::*;
use directories::ProjectDirs;

#[derive(Clone, Default)]
pub struct ProcessHandle {
    #[allow(dead_code)]
    inner: Option<Arc<Mutex<Child>>>,
}

impl ProcessHandle {
    #[cfg(not(debug_assertions))]
    pub fn new(child: Child) -> Self {
        Self {
            inner: Some(Arc::new(Mutex::new(child))),
        }
    }

    /// Terminates the child backend process if one is currently active.
    #[expect(dead_code)] // only called in release mode
    pub fn kill(&self) {
        if let Some(child) = &self.inner {
            println!("Attempting to terminate backend server...");
            let mut handle = child.lock().unwrap();
            match handle.kill() {
                Ok(()) => {
                    // Wait for the process to ensure it is completely cleaned up
                    let _ = handle.wait();
                    drop(handle);
                    println!("Backend server terminated successfully.");
                }
                Err(e) => eprintln!("Error terminating backend server: {e}"),
            }
        }
    }
}

/// Decodes the window icon embedded in the application binary.
fn read_icon() -> Option<Icon> {
    // Relative path from src/platform/ to assets/
    let icon_bytes: &[u8] = include_bytes!("../../assets/icons/32x32.png");

    let img = image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

/// Spawns the opossum_backend binary alongside the frontend in release builds.
#[cfg(not(debug_assertions))]
fn start_backend() -> Result<ProcessHandle, String> {
    use std::env;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    let gui_exe_path =
        env::current_exe().map_err(|e| format!("Could not get current executable path: {e}"))?;

    let gui_exe_dir = gui_exe_path
        .parent()
        .ok_or("Could not get executable directory.")?;

    #[cfg(target_os = "windows")]
    let backend_path = gui_exe_dir.join("opossum_backend.exe");
    #[cfg(target_os = "linux")]
    let backend_path = gui_exe_dir.join("opossum_backend");

    println!("Starting backend server at {}", backend_path.display());

    if !backend_path.exists() {
        return Err(format!(
            "The backend executable was not found.\nPath: {}",
            backend_path.display()
        ));
    }

    let mut command = Command::new(&backend_path);
    command.stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match command.spawn() {
        Ok(mut child_process) => {
            thread::sleep(Duration::from_millis(200));

            if let Ok(Some(status)) = child_process.try_wait() {
                let mut error_details = String::new();
                if let Some(mut stderr) = child_process.stderr.take() {
                    let _ = stderr.read_to_string(&mut error_details);
                }

                let error_msg = if error_details.trim().is_empty() {
                    format!(
                        "The backend server crashed with Exit Status: {status}.\nNo further details provided."
                    )
                } else {
                    format!(
                        "The backend server failed to start.\n\nBackend Error:\n{error_details}"
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

/// Helper to configure and launch the Dioxus desktop window.
fn launch_desktop_window(backend_handle: ProcessHandle, root_component: fn() -> Element) {
    dioxus::logger::init(dioxus::logger::tracing::Level::INFO).ok();
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
        .launch(root_component);
}

/// Public platform launch entry point for desktop environments.
pub fn launch(root_component: fn() -> Element) {
    #[cfg(not(debug_assertions))]
    {
        match start_backend() {
            Ok(backend_handle) => {
                launch_desktop_window(backend_handle, root_component);
            }
            Err(error_message) => {
                rfd::MessageDialog::new()
                    .set_title("OPOSSUM - Startup Error")
                    .set_description(&error_message)
                    .set_level(rfd::MessageLevel::Error)
                    .show();

                std::process::exit(1);
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        launch_desktop_window(ProcessHandle::default(), root_component);
    }
}

/// Lifecycle wrapper component to manage backend process shutdown on exit.
#[component]
pub fn PlatformLifecycle(children: Element) -> Element {
    #[cfg(not(debug_assertions))]
    {
        let backend_handle = use_context::<ProcessHandle>();
        use_drop(move || {
            backend_handle.kill();
            println!("Stopping app...");
        });
    }

    rsx! {
        {children}
    }
}
