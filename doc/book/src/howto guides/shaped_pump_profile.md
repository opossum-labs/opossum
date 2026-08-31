# Small-signal gain with a shaped pump profile

This guide shows how to move beyond a constant gain factor and model an amplifier head where the
pump beam itself is shaped: it has a transversal intensity profile across the aperture and it is
partially absorbed along the optical axis. The [`amplifier_chain.rs`](model_an_amplifier.md)
example covers the simpler `Const` model; here the gain field depends on position inside the
medium, so two parallel rays through the same slab leave with different gain factors.

## When to use `Small Signal Gain` instead of `Const`

`Const` assigns one factor to the whole ray bundle regardless of where a ray enters the medium or
how long its chord through it is. That is appropriate for chain layout and first estimates. Use
`Small Signal Gain` when any of the following matters:

- The pump spot does not cover the full aperture — rays near the edge are less pumped than axial
  ones.
- The pump is end-pumped and partially absorbed — the near face of the medium is more strongly
  inverted than the far face.
- Oblique rays travel longer chords and therefore accumulate more gain than on-axis ones.
- The analysis needs to be run in ray tracing or ghost focus mode (energy flow cannot use
  `Small Signal Gain` because it carries no per-ray geometry).

`Small Signal Gain` is an *unsaturated* model: extracting energy does not draw the inversion
down. It holds as long as the extracted fluence is small compared with the stored fluence in the
medium. For saturating amplifiers the next gain stage (Frantz–Nodvik) will be the right tool.

## Setting it up in code

The complete, runnable program is `opossum_core/examples/inhomogeneous_small_signal.rs`; run it
with

```bash
cargo run -p opossum_core --example inhomogeneous_small_signal
```

The hardware is a plane-parallel glass slab — a [`Lens`](../reference/nodes/spherical_lens.md)
with infinite radii of curvature. The slab is 40 mm thick and refractive index 1.5, exactly as any
passive lens would be. Nothing about the node says "amplifier":

```rust
use opossum_core::{
    gain::{
        AnalyticPump, BeerLambertProfile, GainModel, LongitudinalProfile, PumpDirection,
        PumpSource, SmallSignalGain, TransversalProfile,
    },
    prelude::*,
    reciprocal_centimeter, square_centimeter,
    utils::super_gaussian::SuperGaussianShape,
};

let head = scenery.add_node(Lens::new(
    "amplifier head",
    millimeter!(f64::INFINITY),  // flat input surface
    millimeter!(f64::INFINITY),  // flat output surface
    millimeter!(40.0),
    RefrIndexConst::new(1.5)?,
)?)?;
```

A pump scenario assigns the gain model and the pump source to that node separately. The gain model
says *how* a ray accumulates gain from the inversion field; the pump source says *what that field
looks like*:

```rust
// Gain model: unsaturated path-integral gain.
// σ_e is the emission cross section of the medium at the laser wavelength.
scenario.set_gain_model(
    head,
    GainModel::SmallSignalGain(SmallSignalGain::new(
        square_centimeter!(2.0e-20), // σ_e ≈ 2 × 10⁻²⁰ cm² (typical solid-state medium)
        (64, 64, 32),               // 64 × 64 transversal, 32 longitudinal cells
    )?),
);

// Pump source: Super-Gaussian spot × Beer-Lambert longitudinal decay.
scenario.set_pump_source(
    head,
    PumpSource::Analytic(AnalyticPump::new(
        reciprocal_centimeter!(0.3),  // peak g₀ at the profile center (axis, near face)
        TransversalProfile::SuperGaussian(SuperGaussianShape::new(
            millimeter!(0.0, 0.0),  // pump centered on the optical axis
            millimeter!(5.0, 5.0),  // 1/e² half-width 5 mm along x and y
            2.0,                    // order 2: flatter top, steeper edges than a Gaussian
            degree!(0.0),
            false,
        )?),
        LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
            reciprocal_centimeter!(0.5), // pump absorption coefficient in the medium
            PumpDirection::Forward,       // pump enters from the input surface
        )?),
    )?),
);
```

### Choosing the grid

The `(64, 64, 32)` grid is the resolution at which the inversion field is discretized. Each cell
holds one inversion density value. A ray traverses the cells exactly (Amanatides–Woo algorithm),
so there is no step-size error — only the discretization of the pump profile onto the grid
introduces approximation. Rules of thumb:

- Make `cells_z` fine enough to resolve the longitudinal profile: for Beer-Lambert with
  α = 0.5 cm⁻¹ over 40 mm, roughly 16–32 slices are sufficient for < 1 % error.
- Make `cells_x` and `cells_y` fine enough to resolve the transversal profile: a Super-Gaussian
  with σ = 5 mm over a 20 mm aperture needs around 32–64 cells to be well-sampled.
- Finer grids cost proportionally more memory but no additional traversal time per ray.

### The emission cross section

`SmallSignalGain` holds one emission cross section σ_e. It is a parameter of the model, not a
property of the material node, because the material does not carry spectroscopic data yet. The
value cancels out exactly when the pump source and the gain model both use the same σ_e (the pump
converts g₀ → inversion density via σ_e; the gain model converts density → g₀ back, via the same
σ_e). It only becomes a physical input at the next model stage, when gain narrowing across λ
requires a σ_e(λ) curve. For now, any positive finite value produces a physically correct result
as long as both halves share it.

## Running the analysis

`Small Signal Gain` requires a ray-tracing or ghost-focus analysis; an energy-flow analysis is
refused because it carries no per-ray geometry and cannot integrate g₀ along a path. Configuring
one is the same as for any other model:

```rust
let mut config = RayTraceConfig::default();
config.map_source(
    source,
    round_collimated_ray_builder(millimeter!(9.0), joule!(1.0), 5)?,
);
let analyzer_id = document.add_analyzer(AnalyzerType::RayTrace(config));

// Tell the analyzer which scenarios to run — without this the model is analyzed passively.
let scenario_ids = document.pump_scenarios().keys().copied().collect();
document
    .analyzer_mut(analyzer_id)
    .expect("the analyzer just added must be there")
    .set_pump_scenarios(scenario_ids);
```

`round_collimated_ray_builder` produces five concentric rings of collimated rays up to a radius of
9 mm. With the pump spot at σ = 5 mm, the inner rings (r ≈ 0–2 mm) sit near the peak of the
profile; the outer rings (r ≈ 7–9 mm) sit on the tail where the profile has fallen to a small
fraction of its peak.

## What to look for in the result

Open the resulting `.opm` file in the GUI, run the analysis in the `shaped pump` scenario, and
open the spot diagram report. Each dot in the diagram represents one ray; its color encodes the
per-ray energy. You should see:

- Rays near the optical axis carry the highest energy — the pump spot is strongest there.
- Energy drops toward the aperture edge because the Super-Gaussian profile falls off.
- Rays at the same radius but different azimuth carry the same energy (because the profile is
  circularly symmetric and the pump is centered on axis).
- The Beer-Lambert decay along z does not produce a difference between parallel axial rays —
  all of them traverse the same longitudinal inversion gradient at their respective radii.
  The longitudinal effect would show up in the gain difference between an axial ray and an
  oblique ray that crosses more strongly inverted slices near the entrance face.

For parameter definitions and the full list of transversal and longitudinal profile options see
[Pump scenarios](../reference/pump_scenarios.md).
