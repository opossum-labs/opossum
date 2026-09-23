# Getting started

## Add the crate

`dioxus_glb_viewer` is `publish = false`, so depend on it by path:

```toml
[dependencies]
dioxus_glb_viewer = { path = "../dioxus_glb_viewer" }
# Your app brings its own dioxus, with the renderer feature it wants:
dioxus = { version = "0.7", features = ["desktop"] }   # or "web"
```

> The `dioxus` version **must** match the one the component uses (0.7), because props,
> signals and event handlers cross the crate boundary. A mismatch shows up as a type
> error on `GlbViewer`'s props, not as a runtime problem.

The component itself depends on `dioxus` with `default-features = false` and only the
`lib` feature — it uses `dioxus::prelude`, `document::eval` and `asset!`, nothing that
would tie it to a platform. **The host decides between desktop and web.**

### Feature flags

| Feature | Effect |
|---|---|
| *(default)* | Nothing. This is what library consumers use. |
| `desktop` | Adds `dioxus/desktop` + `dioxus/launch`. |
| `web` | Adds `dioxus/web` + `dioxus/launch`. |

`desktop` and `web` exist for exactly one reason: so that `examples/demo.rs` can call
`dioxus::launch`. **Do not enable either one as a library consumer** — that would force a
renderer choice onto your app from a dependency.

## Your first viewer

Two props are required: `objects`, the scene, and `on_pick`, the click handler.
Everything else has a default.

```rust
use dioxus::prelude::*;
use dioxus_glb_viewer::{GlbObject, GlbSource, GlbViewer, PickEvent, Transform};

// A model bundled with your app. `asset!` gives it a URL the WebView can fetch.
const MODEL: Asset = asset!("/assets/lens.glb");

#[component]
fn Scene3d() -> Element {
    // The scene is a signal holding a list of objects. Change the list, and the
    // viewer follows — there is no imperative "add model" call.
    let objects = use_signal(|| {
        vec![GlbObject {
            id: "lens-1".into(),               // your id; picks report it back
            source: GlbSource::Url(MODEL.to_string()),
            transform: Transform::default(),   // origin, unrotated, unscaled
            visible: true,
            selected: false,
        }]
    });

    rsx! {
        // The viewer fills its parent, so the parent needs a size.
        div { style: "width: 100%; height: 600px;",
            GlbViewer {
                objects,
                on_pick: move |pick: PickEvent| {
                    println!("clicked: {:?}", pick.id);
                },
            }
        }
    }
}
```

With the defaults you get a dark background, a reference grid, orbit/dolly/pan
navigation, and the scene framed automatically on the first successful load. From then
on the camera stays exactly where the user put it — see
[Camera control](./camera.md).

## Run the bundled demo

The crate ships a desktop demo that exercises every feature: adding, removing,
replacing, visibility, picking, fit/reset, clearing, and loading a file from disk as
raw bytes.

```bash
cd dioxus_glb_viewer
dx serve --example demo --features desktop --platform desktop
```

This needs the Dioxus CLI (`cargo binstall dioxus-cli`).

### The sample model

Drop any `.glb` at `examples/assets/sample.glb` to enable the Add and Replace buttons.
[Khronos glTF Sample Assets](https://github.com/KhronosGroup/glTF-Sample-Assets) is a
good source — `Box.glb` for a first check, `DamagedHelmet.glb` for something textured.

The demo builds and runs without it: `option_asset!` returns `None` instead of failing
the build, and the app shows a notice with those buttons disabled. The bytes path (the
path input field in the side panel) works either way.

## Run the tests

The Rust side — diff rules, equality semantics, byte chunking — is covered by unit tests
that need no browser:

```bash
cargo test -p dioxus_glb_viewer
```

See [Testing and checks](./testing.md) for the lint commands and the manual acceptance
run.

Next: [The declarative scene](./scene.md).
