# Loading models

A `GlbObject` names its bytes with a `GlbSource`. There are two variants, and the choice
is usually made for you by where the file lives.

## `GlbSource::Url`

The `WebView` fetches the URL itself. This is the cheap path — the bytes never pass
through the eval channel — so prefer it whenever the `WebView` can reach the file.

### Bundled assets

For models shipped with your app, use `manganis`:

```rust
const MODEL: Asset = asset!("/assets/lens.glb");

let source = GlbSource::Url(MODEL.to_string());
```

`option_asset!` is the forgiving variant: it yields `None` instead of failing the build
when the file is missing, which is how the demo stays buildable in a fresh clone.

### Desktop: no `file://`

On the desktop, wry/`WebView`2 **cannot load `file://` URLs**. There is no flag to set and
no path format that works around it. To show a file from the filesystem, read it in Rust
and pass the bytes:

```rust
match std::fs::read(&path) {
    Ok(bytes) => {
        let arc: std::sync::Arc<[u8]> = std::sync::Arc::from(bytes.as_slice());
        objects.write().push(GlbObject {
            id: format!("file-{n}"),
            source: GlbSource::Bytes(arc),
            transform: Transform::default(),
            visible: true,
            selected: false,
        });
    }
    Err(e) => { /* report to the user — the viewer never sees this */ }
}
```

## `GlbSource::Bytes`

Raw `.glb` bytes in an `Arc<[u8]>`. The component Base64-encodes them, streams them to
the `WebView` in chunks, and reassembles them there before handing the buffer to
`GLTFLoader.parse`. All of that is automatic; the mechanics are in [The op
protocol](./op-protocol.md).

Remember the equality rule from [the previous chapter](./scene.md#glbsource-and-what-counts-as-a-change):
`Bytes` compares by pointer identity. Clone the `Arc` you already have; do not re-read
the file to build a new one, or you pay for a reload every time.

Two consequences of the parse path are worth knowing:

- The buffer is parsed with an empty base path, so any **external URI** inside the file
  (a texture referenced by relative path, say) resolves against your page, not against
  the file's original directory. Self-contained `.glb` files — the normal case — are
  unaffected.
- Every byte crosses the eval channel as Base64, which costs about a third more bytes
  than the raw file and arrives as a series of 256 KiB messages. A 20 MB model is
  roughly 80 messages. It works, and the demo does it, but a URL is cheaper.

## Reloading a model

When an object's `source` changes, the component emits a single reload: the old model is
disposed and the new one loaded. Transform, visibility and selection travel **in the same
message**, so the new model is never visible for a frame in the old pose.

Keeping the same `id` across a reload is the point — it is what distinguishes "this
object now shows different geometry" from "a different object appeared".

### Forcing a reload

Because `Url` compares by content, an identical URL is not a change. If the file behind
the URL changed and you want it re-fetched, vary the string — a cache-busting query
parameter is the usual trick, and it is what the demo's *Replace first* button does:

```rust
obj.source = GlbSource::Url(format!("{}?v={}", MODEL, generation));
```

### Overlapping loads

Loads are asynchronous, so a fast sequence of source changes can have several loads in
flight for the same `id` at once. The renderer keeps a generation counter per id and
bumps it on every load: when a load finishes and finds that a newer one has overtaken it,
the result is **discarded and its GPU resources are released** instead of being added to
the scene. The last request you made is the one you see, regardless of which network
response arrives first.

## When loading fails

A failed load reports `ViewerEvent::LoadError { id, message }` through `on_event`, and
the object is simply **not in the scene** — there is no placeholder and no panic. The
`message` comes from `GLTFLoader` or from Base64 decoding.

This is worth wiring up: `on_event` is optional, and without it a model that fails to
load is indistinguishable from one you forgot to add.

### Unsupported compression

**Draco, KTX2 and Meshopt compressed meshes are not supported.** Their decoders are
separate WASM blobs that would have to be vendored alongside three.js, which this crate
deliberately does not do. `GLTFLoader` detects the required extension and fails cleanly,
so a compressed model shows up as an ordinary `LoadError` rather than a broken scene.
If you control the export, export uncompressed.

Next: [Picking and events](./picking.md).
