# Exporting

Once the scene is assembled, one of the export methods turns it into bytes or files.

## The methods

| Method | Produces |
|---|---|
| `to_glb() -> Result<Vec<u8>>` | A self-contained GLB in memory. |
| `write_glb(writer) -> Result<()>` | The same GLB, streamed to any `std::io::Write`. |
| `to_gltf_separate(bin_uri) -> Result<(String, Vec<u8>)>` | The JSON document (with `buffers[0].uri = bin_uri`) and the binary buffer, for a `.gltf` + `.bin` pair. |
| `write_gltf(dir, stem) -> Result<()>` | Writes `<stem>.gltf` and (if non-empty) `<stem>.bin` into `dir`. |
| `layer_to_glb(layer) -> Result<Vec<u8>>` | A GLB containing only one layer's nodes, same root/layer structure. |
| `nodes_to_glb(uids) -> Result<Vec<u8>>` | A GLB containing only the named nodes, same root/layer structure. |

```rust
use std::path::Path;

// In-memory GLB:
let bytes: Vec<u8> = scene.to_glb()?;

// To a file:
scene.write_glb(std::fs::File::create("scene.glb")?)?;

// As a .gltf + .bin pair:
scene.write_gltf(Path::new("target/out"), "scene")?; // scene.gltf + scene.bin
```

`layer_to_glb` and `nodes_to_glb` power the [streaming diff](./streaming.md): they emit a
partial document that still has the same root and layer groups, so a client can splice it into
an already-loaded scene.

## GLB vs `.gltf` + `.bin`

- **GLB** is a single binary file: a header, a JSON chunk, and a binary chunk. Easiest to
  ship and to stream. `buffers[0]` has no `uri`.
- **`.gltf` + `.bin`** is the same JSON with `buffers[0].uri` pointing at a sidecar binary.
  Handy when you want the JSON to be human-readable next to the data.

Both come from the same builder, so their geometry is identical.

## What the output looks like

The exporter always produces this node hierarchy:

```
optoscene_root            rotation = root_rotation
                          extras = { "origin": [x, y, z] }   (f64 meters)
├─ layer:optics           extras = { "layer": "optics" }
│  ├─ <node.name>         mesh, translation = world − origin (f32), rotation
│  │                      extras = { "uid": …, "layer": "optics", "data": … }
│  └─ …
├─ layer:rays             (only present if the layer has nodes)
└─ layer:aux
```

- A layer group is emitted only if it contains nodes.
- Node translation is `world − origin` as `f32` (see [the origin](./concepts.md#the-origin)).
- Node rotation is the transform's quaternion as `[x, y, z, w]`, `f32`, normalized.
- `extras.data` is omitted when the node's `data` is `None`.
- three.js exposes every node's `extras` as `object.userData`, so `uid`, `layer` and `data`
  are all available on the client.

### Buffers and accessors

One binary buffer holds everything; each attribute and index block gets its own
`bufferView`, aligned to four bytes. Positions are `FLOAT` `VEC3` with the required
`min`/`max`; normals (if present) `FLOAT` `VEC3`, unit length; colors `FLOAT` `VEC4`.
Triangles use `mode` 4, ray lines use `mode` 1 (`LINES`).

## Determinism

Two exports of the same scene are **byte-identical**. The exporter never depends on
`HashMap` iteration order — materials, meshes and nodes are always emitted in insertion order.
This makes output diffable and lets you compare or cache by content.

Next: [Streaming updates](./streaming.md).
