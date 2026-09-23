//! Dioxus component [`GlbViewer`] and the associated [`ViewerHandle`].

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::document::{self, Eval};
use dioxus::prelude::*;

use crate::diff;
use crate::event::{PickEvent, ViewerEvent};
use crate::model::{GlbObject, ViewerOptions};
use crate::op::{Op, WireEvent};
use crate::wire;

// ─── Assets ──────────────────────────────────────────────────────────────────

/// The JS viewer module (ES module, unprocessed — not minified as a classic script).
#[allow(clippy::volatile_composites)]
const VIEWER_JS: Asset = asset!("/assets/viewer.js");
/// CSS for `.dxglb-root` and `.dxglb-canvas`.
#[allow(clippy::volatile_composites)]
const VIEWER_CSS: Asset = asset!("/assets/viewer.css");
/// Base directory path of the folder asset containing three.js (unhashed, structure-preserving).
/// Used as the base for importmap URLs.
#[allow(clippy::volatile_composites)]
const THREE_DIR: Asset = asset!("/assets/three");

// ─── Instance counter ─────────────────────────────────────────────────────────

/// Global counter for unique canvas IDs per viewer instance.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

// ─── ViewerHandle ─────────────────────────────────────────────────────────────

/// Imperative handle for the few operations that move the camera.
///
/// Created via [`use_glb_viewer_handle`] and passed to [`GlbViewer`]. Contains a copy
/// of the `Eval` through which `fit_view`/`reset_camera` ops are sent directly to the
/// eval channel, bypassing the object diff.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ViewerHandle(CopyValue<Option<Eval>>);

impl ViewerHandle {
    /// Frames all currently visible models. **Moves the camera.**
    pub fn fit_view(self) {
        if let Some(eval) = *self.0.read() {
            let _ = eval.send(Op::FitView);
        }
    }

    /// Resets the camera to the initial pose configured in [`ViewerOptions`].
    /// **Moves the camera.**
    pub fn reset_camera(self) {
        if let Some(eval) = *self.0.read() {
            let _ = eval.send(Op::ResetCamera);
        }
    }

    /// Removes all models from the scene and releases their GPU resources.
    ///
    /// For normal scene clearing it is enough to set `objects` to an empty `Vec` —
    /// this op exists as an escape hatch for scenarios where a single op is more
    /// efficient than N×`Remove`.
    pub fn clear_scene(self) {
        if let Some(eval) = *self.0.read() {
            let _ = eval.send(Op::Clear);
        }
    }
}

/// Creates a [`ViewerHandle`] that can be passed to [`GlbViewer`].
///
/// The handle can be stored and called from any number of places; it is `Copy` and
/// `PartialEq`.
#[must_use]
pub fn use_glb_viewer_handle() -> ViewerHandle {
    ViewerHandle(use_hook(|| CopyValue::new(None)))
}

// ─── GlbViewer ───────────────────────────────────────────────────────────────

/// Renders `.glb` models with three.js inside a `<canvas>`.
///
/// # Camera stability
///
/// Adding, removing, replacing, and all field updates on objects do **not** move the
/// camera. Only `ViewerHandle::fit_view()` and `ViewerHandle::reset_camera()` may do so.
///
/// # Static canvas shape (maintenance constraint)
///
/// The RSX template has exactly one `div.dxglb-root` with one `canvas#dxglb-<N>`. No
/// `if`, no `key`, no interpolation around that node — Dioxus may only patch canvas
/// attributes, never replace the node. Any reconstruction of the canvas destroys the
/// WebGL context and the camera.
///
/// # Arguments
///
/// * `objects`    — Scene description. Diffed against the last known state.
/// * `on_pick`    — Called for every left-click (including a miss).
/// * `on_event`   — Optional; load progress, errors, ready signal.
/// * `options`    — Optional; scene options. Changes are sent live.
/// * `handle`     — Optional; imperative camera handle.
/// * `attributes` — Forwarded to the outer `div`.
#[component]
pub fn GlbViewer(
    objects: ReadSignal<Vec<GlbObject>>,
    on_pick: EventHandler<PickEvent>,
    #[props(default)] on_event: Option<EventHandler<ViewerEvent>>,
    #[props(default)] options: ReadSignal<ViewerOptions>,
    #[props(default)] handle: Option<ViewerHandle>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    // ── Canvas ID: created once per instance ─────────────────────────────────
    // use_hook runs exactly once → ID never changes → canvas is never recreated.
    let canvas_id = use_hook(|| format!("dxglb-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)));

    // ── Eval: started once ───────────────────────────────────────────────────
    // Deliberately in use_hook (not in an Effect): the boot script polls for the canvas
    // itself, so there is no race. This way the eval exists BEFORE the first diff runs —
    // an Effect would have a window where ops could arrive without a channel.
    // Eval is Copy; its storage belongs to the JS side, not this scope — copies in tasks
    // and in use_drop are therefore valid.
    let eval = use_hook(|| {
        let opts = options.peek().clone();
        document::eval(&build_boot_script(&canvas_id, &opts))
    });

    // ── Connect handle to eval ───────────────────────────────────────────────
    use_hook(|| {
        if let Some(mut h) = handle {
            *h.0.write() = Some(eval);
        }
    });

    // ── JS → Rust: receive events ─────────────────────────────────────────────
    // on_pick and on_event are updated in-place by the props macro (point_to + mark_dirty),
    // so they are always current even if the caller passes a new closure.
    use_future(move || async move {
        let mut e = eval; // Copy
        while let Ok(evt) = e.recv::<WireEvent>().await {
            match evt {
                WireEvent::Pick {
                    id,
                    mesh_name,
                    point,
                    shift,
                    ctrl,
                    alt,
                } => {
                    on_pick.call(PickEvent {
                        id,
                        mesh_name,
                        point,
                        shift,
                        ctrl,
                        alt,
                    });
                }
                WireEvent::Ready => {
                    if let Some(h) = on_event {
                        h.call(ViewerEvent::Ready);
                    }
                }
                WireEvent::Loaded { id } => {
                    if let Some(h) = on_event {
                        h.call(ViewerEvent::Loaded { id });
                    }
                }
                WireEvent::LoadError { id, message } => {
                    if let Some(h) = on_event {
                        h.call(ViewerEvent::LoadError { id, message });
                    }
                }
                WireEvent::Error { message } => {
                    if let Some(h) = on_event {
                        h.call(ViewerEvent::Error { message });
                    }
                }
            }
        }
    });

    // ── Rust → JS: diff ──────────────────────────────────────────────────────
    // Non-reactive mirror of what the renderer already knows.
    // CopyValue (not a Signal), because writing a Signal would re-trigger this Effect.
    let mut applied =
        use_hook(|| CopyValue::new(std::collections::BTreeMap::<String, GlbObject>::new()));

    use_effect(move || {
        // objects.read() subscribes this Effect. Keep the borrow outside any await.
        let (ops, dupes) = {
            let objs = objects.read();
            let mut dup = Vec::new();
            let ops = diff::diff(&mut applied.write(), &objs, &mut dup);
            (ops, dup)
        };

        // Report duplicate IDs as errors
        for id in dupes {
            if let Some(h) = on_event {
                h.call(ViewerEvent::Error {
                    message: format!("Duplicate object ID `{id}` in objects — extra entry ignored"),
                });
            }
        }

        // Send ops
        if !ops.is_empty() {
            match wire::messages(ops) {
                Ok(msgs) => {
                    for msg in msgs {
                        if let Err(e) = eval.send(msg) {
                            if let Some(h) = on_event {
                                h.call(ViewerEvent::Error {
                                    message: format!("eval.send failed: {e}"),
                                });
                            }
                            break;
                        }
                    }
                }
                Err(e) => {
                    if let Some(h) = on_event {
                        h.call(ViewerEvent::Error {
                            message: format!("Op serialisation failed: {e}"),
                        });
                    }
                }
            }
        }
    });

    // ── Options effect ───────────────────────────────────────────────────────
    // Separate Effect so a colour change doesn't run through the object list.
    use_effect(move || {
        let opts = options.read().clone();
        match serde_json::to_value(Op::SetOptions { options: opts }) {
            Ok(msg) => {
                let _ = eval.send(msg);
            }
            Err(e) => {
                if let Some(h) = on_event {
                    h.call(ViewerEvent::Error {
                        message: format!("SetOptions serialisation failed: {e}"),
                    });
                }
            }
        }
    });

    // ── Cleanup on unmount ───────────────────────────────────────────────────
    // Eval is Copy and not scope-owned → safe to use in use_drop.
    use_drop(move || {
        let _ = eval.send(Op::Destroy);
    });

    rsx! {
        document::Stylesheet { href: VIEWER_CSS }
        // STATIC SHAPE: no `if`, no `key`, no interpolation around this div/canvas.
        // Any reconstruction of the canvas destroys the WebGL context and the camera.
        div {
            class: "dxglb-root",
            ..attributes,
            canvas { id: "{canvas_id}", class: "dxglb-canvas" }
        }
    }
}

// ─── Boot script ─────────────────────────────────────────────────────────────

/// Normalises an asset path to a URL the `WebView` can fetch.
///
/// Under `dx serve`/`dx bundle` `Asset::to_string()` already returns an `/assets/...`
/// path (`is_bundled_app()` is set when `DIOXUS_CLI_ENABLED` is active). On a bare
/// `cargo run` the path may contain Windows backslashes — those are normalised here.
fn asset_url(asset: Asset) -> String {
    let s = asset.to_string().replace('\\', "/");
    if s.starts_with('/') || s.contains("://") {
        s
    } else {
        format!("/{s}")
    }
}

/// Builds the boot script executed in `document::eval`.
///
/// The script:
/// 1. Dynamically imports `viewer.js` (the module cache prevents re-evaluation).
/// 2. Creates a viewer instance for this canvas, passing `threeBase` so the module
///    can load three.js addons by absolute URL without relying on an importmap.
/// 3. Runs as a `for(;;)` loop, receiving ops.
///
/// **The script must NEVER `return`.** A `return` resolves the promise, upon which Dioxus
/// calls `dioxus.close()` — after that all further `eval.send()` calls fail.
fn build_boot_script(canvas_id: &str, opts: &ViewerOptions) -> String {
    let viewer_url = asset_url(VIEWER_JS);
    let three_base = asset_url(THREE_DIR);
    let canvas_id_j = serde_json::to_string(canvas_id).expect("canvas_id is ASCII");
    let opts_j = serde_json::to_string(opts).expect("ViewerOptions is serialisable");
    let viewer_url_j = serde_json::to_string(&viewer_url).expect("viewer_url is a string");
    let three_base_j = serde_json::to_string(&three_base).expect("three_base is a string");

    // All interpolations are serde_json::to_string() results → syntactically valid JS literals.
    // Nothing from canvas IDs, paths, or colour values can break out of the string.
    format!(
        r"
// 1. Load the viewer module (module registry caches it; multiple instances share the namespace).
//    No importmap needed: the three.js addons use relative imports within their directory,
//    and viewer.js receives threeBase as an explicit parameter.
let mod_ = null;
try {{
    mod_ = await import({viewer_url_j});
}} catch (e) {{
    dioxus.send({{ event: 'error', message: 'Could not load viewer module: ' + String(e) }});
}}

// 2. Create a viewer instance for this canvas.
let viewer_ = null;
if (mod_) {{
    try {{
        viewer_ = await mod_.createViewer({canvas_id_j}, {opts_j}, (m) => dioxus.send(m), {three_base_j});
        dioxus.send({{ event: 'ready' }});
    }} catch (e) {{
        dioxus.send({{ event: 'error', message: String((e && e.stack) || e) }});
    }}
}}

// 3. Op loop. MUST NEVER `return`: a `return` resolves the promise and Dioxus calls
//    dioxus.close() — after that every eval.send() fails with EvalError::Finished.
for (;;) {{
    const op_ = await dioxus.recv();
    if (op_.op === 'destroy') {{
        if (viewer_) viewer_.dispose();
        break;   // leaves the loop; the promise resolves; the channel is closed.
    }}
    if (!viewer_) continue;   // fatal init error: ignore ops.
    try {{
        viewer_.apply(op_);
    }} catch (e) {{
        dioxus.send({{ event: 'error', message: String((e && e.stack) || e) }});
    }}
}}
"
    )
}
