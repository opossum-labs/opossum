# Reference

A compact index of the public API. The authoritative, always-current version is the API
doc:

```bash
cargo doc -p dioxus_glb_viewer --no-deps --open
```

## Types

| Type | Summary |
|---|---|
| `GlbViewer` | The component. Renders `.glb` models in a canvas. |
| `GlbObject` | One model: `id`, `source`, `transform`, `visible`, `selected`. |
| `GlbSource` | `Url(String)` or `Bytes(Arc<[u8]>)`. `Url` compares by content, `Bytes` by pointer. |
| `Transform` | `position`, `rotation_euler_xyz` (radians, Euler XYZ), `scale`. `Default` is the identity. |
| `ViewerOptions` | Nine scene-wide settings; `Default` gives a usable dark scene with a grid. |
| `PickEvent` | `id`, `mesh_name`, `point`, `shift`, `ctrl`, `alt`. |
| `ViewerEvent` | `Ready`, `Loaded`, `LoadError`, `Error`. `#[non_exhaustive]`. |
| `ViewerHandle` | `Copy` handle for the camera operations. |

## `GlbViewer` props

| Prop | Type | Required |
|---|---|---|
| `objects` | `ReadSignal<Vec<GlbObject>>` | **yes** |
| `on_pick` | `EventHandler<PickEvent>` | **yes** |
| `on_event` | `Option<EventHandler<ViewerEvent>>` | no |
| `options` | `ReadSignal<ViewerOptions>` | no |
| `handle` | `Option<ViewerHandle>` | no |
| *global attributes* | `Vec<Attribute>` | no — forwarded to the outer `div` |

## `ViewerHandle`

```rust
fn use_glb_viewer_handle() -> ViewerHandle;

impl ViewerHandle {
    fn fit_view(self);      // frames visible models — MOVES THE CAMERA
    fn reset_camera(self);  // back to the initial pose — MOVES THE CAMERA
    fn clear_scene(self);   // removes every model; see the trap below
}
```

The handle must be passed to the component as the `handle` prop, or its methods do
nothing.

## `ViewerOptions` defaults

| Field | Default |
|---|---|
| `background` | `"#1e1e1e"` |
| `grid` | `true` |
| `ambient_intensity` | `0.8` |
| `directional_intensity` | `1.2` |
| `selection_color` | `"#00aaff"` |
| `fit_on_first_load` | `true` |
| `initial_camera` | `[5.0, 3.0, 5.0]` |
| `initial_target` | `[0.0, 0.0, 0.0]` |
| `environment` | `None` |
| `fov_degrees` | `50.0` |

Only some of these apply live; see [when option changes take
effect](./scene.md#when-option-changes-take-effect).

## Ops

| Tag | Emitted by |
|---|---|
| `add`, `reload`, `remove`, `set_transform`, `set_visible`, `set_selected` | the diff |
| `set_options` | the options effect |
| `fit_view`, `reset_camera`, `clear` | `ViewerHandle` |
| `destroy` | unmount |
| `bytes_begin`, `bytes_chunk` | the transport layer |

Details in [The op protocol](./op-protocol.md).

## Events

| Tag | Delivered as |
|---|---|
| `pick` | `PickEvent`, to `on_pick` |
| `ready` | `ViewerEvent::Ready`, to `on_event` |
| `loaded` | `ViewerEvent::Loaded` |
| `load_error` | `ViewerEvent::LoadError` |
| `error` | `ViewerEvent::Error` |

## Feature flags

| Feature | Effect |
|---|---|
| *(default)* | Nothing. What library consumers use. |
| `desktop` | `dioxus/desktop` + `dioxus/launch` — for the example only. |
| `web` | `dioxus/web` + `dioxus/launch` — for the example only. |

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Blank area, no errors anywhere | The parent has automatic height, so `height: 100%` resolves to zero. Give it a definite height. |
| `Error`: WebGL not available | A VM, an RDP session, or an outdated graphics driver. |
| `Error`: canvas not found after 10 s | The canvas never mounted — usually the RSX shape was changed so the node became conditional. |
| `Loaded` fired but nothing is visible | Scale. The clip range is 0.01 to 10 000 units, and the default camera sits at `[5, 3, 5]`. |
| `LoadError` naming an extension | Draco, KTX2 or Meshopt — not supported. |
| The view jumps when the scene changes | A broken [canvas shape](./invariants.md#the-rsx-around-the-canvas-stays-static), or a camera-moving op reachable from the diff. |
| The initial scene renders, then updates are ignored | The [boot script returned](./invariants.md#the-boot-script-never-returns) and the channel closed. |
| Edits to `viewer.js` seem to do nothing | The module is cached by URL. Restart `dx serve`. |
| A desktop file URL never loads | wry cannot fetch `file://`. Use `GlbSource::Bytes`. |
| Models vanish after `clear_scene()` and do not come back | The [applied-state desync](./camera.md#the-clear_scene-trap). Clear your `objects` list too. |
| Memory grows with every model swap | A disposal regression in `viewer.js`. |
| A model reloads on every parent render | A new `Arc` each time. Hold on to the one you have and clone it. |

## Glossary

- **Op** — one message from Rust to the renderer, as a single tagged JSON object.
- **Diff key** — the object's `id`; how the diff recognises it across renders.
- **Transport key** — the key tying a byte stream to the op that consumes it. In practice
  the object's id.
- **Generation** — a per-id counter that lets the renderer recognise and discard a load
  that a newer one has overtaken.
- **Applied state** — the renderer's contents as the Rust side believes them to be. The
  diff's input and the thing `clear_scene()` bypasses.
- **Folder asset** — an asset directory served unhashed and structure-preserving, so
  relative imports inside it keep resolving.
- **Boot script** — the JavaScript evaluated once per viewer instance: it imports the
  module, creates the viewer, and runs the op loop for the lifetime of the component.
