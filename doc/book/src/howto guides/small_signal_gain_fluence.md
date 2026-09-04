# Visualizing the pump profile in the output fluence

This guide shows what happens when a collimated beam with a uniform energy distribution passes
through a medium whose pump spot does not fill the full aperture. Because the gain is stronger
where the pump is most intense, the output fluence map at the detector reproduces the transversal
pump profile — even though the input beam was flat. The complete, runnable program is
`opossum_core/examples/amplifier_gaussian_pump_fluence.rs`; run it with

```bash
cargo run -p opossum_core --example amplifier_gaussian_pump_fluence
```

## The setup

The optical chain is minimal: a source, a single amplifier head, and a fluence detector.

```
oscillator ──[200 mm]── amplifier head ──[100 mm]── output fluence
```

The amplifier head is a plane-parallel glass slab (flat surfaces, 100 mm thick, n = 1.5). No
special node type is needed — a [`Lens`](../reference/nodes/spherical_lens.md) with infinite radii
of curvature is the active medium. Marking it as an amplifier is what makes the GUI show it in the
pump-scenario editor:

```rust
document.set_is_amplifier_node(head, true);
```

## Pump scenario

A pump scenario assigns the gain model and the pump source to the head separately.

**Gain model.** `MonochromaticSmallSignalGain` integrates the local gain coefficient along each
ray's path through the medium: `G = exp(∫ g₀·β ds)`. The peak coefficient `g₀ = 0.2 cm⁻¹` is the
gain per unit length where the pump is strongest (on axis). For a ray going straight through the
100 mm slab, the peak single-pass gain is

```text
G = exp(g₀ · L) = exp(0.2 cm⁻¹ × 10 cm) = exp(2) ≈ 7.4
```

**Pump source.** The pump carries only the spatial *shape* — how the gain coefficient is
distributed across the medium. Here it is a standard Gaussian (`SuperGaussian` of order 1) with a
half-width σ = 5 mm, combined with a flat longitudinal profile. A flat longitudinal profile means
the inversion is the same at the entrance face and at the exit face, so the only spatial variation
is the Gaussian fall-off across the aperture.

```rust
scenario.set_gain_model(
    head,
    GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
        reciprocal_centimeter!(0.2),
    )?),
);
scenario.set_pump_source(
    head,
    PumpSource::Analytic(AnalyticPump::new(
        TransversalProfile::SuperGaussian(SuperGaussianShape::new(
            millimeter!(0.0, 0.0),  // pump centered on the optical axis
            millimeter!(5.0, 5.0),  // 1/e² half-width 5 mm
            1.0,                    // order 1 = standard Gaussian
            degree!(0.0),
            false,
        )?),
        LongitudinalProfile::Flat,   // uniform inversion from entrance to exit
        (64, 64, 16),                // 64 × 64 transversal cells; 16 longitudinal
    )?),
);
```

For an explanation of the grid argument and a comparison with the Lambert-Beer longitudinal
profile, see [Shaped pump profiles](./shaped_pump_profile.md).

## Source and detector

The input source uses 10 000 rays distributed on a Fibonacci grid, which tiles a circular aperture
of radius 10 mm without clustering. The energy distribution is a high-order Super-Gaussian (order
8 ≈ flat-top): every ray starts with roughly the same energy per unit area, so any fluence
variation at the output is entirely due to position-dependent gain, not to the input beam shape.

The output detector is a [`FluenceDetector`](../reference/nodes/fluence_detector.md) using the
Voronoi estimator, which assigns each ray an area equal to its Voronoi cell and reports the energy
per area. With a Fibonacci source grid the cells have nearly equal area, giving a low-noise
estimate.

## What to look for

Open the `.opm` file in the GUI, select the `Gaussian pump` scenario in the pump-scenarios panel,
and run the analysis. The fluence map at the output detector should show:

- A bright circular spot in the centre where the Gaussian pump profile is strongest, and where
  the gain is highest.
- A smooth radial fall-off that mirrors the pump's σ = 5 mm half-width. At a radius of 5 mm the
  fluence has dropped to 1/e of its axial value.
- Near-zero fluence beyond the pump spot (r ≫ σ): rays there barely see the pump and leave the
  medium essentially unamplified.

If you switch the gain model to `Const` with the same overall factor for a quick comparison, the
fluence map becomes uniform across the aperture — the spatial information that `MonochromaticSmallSignalGain`
preserves is lost.

## When to use a flat longitudinal profile

`LongitudinalProfile::Flat` is the right choice when the pump is not significantly absorbed on its
way through the medium — for example when the medium is short compared with the pump absorption
length, or when the pump is a side-pump geometry whose longitudinal variation is captured in the
transversal profile instead. For end-pumped media where the pump is partially absorbed along the
optical axis, `LongitudinalProfile::LambertBeer` models the exponential fall-off of the pump
intensity from entrance to exit. That case is covered in
[Shaped pump profiles](./shaped_pump_profile.md).

For the full list of transversal and longitudinal profile options and their parameters, see
[Pump scenarios](../reference/pump_scenarios.md).
