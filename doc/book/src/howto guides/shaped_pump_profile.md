# Small-signal gain with a shaped pump profile

This guide shows how to move beyond a constant gain factor and model an amplifier head where the
pump beam itself is shaped: it has a transversal intensity profile across the aperture and it is
partially absorbed along the optical axis. The [`amplifier_chain.rs`](model_an_amplifier.md)
example covers the simpler `Const` model; here the gain field depends on position inside the
medium, so two parallel rays through the same slab leave with different gain factors.

## When to use `Monochromatic Small Signal Gain` instead of `Const`

`Const` assigns one factor to the whole ray bundle regardless of where a ray enters the medium or
how long its chord through it is. That is appropriate for chain layout and first estimates. Use
`Monochromatic Small Signal Gain` when any of the following matters:

- The pump spot does not cover the full aperture — rays near the edge are less pumped than axial
  ones.
- The pump is end-pumped and partially absorbed — the near face of the medium is more strongly
  inverted than the far face.
- Oblique rays travel longer chords and therefore accumulate more gain than on-axis ones.
- The analysis needs to be run in ray tracing or ghost focus mode (energy flow cannot use
  `Small Signal Gain` because it carries no per-ray geometry).

`Monochromatic Small Signal Gain` is an *unsaturated* model: extracting energy does not draw the
inversion down. It holds as long as the extracted fluence is small compared with the stored
fluence in the medium. For saturating amplifiers the next gain stage (Frantz–Nodvik) will be the
right tool.

## Setting it up in code

The complete, runnable program that shows these concepts with a fluence detector is
`opossum_core/examples/amplifier_gaussian_pump_fluence.rs`; run it with

```bash
cargo run -p opossum_core --example amplifier_gaussian_pump_fluence
```

That example uses a flat longitudinal profile to isolate the transversal effect. The code
below focuses on the gain and pump source configuration for a Lambert-Beer longitudinal
profile; the rest of the setup follows the same pattern.

The hardware is a plane-parallel glass slab — a [`Lens`](../reference/nodes/spherical_lens.md)
with infinite radii of curvature. The slab is 40 mm thick and refractive index 1.5, exactly as any
passive lens would be. Nothing about the node says "amplifier":

```rust
use opossum_core::{
    gain::{
        AnalyticPump, LambertBeerProfile, GainModel, LongitudinalProfile, MonochromaticSmallSignalGain,
        PumpDirection, PumpSource, TransversalProfile,
    },
    prelude::*,
    reciprocal_centimeter,
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
carries the *magnitude* — the peak gain coefficient g₀ where the pump is strongest; the pump
source carries only the *shape* — the spatial profile β ∈ [0, 1] that says how strongly each
point of the medium is pumped relative to that peak:

```rust
// Gain model: unsaturated path-integral gain.
// g₀ is the peak small-signal gain coefficient at the strongest point of the pump profile.
scenario.set_gain_model(
    head,
    GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
        reciprocal_centimeter!(0.3), // peak g₀ where the pump is strongest (axis, near face)
    )?),
);

// Pump source: Super-Gaussian spot × Lambert-Beer longitudinal decay.
// The pump carries only the shape and the grid it is resolved on — no amplitude here.
scenario.set_pump_source(
    head,
    PumpSource::Analytic(AnalyticPump::new(
        TransversalProfile::SuperGaussian(SuperGaussianShape::new(
            millimeter!(0.0, 0.0),  // pump centered on the optical axis
            millimeter!(5.0, 5.0),  // 1/e² half-width 5 mm along x and y
            2.0,                    // order 2: flatter top, steeper edges than a Gaussian
            degree!(0.0),
            false,
        )?),
        LongitudinalProfile::LambertBeer(LambertBeerProfile::new(
            reciprocal_centimeter!(0.5), // pump absorption coefficient in the medium
            PumpDirection::Forward,      // pump enters from the input surface
        )?),
        (64, 64, 32), // 64 × 64 transversal, 32 longitudinal cells
    )?),
);
```

### Choosing the grid

The `(64, 64, 32)` grid is the third argument of `AnalyticPump::new` — the resolution at which
the pump shape β is discretized over the medium. Each cell holds one β value. A ray traverses the
cells exactly (Amanatides–Woo algorithm), so there is no step-size error — only the
discretization of the pump profile onto the grid introduces approximation. Rules of thumb:

- Make `cells_z` fine enough to resolve the longitudinal profile: for Lambert-Beer with
  α = 0.5 cm⁻¹ over 40 mm, roughly 16–32 slices are sufficient for < 1 % error.
- Make `cells_x` and `cells_y` fine enough to resolve the transversal profile: a Super-Gaussian
  with σ = 5 mm over a 20 mm aperture needs around 32–64 cells to be well-sampled.
- Finer grids cost proportionally more memory but no additional traversal time per ray.

A constant pump (`PumpSource::Const`) has no shape to resolve, so it needs no grid and no grid
argument — the model integrates over the exact chord the ray travels through the body.

## Running the analysis

`Monochromatic Small Signal Gain` requires a ray-tracing or ghost-focus analysis; an energy-flow
analysis is refused because it carries no per-ray geometry and cannot integrate g₀ along a path. Configuring
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

Open the `amplifier_gaussian_pump_fluence.opm` file in the GUI, run the analysis in the
`Gaussian pump` scenario, and open the fluence detector report. The fluence map shows:

- A bright central spot where the Gaussian pump profile is strongest and gain is highest.
- A smooth radial fall-off that mirrors the pump's σ = 5 mm half-width.
- Near-zero fluence beyond the pump spot (r ≫ σ): rays there barely see the pump.

To see the Lambert-Beer longitudinal effect on top of a transversal profile, configure the
pump source as shown above (Super-Gaussian × Lambert-Beer) and compare the fluence map with
the flat-profile result: oblique rays and axial rays at the same radius will carry slightly
different energies because they enter the slab at different longitudinal inversion gradients.

For parameter definitions and the full list of transversal and longitudinal profile options see
[Pump scenarios](../reference/pump_scenarios.md).
