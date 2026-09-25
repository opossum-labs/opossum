# The JavaScript renderer

`assets/viewer.js` is the other half of the component: about 400 lines of ES module that
owns the three.js scene and applies the ops. Nothing else in the crate touches three.js.

## One export

```js
export async function createViewer(canvasId, options, send, threeBase)
    -> { apply, dispose }
```

That is the whole interface. `apply(op)` takes one decoded op, `dispose()` releases
everything, and `send(message)` is the callback the module uses to report events back to
Rust. A viewer instance is a closure over its own state — several instances coexist
without sharing anything but the module itself.

## Why `threeBase` is a parameter

three.js is loaded by explicit URL:

```js
const THREE = await import(threeBase + '/three.module.js');
const { OrbitControls } = await import(threeBase + '/examples/jsm/controls/OrbitControls.js');
const { GLTFLoader }    = await import(threeBase + '/examples/jsm/loaders/GLTFLoader.js');
```

**There is no importmap, and none is needed.** The vendored addons import their own
dependencies by relative path, so once the base directory is known, every transitive
import resolves on its own. Passing the base URL as an argument means the module never has
to guess where the assets ended up, and the boot script can hand it the hashed or unhashed
path that the asset pipeline actually produced — see [Vendored
three.js](./vendoring.md).

## Starting up

In order:

1. Import three.js and the two addons.
2. Wait for the canvas element to exist.
3. Refuse to continue without WebGL: if neither `webgl2` nor `webgl` yields a context, the
   module throws, and Rust turns that into `ViewerEvent::Error`. The message names the
   usual culprits — a VM, an RDP session, or an outdated graphics driver.
4. Build the renderer (`antialias: true`, device pixel ratio capped at 2 so a HiDPI screen
   does not quadruple the fragment cost), the scene and its background colour, a
   `PerspectiveCamera` at `initial_camera`, `OrbitControls` targeting `initial_target` with
   damping on, an ambient and a directional light, the grid if enabled, a `GLTFLoader` and
   a `Raycaster`.

### Waiting for the canvas

The module polls for the element every 16 ms, giving up after 10 seconds.

Polling with `setTimeout` rather than `requestAnimationFrame` is deliberate:
**`requestAnimationFrame` does not fire in a hidden or minimised window.** A viewer
mounted inside a collapsed panel, or in a window the user minimised before it finished
booting, would wait forever for a frame that never comes. `setTimeout` keeps running, so
the viewer is ready when the panel opens.

## What the instance keeps

Three maps and three flags, all private to the closure:

| State | Holds |
|---|---|
| `objects` | `id` to `{ root, helper }` — the loaded model group and its selection box. |
| `generations` | `id` to a counter, bumped on every load, to detect overtaken loads. |
| `pendingBytes` | transport key to the array of Base64 chunks received so far. |
| `hasAutoFitted` | Whether the one-time automatic framing has happened. |
| `needsResize` | Set by the `ResizeObserver`, cleared by the next frame. |
| `disposed` | Guards against doing anything after teardown. |

Plus an `AbortController`, which is how every DOM listener is registered — one
`ac.abort()` at teardown removes all of them, with no per-listener bookkeeping.

## The render loop

The loop runs continuously, because `OrbitControls` damping needs a frame after the user
lets go. Each frame it:

1. Returns immediately if disposed.
2. **Checks whether the canvas is still in the document**, and disposes itself if not.
   This is the safety net for an unmount where the `Destroy` op never arrives.
3. Resizes if the observer flagged it.
4. Updates the controls and renders.

Resizing reads the canvas's CSS box, clamps it to at least 1×1, and resizes the renderer
**without letting three.js write inline width/height styles** — those would change the
element's size, which the `ResizeObserver` would report, which would resize again.

## Applying ops

`apply(op)` is a `switch` on the op tag. Unknown tags are logged as a warning and
otherwise ignored, so a newer Rust side talking to an older module degrades rather than
breaks.

One thing runs through the whole dispatcher: **an op naming an id the renderer does not
have is silently ignored.** `set_transform`, `set_visible` and `set_selected` all look up
the object first and return if it is missing. That is what makes the
[`clear_scene` trap](./camera.md#the-clear_scene-trap) behave the way it does.

`set_transform` also updates the selection box, so a highlighted object's outline follows
it instead of being left behind.

### Loading a model

Both `add` and `reload` go through the same function, which:

1. Increments this id's generation and remembers it.
2. Removes whatever is currently under that id, disposing it.
3. Starts the load — `loader.load(url, ...)` for a URL, or for bytes, joins the pending
   chunks, decodes the Base64, and calls `loader.parse(buffer, '', ...)`.
4. On success, **checks the generation first.** If a newer load has started meanwhile, the
   result is thrown away and its scene graph disposed rather than added. Otherwise the
   group is tagged with the object id, transformed, made visible or not, added to the
   scene, highlighted if selected, and `loaded` is reported.
5. On failure, the generation is checked again before reporting `load_error`, so a
   superseded load cannot report an error about a model nobody is waiting for.

Base64 decoding uses `Uint8Array.fromBase64` where the engine has it, and falls back to an
`atob` loop otherwise.

### Selection highlights

A selection is a `THREE.BoxHelper` around the model's group, created on demand in the
current `selection_color` and destroyed on deselect — geometry and material both disposed.
Changing `selection_color` recolours the existing helpers in place rather than rebuilding
them.

### Options

`set_options` replaces the stored options and applies the ones that can change at runtime:
background colour, both light intensities, the grid (added or removed, and disposed when
removed), the orientation gizmo, the ground, and the colour of existing selection boxes. It does **not** touch the camera —
which is why `fov_degrees` changes have no effect after boot, as
[the options table](./scene.md#when-option-changes-take-effect) records.

### The environment map

`environment` is the one option that cannot be applied synchronously, and the only one that
exists for a reason of physics rather than taste.

glTF exports a refractive material as `KHR_materials_transmission`, which three.js renders by
showing what lies *behind* the surface. With nothing around the scene there is nothing behind it,
so the material renders **black** — a lens looks like a hole rather than like glass. Metals have
the same problem more mildly: with nothing to reflect they read as flat grey.

`Environment::Room` fixes that by rendering three.js's `RoomEnvironment` — a small grey room with
a few bright panels — through a `PMREMGenerator` into a prefiltered cube texture, and assigning it
to `scene.environment`. The room is neutral on purpose: it gives glass something to refract without
tinting the model.

Both the module import and the render-to-cubemap happen on **first use**, not at boot, so a viewer
that never asks for an environment pays for neither. That also means the map appears a frame or two
after the option changes.

Two things the implementation has to get right, both of them consequences of that `await`:

- The viewer may have been disposed while the import was in flight, and a second `set_options` may
  have overtaken this one. The generated texture is therefore discarded unless it is still the one
  being asked for.
- `PMREMGenerator` and the room scene are scaffolding: both are disposed immediately, and only the
  cube texture survives the call. The texture itself is disposed when the environment changes again
  and when the viewer shuts down.

### The ground

`ground` draws a floor that reaches to the horizon. The viewer knows nothing about what it shows:
the host supplies an image, the size of one copy of it in the world, and the height, and the viewer
tiles it.

A truly infinite plane cannot be drawn, so the floor is one plane that keeps up with the camera.
Every frame it is resized with the zoom — twenty times the camera's distance to its target on
either side, and never narrower than 64 tiles — and moved under the camera target. It only ever
moves in whole tiles and is always an even number of tiles wide, so the image's origin stays on a
tile corner: the plane moves, but the pattern stays fixed in the world. Towards its rim the floor
fades into the background through a radial alpha map, so its edge is never seen.

- The ground is **not** an entry of the object list. `fit_view` frames only the objects and picking
  only tests them, so the floor is never framed and never hit.
- `set_options` arrives on every option change. A new `height` only moves the plane; only a new
  `tile_url` or `tile_size` fetches the image again.
- A tile size that is not a positive number, or an image that fails to load, is reported as an
  `error` event rather than drawn.
- The floor is transparent (for the fade), and three.js renders only opaque objects into the buffer
  that transmissive glass refracts, so the floor is not seen through a lens.

## Releasing resources

WebGL resources are not garbage collected. Every geometry, material and texture has to be
disposed explicitly, and getting this wrong shows up as memory that grows with every model
swap until the context is lost. It is the single most repetitive concern in the module, and
worth understanding before changing anything there.

### Disposing a model

Disposal walks the whole scene graph of the model and, for every object in it, disposes the
geometry and each material — handling both a single material and an array of them. For each
material it then walks the material's own properties and disposes **everything that is a
texture**, whatever the property is called.

That last part is why it is a property scan rather than a list of known slots: a glTF
material may carry `map`, `normalMap`, `roughnessMap`, `aoMap`, `emissiveMap` and more, in
combinations that depend on the file. Checking every property for `isTexture` cannot miss
one, whereas an explicit list silently would.

Removing an object disposes its selection box as well, then detaches the group from the
scene and forgets it.

### Disposing the viewer

`dispose()` is idempotent and runs in an order that matters:

1. Mark disposed, so an in-flight frame stops.
2. Stop the render loop.
3. Disconnect the `ResizeObserver` and abort the `AbortController`, removing every DOM
   listener at once.
4. Dispose the controls.
5. Remove and dispose every model, then the grid, the gizmo and the ground.
6. Clear the scene and dispose the renderer.
7. **Force the WebGL context loss.** Browsers allow only a limited number of live contexts
   — roughly sixteen — and waiting for the garbage collector to release one is not good
   enough in an app that creates and destroys viewers as panels open and close.

Next: [Invariants](./invariants.md).
