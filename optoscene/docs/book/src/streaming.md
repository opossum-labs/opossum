# Streaming updates

Re-sending a whole scene after every small change is wasteful. With the **`protocol`** feature,
`optoscene` computes the minimal set of update messages between two scenes, and the companion
`optoscene-protocol` crate frames them for the wire.

```toml
optoscene = { path = "…", features = ["protocol"] }
```

## Messages and framing

A `SceneMessage` is a header plus an optional binary payload; `to_frame()` turns it into wire
bytes:

```rust
use optoscene::{diff, full_scene};

// A full snapshot (payload is the whole scene as GLB):
let frame = full_scene(&scene)?.to_frame()?;

// The minimal updates that turn `old` into `new`:
for message in diff(&old, &new)? {
    let frame: Vec<u8> = message.to_frame()?;
    // send `frame` to the client
}
```

The five header variants (from `optoscene-protocol`):

| Header | Payload | Meaning |
|---|---|---|
| `FullScene` | whole scene GLB | Replace everything. |
| `ReplaceLayer { layer }` | that layer's GLB | Replace one layer group wholesale. |
| `UpsertNodes { uids }` | GLB of those nodes | Add or replace these nodes by uid. |
| `UpdateTransforms { nodes }` | *(none)* | New translation/rotation for existing nodes. |
| `RemoveNodes { uids }` | *(none)* | Remove these nodes. |

## The frame layout

Each frame is a small self-describing envelope (`optoscene-protocol`), all integers
little-endian:

```
| offset | size | contents                          |
|--------|------|-----------------------------------|
| 0      | 4    | magic  b"OSCN"                    |
| 4      | 1    | protocol version (1)              |
| 5      | 4    | header length H (u32)             |
| 9      | H    | header, as UTF-8 JSON             |
| 9 + H  | rest | payload (GLB, or empty)           |
```

`encode(&header, &payload)` builds a frame; `decode(&frame)` returns the header and a borrowed
payload slice, or a `FrameError` (`BadMagic`, `UnsupportedVersion`, `Truncated`,
`HeaderTooLarge`, `Json`). `optoscene-protocol` depends only on `serde`/`serde_json` and
compiles for `wasm32-unknown-unknown`, so a browser can decode frames in Rust/WASM if you
prefer that over JavaScript.

## How `diff` decides

`diff(old, new)` applies these rules in order:

1. If the effective **origin** or the **root rotation** differ, the result is a single
   `FullScene` (every exported coordinate would shift, so a full resend is required).
2. The **`Rays` layer** is treated as a whole: if its set of uids, any node's content, or any
   node's transform differs, one `ReplaceLayer { layer: "rays" }` is emitted, and those nodes
   are excluded from rules 3–5.
3. Non-rays uids present only in `old` → one `RemoveNodes`.
4. Non-rays uids that are new, or whose content changed (mesh incl. materials, name, layer,
   data) → one `UpsertNodes`, in `new`'s insertion order.
5. The remaining non-rays nodes whose transform changed (translation > 1e-9 m or rotation
   > 1e-9 rad) → one `UpdateTransforms`, with translation relative to the origin as `f32`.

Messages are ordered **`RemoveNodes`, `UpsertNodes`, `UpdateTransforms`, `ReplaceLayer`**; an
unchanged scene yields an empty vector.

> **Why rays are all-or-nothing.** A ray node's local vertices are stored relative to the
> bundle center, and the center *is* the node translation. If the whole bundle merely
> translates, the local geometry is unchanged and only the transform moves — which is exactly
> the case rule 2's transform check catches, so a moved bundle still triggers `ReplaceLayer`.

## Keeping the origin stable

Because rule 1 turns any origin change into a full resend, pin the origin
(`SceneOptions::origin = Some(p)`) when you expect incremental updates — otherwise moving a
node shifts the auto-computed bounding-box center and forces a `FullScene` every time. A
positioning loop that nudges a component should keep a fixed origin so it emits
`UpdateTransforms`/`ReplaceLayer`, not `FullScene`.

Next: [Embedding in a web backend](./web-backend.md).
