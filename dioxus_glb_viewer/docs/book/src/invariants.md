# Invariants

Most mistakes in a codebase announce themselves: a test fails, a type does not check,
something panics. The rules in this chapter are the other kind. Break one and the crate
still compiles, the tests still pass, and the component still renders — it just quietly
stops doing the one thing it exists to do.

Read this before changing `viewer.rs` or `viewer.js`.

| Invariant | What a violation looks like |
|---|---|
| The RSX around the canvas stays static | The view jumps whenever the scene changes |
| The boot script never returns | Initial scene renders, then all updates are ignored |
| The eval channel is opened in `use_hook` | Occasional lost ops right after mount |
| The applied-state record is not a signal | Infinite effect loop |
| `GlbSource::Bytes` compares by pointer | Multi-megabyte `memcmp` on every parent render |
| No diff-producible op moves the camera | The view jumps whenever the scene changes |
| `viewer.js` is not hot-reloadable | JS edits appear to be ignored |

## The RSX around the canvas stays static

The component's template is exactly one `div.dxglb-root` containing one
`canvas#dxglb-<N>`. **Never put an `if`, a `key`, or an interpolated node around that
subtree.**

Dioxus diffs templates structurally. As long as the shape is fixed it patches the canvas
element's attributes, and the element — with its WebGL context, its camera and everything
in its scene — survives. Introduce a conditional or a key and Dioxus is entitled to
*replace* the element instead. A new canvas means a new WebGL context, an empty scene, and
a camera back at its initial pose.

Two things protect this today, and both should stay:

- The canvas id is generated in a `use_hook`, which runs exactly once per component
  instance. The id therefore never changes across renders, so the attribute never forces a
  replacement.
- The template contains no conditional nodes at all. A loading spinner or an error banner
  must go *beside* the root div, not around the canvas.

**Symptom:** the user orbits to a useful view, adds a model, and the view snaps back to
the default angle. Camera stability is the component's core promise, so this is a serious
regression that no automated test will catch.

**How to check:** run the demo, rotate and zoom to a distinctive oblique view, then
trigger whatever state change you touched. The view must not move.

## The boot script never returns

The script evaluated in `document::eval` ends in a `for (;;)` loop that awaits ops. It
leaves that loop only via `break`, and only on the `destroy` op.

A `return` resolves the promise Dioxus is awaiting. Dioxus takes that as "this eval is
finished" and calls `dioxus.close()`. From then on every `eval.send()` fails with
`EvalError::Finished`.

**Symptom:** the initial scene appears correctly and then nothing ever updates again. No
error, no panic — the Rust side keeps computing correct ops and sending them into a closed
channel.

**Related:** every value interpolated into the boot script comes from
`serde_json::to_string`, which guarantees a syntactically valid JS literal. Keep it that
way; a hand-quoted interpolation is how a canvas id or a colour string turns into a syntax
error at runtime.

## The eval channel is opened in `use_hook`

`document::eval` is called in a `use_hook`, which runs during the component's first
render — **before** the effect that computes the first diff.

Moving it into an effect would look harmless and introduce a window in which the diff has
already produced ops but there is no channel to send them on. `Eval` is `Copy` and its
storage belongs to the JavaScript side rather than the component scope, which is why
copies of it are valid inside spawned futures and inside `use_drop`.

**Symptom:** intermittent, timing-dependent loss of the first ops after mount — the
hardest possible class of bug to reproduce.

## The applied-state record is not a signal

The record of what the renderer currently shows lives in a `CopyValue`, not a `Signal`.

The diff runs inside an effect that reads your `objects` signal, and it *writes* that
record as it goes. If the record were a signal, writing it would mark it dirty, which
would rerun the effect, which would write it again.

**Symptom:** an effect that never settles — a busy loop rather than a crash.

## `GlbSource::Bytes` compares by pointer

Equality for `Bytes` is `Arc::ptr_eq`, not content comparison. This is a performance
requirement, not a preference: Dioxus's props macro compares old and new props on every
render of the *parent* component, and content comparison would `memcmp` the entire model
each time — tens of megabytes, repeatedly, for a large file.

Pointer identity is also the honest semantic. A new `Arc` means the caller produced a new
model; the worst consequence is one unnecessary reload, never a wrong image.

**Symptom of "fixing" this:** the UI grows sluggish in proportion to model size, with no
obvious culprit in a profile beyond a lot of time in `memcmp`.

## No diff-producible op moves the camera

`FitView` and `ResetCamera` exist only on `ViewerHandle`. The diff cannot construct them,
and that is deliberate — it makes camera stability a property of the code rather than a
convention to remember.

If you add an op that moves the camera, keep it out of the diff's reach.

**Symptom:** the same as a broken canvas shape — the view jumps when the scene changes —
which is why the two are worth distinguishing when you go looking for the cause.

## `viewer.js` is not hot-reloadable

The browser caches ES modules by URL. An existing viewer instance holds the module it
already imported, so editing `assets/viewer.js` on disk changes nothing in a running
`dx serve` session.

**Restart `dx serve` after every JavaScript edit.** Do not diagnose this as a broken asset
pipeline, a stale build, or a caching bug to be worked around — it is how module loading
works.

## Structure worth preserving

Not invariants, but decisions that will look arbitrary and are not:

- **Two separate effects**, one for the object diff and one for options, so that changing a
  background colour does not run the object list through the diff.
- **Byte payloads travel inside the op** rather than in a parallel lookup. The parallel
  version was a real bug; see [The op protocol](./op-protocol.md#why-the-payload-travels-with-the-op).
- **`ViewerEvent` is `#[non_exhaustive]`**, so new variants are not breaking changes for
  downstream matches.

Next: [Vendored three.js](./vendoring.md).
