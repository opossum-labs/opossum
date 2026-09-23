# The op protocol

Between the `Vec<GlbObject>` you write and the three.js scene sits a small message
protocol. This chapter is what travels across it, who produces each message, and in what
order.

## Two directions

- **Rust → JS**: *ops*, one JSON object per message, sent over the `document::eval`
  channel. Never batched — each op is its own `eval.send()`.
- **JS → Rust**: *events*, sent with `dioxus.send(...)` and received in Rust by
  `eval.recv::<WireEvent>()` inside a long-running future.

Both are externally tagged enums, so the tag field names the variant and the rest of the
object carries its payload.

## Rust → JS: the ops

| Op | Tag | Payload |
|---|---|---|
| `Add` | `add` | `id`, `source`, `transform`, `visible`, `selected` |
| `Reload` | `reload` | same as `Add` |
| `Remove` | `remove` | `id` |
| `SetTransform` | `set_transform` | `id`, `transform` |
| `SetVisible` | `set_visible` | `id`, `visible` |
| `SetSelected` | `set_selected` | `id`, `selected` |
| `SetOptions` | `set_options` | `options` |
| `FitView` | `fit_view` | — |
| `ResetCamera` | `reset_camera` | — |
| `Clear` | `clear` | — |
| `Destroy` | `destroy` | — |
| `BytesBegin` | `bytes_begin` | `key`, `chunks` |
| `BytesChunk` | `bytes_chunk` | `key`, `seq`, `b64` |

`Add` and `Reload` carry transform, visibility and selection rather than leaving them to
follow-up ops. That is what stops a freshly loaded model from being visible for one frame
in the previous pose.

### Where each op comes from

The split matters, because it is what enforces camera stability structurally rather than
by convention:

| Producer | Ops it can emit |
|---|---|
| `diff` | `Add`, `Reload`, `Remove`, `SetTransform`, `SetVisible`, `SetSelected` |
| The options effect | `SetOptions` |
| `ViewerHandle` | `FitView`, `ResetCamera`, `Clear` |
| `use_drop` on unmount | `Destroy` |
| `wire::messages` | `BytesBegin`, `BytesChunk` |

`FitView` and `ResetCamera` are the only ops that move the camera, and `diff` cannot
construct them. No change to your object list can produce one — that is a property of the
code, not a promise in a comment.

### How an op looks on the wire

```json
{
  "op": "add",
  "id": "lens-1",
  "source": { "kind": "url", "url": "/assets/lens-a1b2c3.glb" },
  "transform": {
    "position": [0.0, 0.0, 0.0],
    "rotation_euler_xyz": [0.0, 0.0, 0.0],
    "scale": [1.0, 1.0, 1.0]
  },
  "visible": true,
  "selected": false
}
```

`Transform` and `ViewerOptions` have hand-written `Serialize` implementations, so their
field names are fixed in one place and match what `viewer.js` reads. A renamed Rust field
does not silently change the wire format.

## JS → Rust: the events

| Tag | Payload | Becomes |
|---|---|---|
| `ready` | — | `ViewerEvent::Ready` |
| `loaded` | `id` | `ViewerEvent::Loaded` |
| `load_error` | `id`, `message` | `ViewerEvent::LoadError` |
| `error` | `message` | `ViewerEvent::Error` |
| `pick` | `id`, `mesh_name`, `point`, `shift`, `ctrl`, `alt` | `PickEvent`, to `on_pick` |

Picks go to the required `on_pick` handler; everything else goes to the optional
`on_event` handler and is dropped if there is none.

## The diff

The diff is a pure function: it takes the state the renderer is known to have, the state
you want, and produces the ops that close the gap, updating its record as it goes. The
record is held in a `CopyValue`, deliberately *not* a signal, because writing a signal
would retrigger the very effect that computed the diff.

### The rules

1. **Resolve duplicates first**, keeping the first entry for each `id` and collecting the
   rest for error reporting.
2. **Emit `Remove`** for every id the renderer has that your list no longer contains.
3. **Then, per id:** unknown produces `Add`; a changed `source` produces `Reload`;
   otherwise compare `transform`, `visible` and `selected` and emit one op per differing
   field.
4. An id whose entry is unchanged produces nothing, and **reordering the list produces
   nothing** — position in the `Vec` carries no meaning.

### What order they arrive in

The guarantee is: **every `Remove` precedes every other op.** That is what matters — it
means a wholesale scene swap never has the outgoing and incoming models on the GPU at the
same time, so peak memory is the larger of the two scenes rather than their sum.

Beyond that, adds, reloads and field updates are interleaved in **id order**, not grouped
into an "updates, then adds" sequence. Nothing depends on their relative order, but it is
worth knowing if you are reading an op log and expecting neat blocks.

### Duplicate ids

The first entry wins. Each ignored duplicate is reported as a `ViewerEvent::Error`, and
because the check runs on every diff, a duplicate that stays in your list keeps
reporting. That is intended: it is a bug in the caller, not a transient condition.

## Byte payloads

A `GlbSource::Bytes` model cannot ride along inside the op: a multi-megabyte Base64 string
in a single `evaluate_script` call is not something to rely on. So the transport layer
splits it up and sends the stream **before** the op that references it:

```json
{ "op": "bytes_begin", "key": "file-0", "chunks": 3 }
{ "op": "bytes_chunk", "key": "file-0", "seq": 0, "b64": "Z2xURgIAAAA..." }
{ "op": "bytes_chunk", "key": "file-0", "seq": 1, "b64": "..." }
{ "op": "bytes_chunk", "key": "file-0", "seq": 2, "b64": "..." }
{ "op": "add", "id": "file-0", "source": { "kind": "bytes", "key": "file-0" } }
```

(The `add` also carries `transform`, `visible` and `selected`, omitted here for brevity.)

Chunks hold 256 KiB of raw bytes each, Base64-encoded, so roughly 341 KiB per message. The
`key` is the object's id. Note what is *absent* from the `add`: its source carries only the
key, never the data. The payload field is marked `#[serde(skip)]`, so it cannot leak into
the JSON even by accident.

The chunk size is a plain constant rather than part of the protocol: `bytes_begin`
announces how many chunks follow, so the size can be retuned without touching the
JavaScript.

### Why the payload travels with the op

The `Arc<[u8]>` is cloned into the `WireSource` that the diff produces, so the bytes that
get streamed are by construction the bytes the diff chose.

This replaced an earlier design that rebuilt the payloads in a separate lookup, and that
was a real bug: the lookup resolved duplicate ids **last**-wins while the diff resolves
them **first**-wins, so for a duplicated id the bytes actually streamed were those of the
entry that had been ignored. The model on screen was not the model the winning entry
described. Two regression tests pin this down — one checks that a duplicated id streams the
first entry's payload, the other walks a whole message sequence and asserts that every
bytes op is immediately preceded by its own complete, uninterrupted stream.

## The channel

The eval channel is opened during the component's first render, before the first diff runs.
That ordering is why you never have to wait for `Ready`: ops sent early sit in the channel
and are applied once the renderer is up.

### The boot script

The script Rust evaluates does three things:

1. Dynamically import `viewer.js`. The browser's module cache means several viewers share
   one instance of the module.
2. Call `createViewer(canvasId, options, send, threeBase)` and report `ready`.
3. Loop forever, awaiting ops and handing each to `viewer.apply(op)`, with `destroy`
   breaking the loop after disposing the viewer.

Every value interpolated into that script comes from `serde_json::to_string`, so canvas
ids, asset paths and colour strings are always syntactically valid JS literals, and none
of them can break out of the string they sit in.

### The rule that keeps the channel alive

**The boot script must never `return`.** A `return` resolves the promise Dioxus is
awaiting, Dioxus responds by calling `dioxus.close()`, and from that moment every
`eval.send()` fails with `EvalError::Finished` — a viewer that renders the initial scene
and then silently ignores everything afterwards. The loop is written as `for (;;)` and
leaves only through `break` on `destroy`, when shutting down is what was asked for.

Next: [The JavaScript renderer](./renderer.md).
