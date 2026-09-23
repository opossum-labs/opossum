# Embedding in a Dioxus app

The component is one `<canvas>`, one WebGL context, one three.js renderer. Putting it in
an app is mostly a matter of giving it a size and letting it clean up after itself.

## Sizing

`GlbViewer` renders a `div.dxglb-root` containing a `canvas.dxglb-canvas`, and both are
`width: 100%; height: 100%`. The component therefore fills its parent and never decides
its own size — **the host controls the size.**

Any global attribute you pass is forwarded to that outer `div`:

```rust
GlbViewer { style: "flex: 1; height: 100%;", objects, on_pick }
GlbViewer { class: "viewer-pane", objects, on_pick }
```

> **The most common mistake** is a parent with automatic height. `height: 100%` resolves
> against the parent's height, so inside a parent that is only as tall as its content the
> canvas collapses to zero and you get a blank area with no error anywhere. Give the
> parent a definite height (`height: 600px`, `height: 100vh`, or `flex: 1` inside a sized
> flex container).

## Resizing

The canvas watches itself with a `ResizeObserver`, and the render loop picks up the new
size on the next frame.

This is deliberately **not** `window.resize`. A splitter that narrows a side panel, a
collapsing sidebar, or a CSS grid that reflows all change the canvas size without the
window changing at all, and a `window.resize` listener would sleep through every one of
them. Observing the element covers those cases and the window case alike.

The renderer is resized without writing inline CSS onto the canvas, because doing so
would change the element's size and retrigger the observer — a feedback loop.

## Several viewers at once

Multiple `GlbViewer`s in one app are fine. Each instance takes a unique canvas id
(`dxglb-0`, `dxglb-1`, …) from a global counter, and they share one cached copy of the
JavaScript module, so the second viewer costs no extra download.

They do not share a WebGL context, though, and browsers cap the number of live contexts
at roughly sixteen. Beyond that the oldest contexts start being dropped. The component
explicitly forces its context loss on teardown rather than waiting for the garbage
collector, so viewers that come and go do not accumulate — but a UI that shows twenty
viewers at once is asking for trouble regardless.

## Teardown

Two independent mechanisms make sure a removed viewer stops consuming resources:

1. On unmount the component sends a `Destroy` message, and the renderer stops its render
   loop, disconnects the observer, removes every DOM listener at once, disposes controls,
   models, grid and renderer, and forces the WebGL context loss.
2. Independently, the render loop checks each frame whether the canvas is still attached
   to the document, and shuts itself down if it is not. This catches unmounts where the
   `Destroy` message never arrives.

You do not have to do anything for either of these. What you *do* have to avoid is
forcing Dioxus to rebuild the canvas element while the viewer is alive — see
[Invariants](./invariants.md).

## Desktop and web

The crate takes no position on the platform: it depends on `dioxus` with only the `lib`
feature and your app chooses the renderer. The one behavioural difference to plan for is
that on the desktop wry/`WebView`2 cannot fetch `file://` URLs, so local files must go
through `GlbSource::Bytes` — see [Loading models](./loading-models.md#desktop-no-file).

## A worked example

`examples/demo.rs` is a complete desktop app: a side panel of buttons beside a viewer
filling the rest of the window. It is the best place to see the pieces together, and it
is laid out the way a real integration would be.

```rust
div { style: "display:flex; height:100vh;",
    aside { style: "width:260px; flex-shrink:0;",
        // buttons that mutate `objects`, and the camera handle's methods
    }
    GlbViewer {
        style: "flex:1; height:100%;",     // sized by the flex parent
        objects,                            // the scene
        handle: viewer_handle,              // fit / reset / clear
        options: use_signal(|| ViewerOptions { .. }),
        on_pick: move |p: PickEvent| { /* selection lives here */ },
        on_event: move |e: ViewerEvent| { /* log loads and errors */ },
    }
}
```

Note the division of labour, which is the pattern to copy: every button mutates the
caller's own `objects` signal and lets the diff do the work. Only *Fit view*, *Reset
camera* and *Clear all* reach for the handle, because those are the only three things the
declarative scene cannot express.

Run it with:

```bash
cd dioxus_glb_viewer
dx serve --example demo --features desktop --platform desktop
```

Next: [The op protocol](./op-protocol.md).
