# Introduction

`optoscene` turns ready-made 3D geometry into [glTF 2.0](https://www.khronos.org/gltf/)
(GLB or `.gltf` + `.bin`) and makes that convenient for **optical scenes** — lenses,
mirrors, detectors and the ray bundles that travel between them — so they can be rendered
in a browser with [three.js](https://threejs.org/).

## What it is, and what it is not

`optoscene` is an **export** crate, not a geometry crate.

- It **does** assemble geometry into a scene graph, map optical materials to physically
  based glTF materials, decimate ray bundles into lines, deduplicate meshes and materials,
  shift coordinates to a numerically friendly origin, and serialize everything into
  deterministic bytes.
- It **does not** compute geometry from physical quantities. Tessellation, vertex normals
  and the true shape of a ray bundle are supplied *by the caller*, because only the caller
  knows the parametrization, the apertures and the vignetting. The one exception is a
  deliberately crude default envelope ([`Envelope::DefaultRound`](./rays.md)) for callers
  that only have ray lines but still want to see a 3D body.

This split keeps the crate small and dependency-light, and keeps the physics where it
belongs — in the caller.

## Conventions

- **Units are meters.** Every input length and coordinate is meters.
- **Input is `f64`, output is `f32`.** You build a scene with `f64` `nalgebra` types; the
  cast to the `f32` that glTF stores happens only in the exporter, after coordinates are
  shifted relative to a scene origin so they stay small and precise.
- **Output is deterministic.** Identical input produces byte-identical output. The exporter
  never iterates a `HashMap` when producing bytes; it follows insertion order.
- **The library never panics on your input.** Invalid meshes, ray traces or ids return an
  [`Error`](./reference.md#errors), never a panic.

## Where it fits

`optoscene` is transport-agnostic: it hands you `Vec<u8>`, and your application ships the
bytes however it likes. In the OPOSSUM project the bytes travel over an existing actix-web
backend to a Dioxus frontend, but the same pattern works with axum, Tauri or Dioxus IPC —
see [Embedding in a web backend](./web-backend.md).

## Workspace layout

| Crate | Role |
|---|---|
| `crates/optoscene` | The core export library. Returns bytes; the caller ships them. |
| `crates/optoscene-protocol` | The streaming frame format. Compiles for `wasm32-unknown-unknown` (no `std::fs`, no threads). |
| `examples/actix-viewer` | An example actix-web server plus a three.js viewer, showing the embedding pattern end to end. |

## How to read this book

- [Getting started](./getting-started.md) — add the crate and export your first GLB.
- [Core concepts](./concepts.md) — the `Scene`, materials, meshes, nodes, layers, the origin.
- [Building a lens](./building-a-lens.md) — a full worked optical component.
- [Rays](./rays.md) — ray traces, line decimation and bundle envelopes.
- [Exporting](./exporting.md) — every output form and the glTF structure produced.
- [Streaming updates](./streaming.md) — the `protocol` feature: `diff` and the frame format.
- [Embedding in a web backend](./web-backend.md) — the actix + three.js example, dissected.
- [Reference](./reference.md) — a compact API and glossary.
