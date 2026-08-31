//! Single-pass small-signal amplifier with a non-homogeneous pump profile.
//!
//! This example is the natural complement to `amplifier_chain.rs`. Where that example assigns a
//! constant gain factor to an amplifier head, this one pumps it with a shaped profile: a
//! Super-Gaussian transversal spot (order 2, 1/e² half-width 5 mm) and a Beer-Lambert longitudinal
//! decay (α = 0.5 cm⁻¹, forward pumping). The consequence is that rays at different radial
//! positions leave the medium with different gain factors — something a `ConstGain` model cannot
//! express.
//!
//! # What to look for
//!
//! Load the resulting `.opm` file in the GUI and run the ray-tracing analysis in the
//! `shaped pump` scenario. The spot diagram at the output shows per-ray energy: rays near the
//! optical axis — where the pump spot is strongest — carry significantly more energy than rays
//! near the aperture edge.
//!
//! Run with
//!
//! ```bash
//! cargo run -p opossum_core --example inhomogeneous_small_signal
//! ```
use opossum_core::{
    gain::{
        AnalyticPump, BeerLambertProfile, GainModel, LongitudinalProfile, PumpDirection,
        PumpSource, SmallSignalGain, TransversalProfile,
    },
    prelude::*,
    reciprocal_centimeter, square_centimeter,
    utils::super_gaussian::SuperGaussianShape,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Inhomogeneous small-signal amplifier");

    // Hardware: a plane-parallel glass slab acting as the active medium.
    // Flat surfaces (infinite radii), 40 mm thick, refractive index 1.5.
    let source = scenery.add_node(SourcePort::new("oscillator"))?;
    let head = scenery.add_node(Lens::new(
        "amplifier head",
        millimeter!(f64::INFINITY),
        millimeter!(f64::INFINITY),
        millimeter!(40.0),
        RefrIndexConst::new(1.5)?,
    )?)?;
    let diagram = scenery.add_node(SpotDiagram::new("output spot")?)?;

    scenery.connect_nodes(source, "output_1", head, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(head, "output_1", diagram, "input_1", millimeter!(50.0))?;

    let mut document = OpmDocument::new(scenery);
    document.set_is_amplifier_node(head, true);

    // Pump scenario: Super-Gaussian transversal profile combined with Beer-Lambert longitudinal
    // decay. g₀ = 0.3 cm⁻¹ at the peak (axis, near face). The profile falls off as the pump
    // spot intensity drops with radius and as the pump beam is absorbed on its way through the medium.
    let scenario_id = document.add_pump_scenario("shaped pump");
    let scenario = document
        .pump_scenario_mut(scenario_id)
        .expect("the scenario just added must be there");

    scenario.set_gain_model(
        head,
        GainModel::SmallSignalGain(SmallSignalGain::new(
            square_centimeter!(2.0e-20), // σ_e ≈ 2 × 10⁻²⁰ cm², a typical solid-state medium
            (64, 64, 32),                // 64 × 64 transversal, 32 longitudinal cells
        )?),
    );
    scenario.set_pump_source(
        head,
        PumpSource::Analytic(AnalyticPump::new(
            reciprocal_centimeter!(0.3),
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                millimeter!(0.0, 0.0), // pump centered on the optical axis
                millimeter!(5.0, 5.0), // 1/e² half-width 5 mm along x and y
                2.0,                   // order 2: flatter top than a Gaussian, steeper edges
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                reciprocal_centimeter!(0.5), // absorption coefficient of the pump in the medium
                PumpDirection::Forward,      // pump enters from the input surface
            )?),
        )?),
    );

    // Ray-tracing analysis: SmallSignalGain integrates gain along each ray's actual path through
    // the medium, so only a ray trace can reveal the spatially varying gain across the beam.
    let mut config = RayTraceConfig::default();
    config.map_source(
        source,
        // Five concentric rings of rays up to a radius of 9 mm.
        // Inner rings cross the pump spot peak; outer rings see progressively weaker pumping.
        round_collimated_ray_builder(millimeter!(9.0), joule!(1.0), 5)?,
    );
    let analyzer_id = document.add_analyzer(AnalyzerType::RayTrace(config));
    let scenario_ids = document.pump_scenarios().keys().copied().collect();
    document
        .analyzer_mut(analyzer_id)
        .expect("the analyzer just added must be there")
        .set_pump_scenarios(scenario_ids);

    document.save_to_file(Path::new(
        "./opossum_core/playground/inhomogeneous_small_signal.opm",
    ))
}
