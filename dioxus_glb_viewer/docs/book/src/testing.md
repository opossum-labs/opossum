# Testing and checks

The crate splits cleanly into a part that can be tested without a browser and a part that
cannot. Knowing which is which saves time in both directions.

## What is covered automatically

```bash
cargo test -p dioxus_glb_viewer
```

25 unit tests and one doctest, no browser and no GPU involved. The pure logic — the diff,
the equality semantics, the byte chunking — is deliberately kept free of Dioxus and
three.js so that it can be tested exactly this way.

| Module | Tests | What they pin down |
|---|---|---|
| `model` | 4 | `Url` equality by content, `Bytes` equality by pointer, cross-variant inequality, identity default for `Transform`. |
| `diff` | 13 | Every rule: add, remove, no-op, each field update in isolation, reload on source change, reload on a new `Arc`, order-independence, removes before adds, duplicate handling, multiple simultaneous changes. |
| `wire` | 8 | Single-message pass-through, the begin/chunk/op sequence, chunk-count boundaries, that the payload never appears in the op JSON, and the two duplicate-id regressions. |

Two of those tests exist because of a bug that shipped once — see [why the payload travels
with the op](./op-protocol.md#why-the-payload-travels-with-the-op). If you change the
transport layer, they are the ones to watch.

## Lints and formatting

Run formatting before linting, so clippy sees formatted code:

```bash
cargo fmt -p dioxus_glb_viewer
cargo clippy -p dioxus_glb_viewer -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms
```

Those clippy flags are the ones CI uses; running plain `cargo clippy` locally will pass on
code that CI then rejects.

Scope with `-p`. The crate is a workspace member, so a workspace-wide invocation does
include it, but a scoped run is faster and keeps unrelated crates' warnings out of the
output.

## What is **not** covered automatically

Everything in `assets/viewer.js`. There is no headless browser in the test setup, which
means no automated coverage of:

- camera stability across scene changes — the component's core promise,
- picking, and the discrimination between a click and an orbit drag,
- resize handling,
- GPU resource disposal,
- the actual loading of a `.glb` file.

All of it is verified by running the demo. That is a real limitation, not an oversight to
be apologised for — but it does mean **a change to `viewer.js` or to the RSX in
`viewer.rs` is not verified until someone has looked at it.**

## The manual acceptance run

Start the demo and work through the list. Steps 3 to 6 are the camera-stability check and
matter most; they are what catches a broken
[canvas shape](./invariants.md#the-rsx-around-the-canvas-stays-static).

```bash
cd dioxus_glb_viewer
dx serve --example demo --features desktop --platform desktop
```

1. Start the app, with or without a sample model. It must come up either way.
2. **Add** — a model appears. Now orbit and zoom into a distinctive oblique view, and keep
   it for the next four steps.
3. **Add** again — a second model appears. *The view must not move.*
4. **Remove last** — one model disappears. *The view must not move.*
5. **Replace first** — the first model reloads with a new transform. *The view must not
   move.*
6. **Toggle visibility** — the selected model vanishes and comes back. *The view must not
   move.*
7. Click a model — the panel shows the right id and `mesh_name`, and a selection box
   appears. Click empty space — `id: None`, selection cleared. **Drag** to orbit — no pick
   event at all.
8. Resize the window — the image stays undistorted, with no stretching.
9. **Fit view** — the camera frames the visible models, keeping the angle you were looking
   from. **Reset camera** — back to the initial pose.
10. Alternate **Add** and **Remove last** about twenty times, watching memory in DevTools.
    It must stay flat.
11. Desktop only: type an absolute path to a `.glb` into the input field. It loads.

### What each step guards

| Steps | Guards |
|---|---|
| 3–6 | Camera stability, and therefore the static canvas shape and the camera-free diff. |
| 7 | The 4 px drag threshold, the raycast against visible roots only, and caller-owned selection. |
| 8 | The `ResizeObserver` — and note that resizing the *window* is the easy case; dragging a panel splitter is the case `window.resize` would miss. |
| 9 | The only two camera-moving ops, and that `fit_view` preserves the viewing direction. |
| 10 | Disposal. A leak here means geometries, materials or textures are not being released. |
| 11 | The whole byte path: chunking, Base64, reassembly, `parse`. |

## Iterating on the JavaScript

**Restart `dx serve` after every edit to `viewer.js`.** The browser caches ES modules by
URL, so a running session keeps the module it already imported and your change appears to
do nothing. This catches everyone once; see
[Invariants](./invariants.md#viewerjs-is-not-hot-reloadable).
