# Embedding in a web backend

`optoscene` is transport-agnostic: it hands you bytes, and your application ships them. The
`examples/actix-viewer` crate wires this up end to end with actix-web and a three.js viewer,
but the same shape works with axum, Tauri or Dioxus IPC — only the transport changes.

Run it and open <http://127.0.0.1:8080>:

```bash
cargo run -p actix-viewer
```

Every two seconds it nudges the lens along the optical axis and rebeams the bundle,
simulating a positioning run, and streams the diff to every connected viewer.

## The server side

The state is just the current scene behind a lock, plus a broadcast channel of frames:

```rust
struct AppState {
    scene: Mutex<Scene>,
    frames: broadcast::Sender<Vec<u8>>,
}
```

Routes:

- `GET /` and `GET /viewer.js` serve the static viewer files.
- `GET /scene.glb` serves the current scene as a standalone GLB (via `to_glb`).
- `GET /ws` upgrades to a WebSocket: it **first** sends a `FullScene` frame (snapshotted under
  the same lock the update loop uses, so nothing is lost or double-applied), then forwards
  broadcast frames as they arrive.

The demo loop is the pattern to copy for a real integration:

```rust
let next = build_scene(lens_z);            // rebuild after the change
let messages = {
    let mut scene = state.scene.lock().unwrap();
    let messages = diff(&scene, &next).unwrap_or_default();
    *scene = next;                          // adopt the new scene
    messages
};
for message in &messages {
    if let Ok(frame) = message.to_frame() {
        let _ = state.frames.send(frame);   // broadcast
    }
}
```

The scene pins its origin (`origin: Some(...)`), so moving the lens produces
`UpdateTransforms` and `ReplaceLayer("rays")` frames — never a repeated `FullScene`.

## The client side

`viewer.js` decodes each frame and applies it. Decoding follows the
[frame layout](./streaming.md#the-frame-layout) exactly:

```js
function decodeFrame(buffer) {
    const view = new DataView(buffer);
    // bytes 0..4 must spell "OSCN", byte 4 is the version (1)
    const headerLength = view.getUint32(5, true); // little-endian
    const header = JSON.parse(
        new TextDecoder().decode(new Uint8Array(buffer, 9, headerLength)),
    );
    const payload = buffer.slice(9 + headerLength);
    return { header, payload };
}
```

The viewer keeps one three.js group per layer and a `Map` from `uid` to `Object3D`, then
applies each message type:

| Message | Client action |
|---|---|
| `full_scene` | Remove and dispose everything, then load the GLB. |
| `replace_layer` | Remove and dispose the existing layer group, insert the group from the GLB. |
| `upsert_nodes` | For each object with a `userData.uid` in the GLB, replace the existing object of that uid, or add the new one under its layer group. |
| `update_transforms` | Set each node's `position` and `quaternion`. |
| `remove_nodes` | Remove and dispose the named nodes. |

The node `extras` written by the exporter arrive as `object.userData` automatically, so the
`uid`/`layer` needed to route each object are already on the loaded objects.

> **Always dispose.** On every removal, free the GPU resources —
> `geometry.dispose()` and `material.dispose()` — or memory grows with each update. The
> example's disposal keeps `renderer.info.memory.geometries` flat across updates.

## Transport independence

Nothing above is specific to actix or WebSockets. The library produced `Vec<u8>` frames; the
server only chose how to deliver them. Swap in axum + SSE, a Tauri IPC channel, or Dioxus
eval — the encode/decode and client semantics are identical.

## Renderer notes

- The output targets three.js with `GLTFLoader`. Node `extras` become `object.userData`.
- **Glass needs an environment map.** `Glass` materials use `KHR_materials_transmission`, and
  transmission renders dark without an environment to refract. Provide one (e.g. three.js
  `RoomEnvironment` through a `PMREMGenerator`); the example does this at startup.

Next: [Reference](./reference.md).
