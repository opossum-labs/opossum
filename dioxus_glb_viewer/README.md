# dioxus_glb_viewer

A self-contained, opossum-independent Dioxus component that renders `.glb` files
(glTF-Binary) with three.js inside a `<canvas>`.

## Features

- **OrbitControls** for free navigation (orbit, dolly, pan).
- **Declarative scene**: Pass scene contents as `Vec<GlbObject>`; adding, removing, and
  replacing models does **not** move the camera.
- **Click-picking**: A left-click (not an orbit drag) reports the caller-assigned ID of
  the hit object back to Rust — ideal for configuration menus and live object
  redefinition.
- **`GlbSource::Url`** for HTTP/asset sources; **`GlbSource::Bytes`** for arbitrary
  files from the filesystem (on the desktop wry cannot load `file://` URLs).

## Quick start

```rust,no_run
use dioxus::prelude::*;
use dioxus_glb_viewer::{GlbObject, GlbSource, GlbViewer, PickEvent, Transform};

#[component]
fn App() -> Element {
    let objects = use_signal(|| {
        vec![GlbObject {
            id: "my-model".into(),
            source: GlbSource::Url("https://example.com/model.glb".into()),
            transform: Transform::default(),
            visible: true,
            selected: false,
        }]
    });
    rsx! {
        GlbViewer {
            objects,
            on_pick: move |p: PickEvent| { println!("pick: {:?}", p.id); },
        }
    }
}
```

## Running the demo

```bash
cd dioxus_glb_viewer
dx serve --example demo --features desktop --platform desktop
```

Place a `.glb` file at `examples/assets/sample.glb` before starting (see
`examples/assets/README.md` for sources). The demo works without a sample model but
shows a notice and disables the Add button.

## Documentation

- **Handbook** — the step-by-step guide in [`docs/book`](docs/book). Build it with
  `mdbook serve docs/book`, or read the markdown under [`docs/book/src`](docs/book/src).
  It covers using the component, the Rust ↔ JS op protocol, the three.js renderer, and
  the invariants to respect when changing any of it.
- **API docs** — `cargo doc -p dioxus_glb_viewer --no-deps --open`.
- **Manual acceptance run** — the canonical step-by-step list lives in the handbook, under
  [Testing and checks](docs/book/src/testing.md). Work through it after any change to
  `viewer.js` or to the RSX in `viewer.rs`: neither has automated coverage.

## Architecture

```
src/
  lib.rs    — crate root, re-exports public API
  model.rs  — GlbObject, GlbSource, Transform, ViewerOptions
  event.rs  — PickEvent, ViewerEvent
  op.rs     — Op / WireSource / WireEvent (Rust ↔ JS protocol)
  diff.rs   — pure diff(applied, next) → Vec<Op>
  wire.rs   — transport layer: Op list → JSON messages (chunked bytes)
  viewer.rs — GlbViewer Dioxus component + ViewerHandle
assets/
  viewer.js — three.js ES module (~400 lines)
  viewer.css
  three/    — vendored three.js r174 (folder asset, unhashed)
  VENDORING.md
```

## three.js vendoring

three.js r174 (`three@0.174.0`) is vendored under `assets/three/` as an unhashed folder
asset. See `assets/VENDORING.md` for the file list, upgrade recipe, and licence.

## Caveats

- **Hot-reload of `viewer.js` does not work** — the browser caches the module by URL.
  Restart `dx serve` to pick up JS changes.
- **Static RSX shape** — the `div.dxglb-root > canvas` subtree must never contain an
  `if`, `key`, or interpolated node. Any reconstruction of the canvas destroys the
  WebGL context and the camera.
- **Desktop file loading** — wry cannot load `file://` URLs. To display a local file,
  read it in Rust with `std::fs::read` and pass `GlbSource::Bytes(Arc::from(...))`.
- **Draco / KTX2 / Meshopt** compressed meshes are not supported in v1 (decoders would
  need to be vendored separately). The `GLTFLoader` reports them cleanly as
  `ViewerEvent::LoadError`.
