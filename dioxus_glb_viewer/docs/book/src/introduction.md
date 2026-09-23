# Introduction

`dioxus_glb_viewer` is a [Dioxus](https://dioxuslabs.com/) component that renders
`.glb` files (glTF-Binary) with [three.js](https://threejs.org/) inside a `<canvas>` —
with orbit navigation, click-picking, and a scene you describe declaratively from Rust.

## What it is, and what it is not

It is a **viewer** component, not a geometry or asset crate.

- It **does** take finished `.glb` bytes — from a URL the `WebView` can fetch or from a
  `Arc<[u8]>` you read in Rust — load them with three.js `GLTFLoader`, place them
  according to a transform you give, report clicks back to Rust with your own object
  ids, and release every GPU resource again when a model goes away.
- It **does not** produce, tessellate or validate geometry. It does not own selection
  state: a click is *reported*, and you decide what it means. And it knows nothing about
  OPOSSUM — the crate has no dependency on any `opossum_*` crate, carries its own
  version (`0.1.0`, not the workspace version) and is `publish = false`.

That split is what makes the crate reusable: the same component renders a lens exported
by `optoscene` and a sample model downloaded from the Khronos asset repository, because
to the viewer both are just bytes.

## Conventions

- **The scene is declarative.** You hand the component a `Vec<GlbObject>` and it works
  out what changed. There is no `add_model()` to call. Internally the component diffs
  your list against what the renderer already has and sends only the difference — see
  [The op protocol](./op-protocol.md).
- **`id` is the diff key.** A stable `id` with a changed field produces a minimal
  update; a changed `id` is a different object.
- **Rotations are intrinsic Euler XYZ in radians**, matching the default order of
  `THREE.Euler`. Positions and scales are plain `[f32; 3]`.
- **Camera stability is the central invariant.** Adding, removing, replacing, moving,
  hiding or selecting objects never moves the camera. Exactly two methods are allowed
  to, and both have to be called explicitly — see [Camera control](./camera.md).
- **The host picks the renderer.** The crate depends on `dioxus` with
  `default-features = false, features = ["lib"]`; it never selects desktop or web
  itself.

## Where it fits

The component is agnostic about where the bytes come from. `GlbSource::Url` covers
HTTP endpoints and models bundled with the app through `manganis`; `GlbSource::Bytes`
covers everything else, which on the desktop includes every file on disk, because
wry/`WebView`2 cannot load `file://` URLs. In the OPOSSUM project the bytes are produced
by `optoscene` and served over the backend, but nothing in this crate assumes that.

## Crate layout

| Path | Role |
|---|---|
| `src/lib.rs` | Crate root; re-exports the public API. |
| `src/model.rs` | `GlbObject`, `GlbSource`, `Transform`, `ViewerOptions`. |
| `src/event.rs` | `PickEvent`, `ViewerEvent` — what the renderer reports back. |
| `src/viewer.rs` | The `GlbViewer` component, `ViewerHandle`, and the boot script. |
| `src/op.rs` | The Rust ↔ JS protocol types (`Op`, `WireSource`, `WireEvent`). |
| `src/diff.rs` | The pure diff: known state + desired state → minimal op list. |
| `src/wire.rs` | Transport: op list → JSON messages, with chunked byte payloads. |
| `assets/viewer.js` | The three.js renderer module that applies the ops. |
| `assets/viewer.css` | Styles for `.dxglb-root` and `.dxglb-canvas`. |
| `assets/three/` | Vendored three.js r174, as an unhashed folder asset. |
| `examples/demo.rs` | A desktop demo app exercising every feature. |

## How to read this book

**Using the component** is what you need to render something:

- [Getting started](./getting-started.md) — add the dependency and show a first model.
- [The declarative scene](./scene.md) — `GlbObject`, `Transform`, `ViewerOptions`, and
  which changes produce which updates.
- [Loading models](./loading-models.md) — URLs, assets, raw bytes, reloads, load errors.
- [Picking and events](./picking.md) — what a click reports, and why a drag is not one.
- [Camera control](./camera.md) — the only two ways to move the camera.
- [Embedding in a Dioxus app](./embedding.md) — sizing, resizing, multiple instances,
  unmount.

**Internals** explains what happens between Rust and JavaScript:

- [The op protocol](./op-protocol.md) — the op list, the diff rules, byte chunking.
- [The JavaScript renderer](./renderer.md) — `viewer.js` from the outside in.

**Maintaining the crate** is for changing it without breaking it:

- [Invariants](./invariants.md) — the rules that break silently when violated.
- [Vendored three.js](./vendoring.md) — which files, why, and how to upgrade them.
- [Testing and checks](./testing.md) — unit tests, lints, and the manual acceptance run.

[Reference](./reference.md) is the compact index of types, props, ops and events.
