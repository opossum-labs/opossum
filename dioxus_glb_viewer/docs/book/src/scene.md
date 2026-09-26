# The declarative scene

You never tell the viewer to load or unload anything. You describe the scene you want,
and the component works out the difference from the scene the renderer currently shows.

```rust
GlbViewer {
    objects,                    // ReadSignal<Vec<GlbObject>>  — required
    on_pick: move |p| { … },    // EventHandler<PickEvent>     — required
    on_event: move |e| { … },   // optional: load progress and errors
    options,                    // optional: scene-wide settings
    handle: viewer_handle,      // optional: the camera handle
    style: "flex: 1;",          // any global attribute, forwarded to the outer div
}
```

## `GlbObject`

One model in the scene.

| Field | Type | Meaning |
|---|---|---|
| `id` | `String` | Your identifier. Unique within the list, and the **diff key**. |
| `source` | `GlbSource` | Where the `.glb` bytes come from. |
| `transform` | `Transform` | Placement in world coordinates. |
| `visible` | `bool` | Hidden objects stay in the scene graph. |
| `selected` | `bool` | Draws a `THREE.BoxHelper` around the object. |

The `id` is what [`PickEvent`](./picking.md) reports back, and it is how the diff
recognises an object across renders: a stable `id` whose `transform` changed produces a
small in-place update, while a *changed* `id` means the old object goes away and a new
one is loaded.

`selected` only draws the highlight. The component never sets it — selection state
belongs to you, which is why the pick handler exists. [Picking and
events](./picking.md) shows the pattern.

## `GlbSource` and what counts as a change

```rust
pub enum GlbSource {
    Url(String),        // the WebView fetches it
    Bytes(Arc<[u8]>),   // raw .glb bytes, streamed to the WebView
}
```

The equality rules matter, because they decide when a model reloads:

- **`Url` compares by content.** Two equal strings are the same source.
- **`Bytes` compares by pointer identity** (`Arc::ptr_eq`), *not* by content. Cloning an
  `Arc` is free and changes nothing; a **new** `Arc`, even built from byte-for-byte
  identical data, counts as a new model and triggers a reload.

That asymmetry is deliberate. Dioxus's props macro compares the old and new props on
every parent render. Comparing byte content would `memcmp` the whole model — up to
tens of megabytes, on every render of the enclosing component. Pointer identity is both
cheap and the right semantic: a fresh `Arc` means "I have a different model now". The
worst case is one unnecessary reload; it can never show a wrong image.

The practical rule: **hold on to your `Arc`** and clone it into each new `GlbObject`,
instead of re-reading the file.

[Loading models](./loading-models.md) covers both variants in detail.

## `Transform`

```rust
pub struct Transform {
    pub position: [f32; 3],
    pub rotation_euler_xyz: [f32; 3],   // radians
    pub scale: [f32; 3],
}
```

Rotation is **intrinsic Euler XYZ in radians**, which is the default order of
`THREE.Euler`, so the three components map straight onto `root.rotation.set(...)`.
Unlike a rigid isometry, a `Transform` may scale — non-uniformly, if you want.

`Transform::default()` is the identity: at the origin, unrotated, scale `[1, 1, 1]`.

## `ViewerOptions`

Scene-wide settings. The prop is optional; leaving it out uses these defaults.

| Field | Meaning | Default |
|---|---|---|
| `background` | Canvas background colour. | `"#1e1e1e"` |
| `grid` | Show the reference grid (a `GridHelper` of size 20 with 20 divisions). | `true` |
| `ambient_intensity` | Ambient light intensity (0.0–2.0). | `0.8` |
| `directional_intensity` | Main directional light intensity. | `1.2` |
| `selection_color` | Colour of the selection `BoxHelper`. | `"#00aaff"` |
| `fit_on_first_load` | Frame the scene on the first successful load. | `true` |
| `initial_camera` | Camera position before the first auto-fit. | `[5.0, 3.0, 5.0]` |
| `initial_target` | Camera target before the first auto-fit. | `[0.0, 0.0, 0.0]` |
| `environment` | Surroundings the scene is lit and reflected by (`None` or `Room`). | `None` |
| `fov_degrees` | Vertical field of view. | `50.0` |
| `orientation_gizmo` | Show a corner orientation gizmo (three.js `ViewHelper`) in the bottom-right corner. Coloured axes reflect the current camera orientation; clicking an axis snaps the camera to that view. | `false` |
| `ground` | A floor reaching to the horizon: an image tiled at a given height (`Ground { height, tile_url, tile_size }`). Never framed by `fit_view`, never hit by a click. See [the ground](./renderer.md#the-ground). | `None` |
| `line_width` | Width of line primitives (glTF `LINES`) in screen pixels. WebGL draws lines one pixel wide whatever is asked for, so above `1.0` lines are redrawn as three.js fat lines (`LineSegments2`), which keep their pixel width at any zoom, cast no shadow and are never hit by a click. | `1.0` |

Colour values are handed to `new THREE.Color(...)`, so they must be something three.js
can parse: `"#1e1e1e"`, `"rgb(30,30,30)"`, or a CSS colour name. A *transparent*
background is not achievable this way — the WebGL context is created without an alpha
buffer.

### When option changes take effect

Option changes are sent live as a single `set_options` message, and **none of them move
the camera**. They do not all apply at the same moment, though:

| Field | Applied |
|---|---|
| `background`, `grid`, `ambient_intensity`, `directional_intensity`, `selection_color` | Immediately, on every change. |
| `environment` | Immediately, but asynchronously: the map is imported and rendered on first use. |
| `orientation_gizmo` | Immediately, but asynchronously: the gizmo is imported on first use. |
| `line_width` | Immediately, for the models on screen and every one loaded later. Widening past one pixel imports the fat-line add-on on first use, so that step is asynchronous. |
| `ground` | Immediately. A new `height` moves the floor; a new `tile_url` or `tile_size` fetches the image again, which then appears once it has loaded. |
| `initial_camera`, `initial_target` | On the next `reset_camera()` call. |
| `fit_on_first_load` | Checked on every successful load, until an auto-fit has actually happened once. |
| `fov_degrees` | **Read once, when the viewer boots.** Later changes are ignored. |

## Which changes produce which updates

This is the whole contract between your list and the renderer:

| Change in your `Vec<GlbObject>` | What happens |
|---|---|
| A new `id` appears | The model is loaded and added. |
| An `id` disappears | The model is removed and its GPU resources are freed. |
| `source` differs (per the rules above) | The old model is disposed and the new one loaded, with transform, visibility and selection applied in the same step. |
| `transform` differs | Position, rotation and scale are updated in place. No reload. |
| `visible` differs | The object is hidden or shown; it stays in the scene graph. |
| `selected` differs | The selection `BoxHelper` is added or removed. |
| Only the **order** of entries differs | Nothing at all. Order carries no meaning. |
| Nothing differs | Nothing is sent. |

Several fields changing on one object produce several updates, one per field. And in
every row of that table, **the camera does not move** — that guarantee is the reason the
diff exists rather than a reload-everything approach. See [The op
protocol](./op-protocol.md) for the messages this turns into.

## Duplicate ids

An `id` must be unique within the list. If it is not, the **first** entry wins, the
later ones are ignored, and each duplicate is reported through `on_event` as
`ViewerEvent::Error`. Nothing breaks, but you are not seeing what you described — so
treat it as the bug it is.

Next: [Loading models](./loading-models.md).
