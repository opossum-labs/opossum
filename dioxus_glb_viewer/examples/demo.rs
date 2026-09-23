//! Demo app for `dioxus_glb_viewer`.
//!
//! # Running
//!
//! ```bash
//! cd dioxus_glb_viewer
//! dx serve --example demo --features desktop --platform desktop
//! ```
//!
//! # Acceptance test script (manual)
//!
//! 1. Start the app (with or without a sample model).
//! 2. Click "Add" → model appears. Rotate and zoom into a distinctive oblique view.
//! 3. Click "Add" again → second model appears; **view must not move**.
//! 4. Click "Remove last" → one model disappears; **view stays**.
//! 5. Click "Replace first" → first model with a new transform; **view stays**.
//! 6. Click "Toggle visibility" → model becomes invisible/visible; **view stays**.
//! 7. Click on a model → correct ID + `mesh_name` in the panel, BoxHelper appears.
//!    Click empty space → `id: None`. **Drag** (orbit) → *no* pick event.
//! 8. Resize the window → image stays undistorted (ResizeObserver).
//! 9. 20× add/remove alternating → memory in DevTools stays flat (disposal test).
//! 10. Desktop only: type an absolute path into the input field → loads as bytes source.

#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus_glb_viewer::{
    GlbObject, GlbSource, GlbViewer, PickEvent, Transform, ViewerEvent, ViewerOptions,
    use_glb_viewer_handle,
};

/// Optional sample model.
/// `option_asset!` returns `None` instead of a compile error when the file is absent,
/// so a fresh clone always builds.
const SAMPLE: Option<Asset> = option_asset!("/examples/assets/sample.glb");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut objects = use_signal(Vec::<GlbObject>::new);
    let mut last_pick = use_signal(|| None::<PickEvent>);
    let mut event_log = use_signal(Vec::<String>::new);
    let mut counter = use_signal(|| 0usize);
    let viewer_handle = use_glb_viewer_handle();

    // ── Helper: build a GlbObject with an incrementing ID ────────────────────
    let make_object = move || -> Option<GlbObject> {
        let src = SAMPLE.map(|a| GlbSource::Url(a.to_string()))?;
        let n = *counter.read();
        // Offset each object slightly so they don't stack.
        let offset = n as f32 * 2.0;
        Some(GlbObject {
            id: format!("obj-{n}"),
            source: src,
            transform: Transform {
                position: [offset, 0.0, 0.0],
                ..Transform::default()
            },
            visible: true,
            selected: false,
        })
    };

    rsx! {
        // Basic layout: side panel on the left, viewer on the right
        div {
            style: "display:flex; height:100vh; background:#111; color:#eee; font-family:sans-serif;",

            // ── Side panel ───────────────────────────────────────────────────
            aside {
                style: "width:260px; padding:12px; background:#1a1a1a; overflow-y:auto; flex-shrink:0; border-right:1px solid #333;",

                h3 { style: "margin:0 0 8px;", "dioxus_glb_viewer Demo" }

                // Sample model notice
                if SAMPLE.is_none() {
                    p {
                        style: "font-size:12px; color:#fa0; background:#2a2000; padding:8px; border-radius:4px;",
                        "No sample model. Place a .glb as "
                        code { "examples/assets/sample.glb" }
                        " and restart."
                    }
                }

                // ── Buttons ──────────────────────────────────────────────────
                div { style: "display:flex; flex-direction:column; gap:6px; margin:8px 0;",
                    button {
                        style: button_style(!SAMPLE.is_none()),
                        disabled: SAMPLE.is_none(),
                        onclick: move |_| {
                            if let Some(obj) = make_object() {
                                objects.write().push(obj);
                                *counter.write() += 1;
                            }
                        },
                        "Add"
                    }
                    button {
                        style: button_style(true),
                        onclick: move |_| { objects.write().pop(); },
                        "Remove last"
                    }
                    button {
                        style: button_style(!SAMPLE.is_none() && !objects.read().is_empty()),
                        disabled: SAMPLE.is_none() || objects.read().is_empty(),
                        onclick: move |_| {
                            // Swap the source of the first object to a new String → Reload op.
                            if let Some(obj) = objects.write().first_mut() {
                                if let Some(a) = SAMPLE {
                                    // New String allocation = new WireSource → Reload op
                                    obj.source = GlbSource::Url(format!("{}?v={}", a, *counter.read()));
                                    obj.transform.position[0] += 0.5; // visible change
                                }
                            }
                        },
                        "Replace first"
                    }
                    button {
                        style: button_style(objects.read().iter().any(|o| o.selected)),
                        disabled: !objects.read().iter().any(|o| o.selected),
                        onclick: move |_| {
                            for obj in objects.write().iter_mut() {
                                if obj.selected {
                                    obj.visible = !obj.visible;
                                }
                            }
                        },
                        "Toggle visibility"
                    }
                    button {
                        style: button_style(true),
                        onclick: move |_| { viewer_handle.fit_view(); },
                        "Fit view"
                    }
                    button {
                        style: button_style(true),
                        onclick: move |_| { viewer_handle.reset_camera(); },
                        "Reset camera"
                    }
                    button {
                        style: button_style(true),
                        onclick: move |_| { viewer_handle.clear_scene(); objects.write().clear(); },
                        "Clear all"
                    }
                }

                // ── Desktop: load an arbitrary GLB file by path ───────────────
                // The desktop build cannot load file:// URLs; the file is read in Rust
                // and sent as GlbSource::Bytes. This example requires
                // `required-features = ["desktop"]`, so it is never wasm32 — no #[cfg] needed.
                div { style: "margin-top:8px;",
                    p { style: "font-size:11px; color:#888; margin:0 0 4px;", "Path to .glb (desktop bytes test):" }
                    input {
                        r#type: "text",
                        placeholder: r"C:\path\to\file.glb",
                        style: "width:100%; box-sizing:border-box; background:#222; color:#eee; border:1px solid #444; padding:4px; font-size:11px;",
                        onchange: move |e| {
                            let path = e.value();
                            if path.is_empty() { return; }
                            match std::fs::read(&path) {
                                Ok(bytes) => {
                                    let arc: std::sync::Arc<[u8]> = std::sync::Arc::from(bytes.as_slice());
                                    let n = *counter.read();
                                    objects.write().push(GlbObject {
                                        id: format!("file-{n}"),
                                        source: GlbSource::Bytes(arc),
                                        transform: Transform {
                                            position: [(n as f32) * 2.0, 0.0, 0.0],
                                            ..Transform::default()
                                        },
                                        visible: true,
                                        selected: false,
                                    });
                                    *counter.write() += 1;
                                }
                                Err(e) => {
                                    event_log.write().push(format!("File error: {e}"));
                                }
                            }
                        },
                    }
                }

                // ── Object list ───────────────────────────────────────────────
                h4 { style: "margin:12px 0 4px; font-size:12px;", "Objects:" }
                ul { style: "list-style:none; padding:0; margin:0; font-size:11px;",
                    for obj in objects.read().iter() {
                        li {
                            style: "padding:2px 0; color:#aaa;",
                            "{obj.id}  vis={obj.visible}  sel={obj.selected}"
                        }
                    }
                    if objects.read().is_empty() {
                        li { style: "color:#555;", "(empty)" }
                    }
                }

                // ── Last pick ─────────────────────────────────────────────────
                h4 { style: "margin:12px 0 4px; font-size:12px;", "Last pick:" }
                p {
                    style: "font-size:11px; color:#aaa; margin:0;",
                    {
                        last_pick.read().as_ref().map_or_else(
                            || "—".to_string(),
                            |p| p.id.clone().unwrap_or_else(|| "(miss)".into()),
                        )
                    }
                }

                // ── Event log ─────────────────────────────────────────────────
                h4 { style: "margin:12px 0 4px; font-size:12px;", "Events:" }
                ul { style: "list-style:none; padding:0; margin:0; font-size:10px; max-height:200px; overflow-y:auto;",
                    for line in event_log.read().iter().rev().take(20) {
                        li { style: "padding:1px 0; color:#888; word-break:break-all;", "{line}" }
                    }
                }
            }

            // ── Viewer ───────────────────────────────────────────────────────
            GlbViewer {
                style: "flex:1; height:100%;",
                objects,
                handle: viewer_handle,
                options: use_signal(|| ViewerOptions {
                    background: "#1a1a2e".into(),
                    fit_on_first_load: true,
                    ..ViewerOptions::default()
                }),
                on_pick: move |p: PickEvent| {
                    // Selection lives in the caller's model — the component only reports the click.
                    let hit_id = p.id.clone();
                    for obj in objects.write().iter_mut() {
                        obj.selected = Some(&obj.id) == hit_id.as_ref();
                    }
                    last_pick.set(Some(p));
                },
                on_event: move |e: ViewerEvent| {
                    event_log.write().push(format!("{e:?}"));
                },
            }
        }
    }
}

fn button_style(enabled: bool) -> &'static str {
    if enabled {
        "padding:6px 10px; background:#334; border:1px solid #556; color:#eee; cursor:pointer; border-radius:3px;"
    } else {
        "padding:6px 10px; background:#222; border:1px solid #333; color:#555; cursor:not-allowed; border-radius:3px;"
    }
}
