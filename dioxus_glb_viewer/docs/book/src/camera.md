# Camera control

The camera is the one piece of state the declarative scene deliberately does **not**
own. A user who has spent ten seconds orbiting into a useful view should not lose it
because a model was added, and that guarantee only holds if nothing in the normal update
path is allowed to touch the camera.

Nothing in it is. No op the diff can produce moves the camera — `FitView` and
`ResetCamera` are intentionally not constructible there. The only way to move it is to
ask, explicitly, through a handle.

## Navigation

`OrbitControls` provides orbit, dolly and pan, with damping enabled — which is why the
render loop runs continuously rather than only on demand.

The camera is a `PerspectiveCamera` with a **near plane of 0.01 and a far plane of
10 000**, in whatever units your model uses. Geometry closer than 0.01 or further than
10 000 units from the camera is clipped. If a model is invisible after loading and
`Loaded` did fire, scale is the first thing to check.

## The handle

```rust
use dioxus_glb_viewer::{GlbViewer, use_glb_viewer_handle};

let handle = use_glb_viewer_handle();

rsx! {
    button { onclick: move |_| handle.fit_view(), "Fit view" }
    GlbViewer { objects, handle, on_pick: move |_| {} }
}
```

`ViewerHandle` is `Copy`, so store it, clone it into as many closures as you like, and
pass it to the component as the `handle` prop. Without that prop the handle exists but is
not connected to anything, and its methods do nothing.

| Method | Effect |
|---|---|
| `fit_view()` | Frames all currently visible models. **Moves the camera.** |
| `reset_camera()` | Returns to `initial_camera` / `initial_target`. **Moves the camera.** |
| `clear_scene()` | Removes every model and frees its GPU resources. Does not move the camera. |

## How `fit_view` frames the scene

It takes the bounding box of all **visible** objects, puts the orbit target at its
centre, and backs the camera off far enough that the largest dimension fits the vertical
field of view, with a 1.5× margin.

The one part worth knowing: **the viewing direction is preserved.** `fit_view` moves
along the current camera-to-target axis, so it does not snap to a canned three-quarter
view — the user keeps the angle they chose and only the framing changes.

If nothing visible is in the scene, the bounding box is empty and the call does nothing
at all. It is not an error.

## Automatic framing

`ViewerOptions::fit_on_first_load` (default `true`) runs exactly one `fit_view` for you,
on the first successful load. It is latched: once an auto-fit has happened, it never
happens again, no matter how many models are added afterwards. That is the whole of the
automatic camera behaviour.

Set it to `false` when you want to control the initial view yourself through
`initial_camera` and `initial_target`.

## The `clear_scene` trap

`clear_scene()` goes straight to the renderer and bypasses the diff. The Rust side's
record of what the renderer is showing is therefore **not** updated, and it will believe
those models are still there.

The consequence is specific and easy to misread as a rendering bug:

- The next diff compares your unchanged list against its unchanged record, finds nothing
  different, and sends nothing. The models stay gone.
- Changing a `transform`, `visible` or `selected` field afterwards sends an update for an
  object the renderer no longer has, which it silently ignores. The model does **not**
  come back.
- Only a change that produces a full load brings it back: removing the `id` and adding it
  again, or changing its `source`.

So the normal way to empty the scene is to empty your list:

```rust
objects.write().clear();
```

Use `clear_scene()` only as the escape hatch it is — when one message is genuinely worth
more than N removals — and then clear your list in the same breath, as the demo does:

```rust
handle.clear_scene();
objects.write().clear();
```

Next: [Embedding in a Dioxus app](./embedding.md).
