# Picking and events

Clicking a model reports *which* object was hit back to Rust. The component does not act
on that — it tells you, and you decide what a click means.

## `PickEvent`

Delivered to the required `on_pick` handler.

| Field | Type | Meaning |
|---|---|---|
| `id` | `Option<String>` | The `GlbObject::id` that was hit, or `None` for a miss. |
| `mesh_name` | `Option<String>` | Name of the concrete glTF mesh under the cursor, if the file names it. |
| `point` | `Option<[f32; 3]>` | World coordinates of the intersection, `None` for a miss. |
| `shift` | `bool` | Shift held. |
| `ctrl` | `bool` | Ctrl **or** Meta held — one flag for both, so ⌘-click and Ctrl-click behave alike. |
| `alt` | `bool` | Alt held. |

A miss still calls the handler, with `id: None`. That is deliberate: clicking empty space
is how a user clears a selection, and you need the event to do it.

`mesh_name` is the name of the individual mesh inside the model, not of the object — a
click on a lens assembly might report `"rim"` while `id` reports `"lens-1"`. It is useful
for showing the user what exactly is under the cursor, and for models whose parts are
named meaningfully.

## What counts as a click

Navigation and picking share the left mouse button, so the component has to tell them
apart. It does so on `pointerup`:

- **Left button only.** Other buttons are ignored outright.
- **The pointer must match.** The `pointerup` has to belong to the same pointer as the
  `pointerdown` that started it; `pointercancel` discards a pending press.
- **The pointer must not have travelled more than 4 px.** Beyond that it was an orbit
  drag, and **no event is emitted at all** — not even a miss.
- **There is no time limit.** This is intentional: a slow, deliberate click is still a
  click, and a time threshold would turn careful users' clicks into silence.

So dragging to orbit never disturbs your selection, and a click that happens to wobble by
a pixel or two still registers.

## Coordinates

Normalised device coordinates are computed from the canvas's **CSS box**
(`getBoundingClientRect`), not from `canvas.width`/`canvas.height`. On a display where
`devicePixelRatio != 1` the drawing buffer is larger than the CSS box, and using the
buffer size would put the ray in the wrong place — an error that grows towards the edges
of the canvas and is easy to mistake for a camera problem.

`point` is in world coordinates, i.e. after each object's `transform` has been applied.

## What can be hit

The raycast runs against the **roots of visible objects only**, recursing into their
children, and skips line and point primitives. That has four consequences:

- The reference grid and the ground are **never** a hit. Both are raycastable, so including
  the whole scene would yield hits that belong to no object at all.
- An object with `visible: false` cannot be picked. Hiding something also takes it out of
  reach of the cursor.
- Lines and points (glTF modes `LINES`, `LINE_STRIP`, `POINTS` — ray paths, say) are
  **never** a hit. They are annotations drawn over the models, and three.js would otherwise
  hit a line from up to one world unit away, so a drawn path would swallow nearly every
  click on the model behind it. A click on a line picks whatever surface lies behind it.
- Only the **nearest** intersection is reported. There is no way to pick through a model
  to something behind it.

## Selection belongs to you

The component draws a selection box when `selected` is `true`, but it never sets that
field. The round trip is: the user clicks → `on_pick` fires → **you** update your own
list → the diff sends the highlight. Single selection is four lines:

```rust
on_pick: move |pick: PickEvent| {
    let hit = pick.id.clone();
    for obj in objects.write().iter_mut() {
        obj.selected = Some(&obj.id) == hit.as_ref();
    }
},
```

A miss clears everything, because no `id` matches `None`. Multi-select is the same shape
with `pick.shift` deciding whether to add to the set or replace it — and because the
state is yours, so is the policy.

## `ViewerEvent`

Everything the renderer reports besides picks, delivered to the optional `on_event`
handler.

| Variant | Meaning |
|---|---|
| `Ready` | three.js is initialised, the canvas has a WebGL context, the op pump is running. |
| `Loaded { id }` | That model is loaded and in the scene. |
| `LoadError { id, message }` | That model could not be loaded; it is not in the scene. |
| `Error { message }` | Something non-recoverable: no WebGL, the viewer module failed to import, an op could not be serialised or sent, or a duplicate `id` was ignored. |

Two practical notes:

- **You do not have to wait for `Ready`.** The eval channel exists before the first diff
  runs, so objects you set immediately are queued and applied as soon as the renderer
  comes up. `Ready` is for telling the *user* that the viewer is live, not for
  sequencing your own calls.
- **`ViewerEvent` is `#[non_exhaustive]`.** A `match` on it needs a wildcard arm, so that
  a future variant does not break your build.

Next: [Camera control](./camera.md).
