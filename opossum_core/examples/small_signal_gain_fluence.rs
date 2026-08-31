//! Single-pass amplifier: how a Gaussian pump profile imprints onto the output fluence.
//!
//! An input beam with a flat-top energy distribution enters a pumped glass slab. The pump is a
//! Gaussian spot strongest on the optical axis and falling off radially; the longitudinal profile
//! is uniform (flat) along the axis. Because the unsaturated small-signal model integrates
//! `G = exp(∫ g₀·β ds)` along each ray's actual path, axial rays pick up more gain than edge
//! rays — and the output fluence map at the detector traces the pump shape directly.
//!
//! # What to look for
//!
//! Load the resulting `.opm` file in the GUI and run the ray-tracing analysis in the
//! `Gaussian pump` scenario. The fluence detector shows a bright central spot that falls off
//! radially, even though the input beam was nearly uniform — the pump profile has been imprinted
//! onto the beam. The peak on-axis single-pass gain follows from the standard formula:
//!
//! ```text
//! G = exp(g₀ · L) = exp(0.2 cm⁻¹ × 10 cm) = exp(2) ≈ 7.4
//! ```
//!
//! Run with
//!
//! ```bash
//! cargo run -p opossum_core --example small_signal_gain_fluence
//! ```
use opossum_core::{
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    distributions::{energy::General2DGaussian, position::FibonacciEllipse, spectral::LaserLines},
    gain::{
        AnalyticPump, GainModel, LongitudinalProfile, MonochromaticSmallSignalGain, PumpSource,
        TransversalProfile,
    },
    prelude::*,
    reciprocal_centimeter,
    utils::super_gaussian::SuperGaussianShape,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Small-signal gain: pump profile imprinted on fluence");

    // Hardware: a plane-parallel glass slab (flat surfaces, 100 mm thick, n = 1.5).
    // Nothing about the lens node says "amplifier" — that is decided below in the pump scenario.
    let source = scenery.add_node(SourcePort::new("oscillator"))?;
    let head = scenery.add_node(Lens::new(
        "amplifier head",
        millimeter!(f64::INFINITY), // flat input surface
        millimeter!(f64::INFINITY), // flat output surface
        millimeter!(100.0),
        RefrIndexConst::new(1.5)?,
    )?)?;
    let mut detector = FluenceDetector::new("output fluence");
    detector.set_property("fluence estimator", FluenceEstimator::Voronoi.into())?;
    let detector_id = scenery.add_node(detector)?;

    scenery.connect_nodes(source, "output_1", head, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(head, "output_1", detector_id, "input_1", millimeter!(100.0))?;

    let mut document = OpmDocument::new(scenery);
    // Marks the node in the GUI's pump-scenario editor; the analysis reads the gain model below.
    document.set_is_amplifier_node(head, true);

    // Pump scenario: Gaussian transversal profile (σ = 5 mm), flat along the optical axis.
    // g₀ = 0.2 cm⁻¹ at the axis where the pump is strongest. The flat longitudinal profile
    // means the inversion is the same on the entrance face as on the exit face, so the only
    // spatial variation across the aperture comes from the transversal Gaussian spot.
    let scenario_id = document.add_pump_scenario("Gaussian pump");
    let scenario = document
        .pump_scenario_mut(scenario_id)
        .expect("the scenario just added must be there");

    scenario.set_gain_model(
        head,
        GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
            reciprocal_centimeter!(0.2), // g₀ at the axis; G_max = exp(0.2 × 10) ≈ 7.4
        )?),
    );
    scenario.set_pump_source(
        head,
        PumpSource::Analytic(AnalyticPump::new(
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                millimeter!(0.0, 0.0), // pump centered on the optical axis
                millimeter!(5.0, 5.0), // 1/e² half-width 5 mm along x and y
                1.0,                   // order 1 = standard Gaussian
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,  // uniform inversion from entrance to exit face
            (64, 64, 16),               // 64 × 64 transversal cells; 16 longitudinal
        )?),
    );

    // Ray-tracing analysis: 10 000 rays on a Fibonacci grid so the aperture is sampled uniformly.
    // The energy distribution is a high-order Super-Gaussian (power 8 ≈ flat-top): each ray
    // starts with roughly the same energy, so any fluence variation at the detector is entirely
    // due to gain — not to the input beam shape.
    let mut config = RayTraceConfig::default();
    config.map_source(
        source,
        RayDataSource::Collimated(CollimatedSrc::new(
            FibonacciEllipse::new(millimeter!(10.0), millimeter!(10.0), 10_000)?.into(),
            General2DGaussian::new(
                joule!(0.1),
                millimeter!(0.0, 0.0), // centered on axis
                millimeter!(5.0, 5.0), // sigma — matches the pump spot width
                8.0,                   // high order: nearly flat-top within the aperture
                degree!(0.0),
                false,
            )?
            .into(),
            LaserLines::new(vec![(nanometer!(1054.0), 1.0)])?.into(),
        ))
        .into(),
    );
    let analyzer_id = document.add_analyzer(AnalyzerType::RayTrace(config));
    let scenario_ids = document.pump_scenarios().keys().copied().collect();
    document
        .analyzer_mut(analyzer_id)
        .expect("the analyzer just added must be there")
        .set_pump_scenarios(scenario_ids);

    document.save_to_file(Path::new(
        "./opossum_core/playground/small_signal_gain_fluence.opm",
    ))
}
