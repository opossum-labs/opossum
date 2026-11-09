# Rays

Ray bundles are added with a single call:

```rust
scene.add_ray_trace(&trace, &style)?;
```

It creates up to two nodes in the `Rays` layer: `"<uid>/lines"` (the ray lines) and
`"<uid>/envelope"` (an optional hull surface).

## `RayTrace`: stations

A `RayTrace` is a set of ray positions sampled at consecutive surfaces ("stations"):

```rust
pub struct RayTrace {
    pub uid: String,
    pub stations: Vec<Vec<Option<Point3<f64>>>>, // stations[k][i] = ray i at station k
    pub wavelength: Option<f64>,                  // meters, used for coloring
}
```

- `stations[k][i]` is ray `i` at station `k`. Rays travel in **straight lines** between
  stations, so put a station wherever the direction changes (at each optical surface).
- `None` marks a **lost** ray (vignetted, clipped) at that station.
- Validation requires at least two stations, all stations the same length, and every present
  coordinate finite; otherwise `Error::InvalidRayTrace`.

Building one by hand:

```rust
use nalgebra::Point3;
use optoscene::RayTrace;

let trace = RayTrace {
    uid: "beam".to_string(),
    stations: vec![
        vec![Some(Point3::new(0.0, 0.0, 0.0)), Some(Point3::new(0.005, 0.0, 0.0))],
        vec![Some(Point3::new(0.0, 0.0, 0.1)), Some(Point3::new(0.005, 0.0, 0.1))],
    ],
    wavelength: Some(532e-9),
};
```

The `fixtures` feature provides ready-made bundles for common cases:
`collimated_bundle`, `focusing_bundle`, `folded_bundle`, `vignetted_bundle`, and the
cross-section helper `concentric_bundle_xy`.

## `RayStyle`: how it is drawn

```rust
pub struct RayStyle {
    pub max_lines: usize,           // cap on rays drawn as lines; 0 disables lines. Default 200
    pub line_color: Option<[f32; 4]>, // explicit color; else derived from wavelength
    pub envelope: Envelope,         // the hull surface. Default Envelope::None
    pub envelope_color: [f32; 4],   // color for DefaultRound. Default [0.2, 0.6, 1.0, 0.25]
}
```

### Lines

The rays valid at station 0 are decimated to at most `max_lines`, in even index steps. For
each chosen ray a line segment is emitted between every consecutive pair of stations where it
is valid at both ends. The lines are drawn with an `Unlit` material so they keep their color
regardless of lighting.

Color precedence: `line_color` if set, else `wavelength_to_rgb(wavelength)` for a wavelength
in 380–780 nm, else a magenta fallback.

### Envelopes

The `Envelope` decides the hull surface around the bundle:

```rust
pub enum Envelope {
    None,                                 // no hull
    Mesh(TriMesh),                        // caller-provided, accurate
    DefaultRound { sectors: u32, steps: u32 }, // crude round fallback
}
```

**`Envelope::Mesh` is the accurate option and the normal choice for real integrations.** The
caller — who knows the apertures, clipping and the true bundle shape — builds the hull as an
ordinary `TriMesh` (with its own materials), and `optoscene` exports it verbatim. The crate
does not analyze or reshape it.

**`Envelope::DefaultRound` is a deliberately crude fallback** for callers that only have ray
positions. It lofts a rotationally-symmetric tube along the bundle's centroid path. For each
segment between two stations:

1. take the rays valid at both stations (fewer than three → the segment is skipped, no error);
2. average their directions into a segment axis;
3. sample `steps + 1` cross-section rings along the segment — the fixed subdivision matters, so
   a focus *between* two stations is not missed (a 5 mm → 5 mm segment would otherwise look
   like a cylinder even though the beam pinches to a point in the middle);
4. at each ring, place `sectors` points at the maximum perpendicular radius of the rays there;
5. loft consecutive rings into a tube (no caps, flat shading).

`Envelope::default_round()` gives the documented defaults (`sectors = 32`, `steps = 16`). It
approximates rotationally — its own docs say so. When you need the real shape, pass
`Envelope::Mesh`.

## Examples

```rust
use optoscene::{Envelope, RayStyle};

// Crude round hull from ray lines only:
scene.add_ray_trace(&trace, &RayStyle {
    max_lines: 60,
    envelope: Envelope::default_round(),
    ..RayStyle::default()
})?;

// Lines only, no hull:
scene.add_ray_trace(&trace, &RayStyle {
    max_lines: 40,
    envelope: Envelope::None,
    ..RayStyle::default()
})?;
```

## Precision

Both ray meshes are stored relative to the center of the bounding box of the trace's valid
points, and that center becomes the node translation — so rays get the same `f32` precision as
optics. A supplied `Envelope::Mesh` is shifted the same way, so lines and hull line up exactly.

The runnable version — a lens plus a focused bundle, once with the round envelope and once with
a hand-built mesh envelope — is the `export_rays` example:

```bash
cargo run -p optoscene --example export_rays --features fixtures
```

Next: [Exporting](./exporting.md).
