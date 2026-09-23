//! # `dioxus_glb_viewer`
//!
//! A self-contained, opossum-independent Dioxus component that renders `.glb` files
//! (glTF-Binary) with three.js inside a `<canvas>`.
//!
//! ## Features
//!
//! - **`OrbitControls`** for free navigation (orbit, dolly, pan).
//! - **Declarative scene**: Scene contents are passed as `Vec<GlbObject>`; adding,
//!   removing, and replacing models does **not** move the camera.
//! - **Click-picking**: A left-click (not an orbit drag) reports the caller-assigned ID
//!   of the hit object back to Rust — ideal for configuration menus and live object
//!   redefinition.
//! - **`GlbSource::Url`** for HTTP/asset sources; **`GlbSource::Bytes`** for arbitrary
//!   files from the filesystem (on the desktop wry cannot load `file://` URLs).
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use dioxus::prelude::*;
//! use dioxus_glb_viewer::{GlbObject, GlbSource, GlbViewer, PickEvent, Transform};
//!
//! #[component]
//! fn App() -> Element {
//!     let objects = use_signal(|| {
//!         vec![GlbObject {
//!             id: "my-model".into(),
//!             source: GlbSource::Url("https://example.com/model.glb".into()),
//!             transform: Transform::default(),
//!             visible: true,
//!             selected: false,
//!         }]
//!     });
//!     rsx! {
//!         GlbViewer {
//!             objects,
//!             on_pick: move |p: PickEvent| { println!("pick: {:?}", p.id); },
//!         }
//!     }
//! }
//! ```
//!
//! ## Running the demo
//!
//! ```bash
//! cd dioxus_glb_viewer
//! dx serve --example demo --features desktop --platform desktop
//! ```

mod diff;
mod event;
mod model;
mod op;
mod viewer;
mod wire;

pub use event::{PickEvent, ViewerEvent};
pub use model::{GlbObject, GlbSource, Transform, ViewerOptions};
pub use viewer::{GlbViewer, ViewerHandle, use_glb_viewer_handle};
