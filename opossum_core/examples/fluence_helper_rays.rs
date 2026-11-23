//! Fluence from helper rays, case by case.
//!
//! Seven independent beam paths in one model, each from its own source to a fluence detector using
//! the helper-ray estimator. Every source sends the same collimated Gaussian beam with helper rays
//! (1 J, σ = 2 mm), whose center ray carries the peak fluence F0 = E / (2π σ²). On every path the
//! peak fluence the detector should see follows from geometry alone:
//!
//! | path | detector sees | why |
//! |---|---|---|
//! | A | F0 | straight to the detector |
//! | B | F0 / 2 | the detector is tilted by 60°, the beam spreads over twice the area |
//! | C | 4 F0 | 250 mm behind a lens of f = 500 mm, the beam is half as wide |
//! | D | F0 | behind a 45° mirror, the reflected helper rays start on the mirror |
//! | E | F0 / 2 | behind a filter transmitting 50 % |
//! | F | F0 | totally reflected inside a 45° prism (its side face is not traced) |
//! | G | 4 F0 | like C, but the edge of the lens lies 0.3 µm outside the ring of rays at 4.5 mm |
//!
//! On path G every ray of that ring has a helper ray beyond the edge: its triangle tears, so the
//! ray drops its helper rays and records no fluence, and the log warns about it. The fluence map
//! of G therefore ends at the fifth ring (3.75 mm at the lens), while that of C reaches the
//! eighth (6 mm).
//!
//! The example saves the model and prints the expected next to the computed peak fluence. Run it
//! from the workspace root, then the CLI for the report (which also shows the warnings):
//!
//! ```text
//! cargo run -p opossum_core --example fluence_helper_rays
//! cargo run -p opossum_cli -- -f ./opossum_core/playground/fluence_helper_rays.opm
//! ```
use opossum_core::{
    apertures::{CircleShape, RectangleShape},
    core_optics::{NodeAttrExt, hit_map::fluence_estimator::FluenceEstimator},
    distributions::{fluence::general_gaussian::General2DGaussian, position::Hexapolar},
    geometry::body::OPTIC_SHAPE,
    light::Rays,
    nodes::fluence_detector::Fluence,
    prelude::*,
    reporting::analysis_report::AnalysisReport,
};
use std::{f64::consts::PI, iter::once, path::Path};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

/// The standard deviation of the Gaussian beam, in millimeters.
const SIGMA_MM: f64 = 2.0;

/// One beam path: the name of its detector and the peak fluence expected there, in units of F0.
struct Case {
    detector: String,
    factor: f64,
}

/// The collimated Gaussian beam with helper rays every path starts with: 1 J, rays out to 3σ in
/// eight rings of 0.75 mm.
fn beam() -> OpmResult<Rays> {
    Rays::new_collimated_w_fluence_helper(
        nanometer!(1000.0),
        &General2DGaussian::new(
            joule!(1.0),
            millimeter!(0.0, 0.0),
            millimeter!(SIGMA_MM, SIGMA_MM),
            radian!(0.0),
        )?,
        &Hexapolar::new(millimeter!(3.0 * SIGMA_MM), 8)?,
    )
}

/// A fluence detector using the helper-ray estimator, with a window of 50 mm square.
///
/// # Errors
///
/// This function returns an error if a property cannot be set.
fn detector(name: &str) -> OpmResult<FluenceDetector> {
    let mut detector = FluenceDetector::new(name);
    detector.set_property("fluence estimator", FluenceEstimator::HelperRays.into())?;
    detector.set_property(
        OPTIC_SHAPE,
        OpticShape::from(RectangleShape::new(millimeter!(50.0), millimeter!(50.0))?).into(),
    )?;
    Ok(detector)
}

/// Add a beam path to the scenery: a source sending the beam, the given optics and the detector,
/// each connected from its `output_1` to the `input_1` of the next at the given distances.
///
/// # Arguments
///
/// * `optics` - the nodes between source and detector, already in the scenery.
/// * `distances` - the distances along the path in millimeters, one per connection.
/// * `factor` - the peak fluence expected at the detector, in units of F0.
///
/// # Errors
///
/// This function returns an error if the beam cannot be built or a node cannot be added or
/// connected.
fn path(
    scenery: &mut NodeGroup,
    config: &mut RayTraceConfig,
    optics: &[Uuid],
    distances: &[f64],
    detector: FluenceDetector,
    factor: f64,
) -> OpmResult<Case> {
    let name = detector.name().to_owned();
    let source = scenery.add_node(SourcePort::new(&format!("source of {name}")))?;
    config.map_source(source, RayDataSource::Raw(beam()?).into());
    let detector = scenery.add_node(detector)?;
    let nodes: Vec<Uuid> = once(source)
        .chain(optics.iter().copied())
        .chain(once(detector))
        .collect();
    for (pair, distance) in nodes.windows(2).zip(distances) {
        scenery.connect_nodes(
            pair[0],
            "output_1",
            pair[1],
            "input_1",
            millimeter!(*distance),
        )?;
    }
    Ok(Case {
        detector: name,
        factor,
    })
}

/// Add the seven beam paths to the scenery.
///
/// # Errors
///
/// This function returns an error if a node cannot be built, added or connected.
fn add_paths(scenery: &mut NodeGroup, config: &mut RayTraceConfig) -> OpmResult<Vec<Case>> {
    let lens = scenery.add_node(ParaxialSurface::new("C lens", millimeter!(500.0))?)?;
    let mirror =
        scenery.add_node(ThinMirror::new("D mirror").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let filter = scenery.add_node(IdealFilter::new(
        "E filter 50 %",
        &FilterTypeBuilder::try_from(0.5)?,
    )?)?;
    // Entering at 0°, the beam meets the rear face at 45°, beyond the critical angle of 41.8°.
    let prism = scenery.add_node(Wedge::new(
        "F prism",
        millimeter!(20.0),
        degree!(45.0),
        RefrIndexConst::new(1.5)?,
    )?)?;
    // The sixth ring of rays lies at 4.5 mm, 0.3 µm inside the edge; the rings beyond miss it.
    let mut clipping_lens = ParaxialSurface::new("G lens, edge at 4.5003 mm", millimeter!(500.0))?;
    clipping_lens.set_property(
        OPTIC_SHAPE,
        OpticShape::from(CircleShape::new(millimeter!(4.5003))?).into(),
    )?;
    let clipping_lens = scenery.add_node(clipping_lens)?;
    let tilted = detector("B tilted by 60°")?.with_tilt(degree!(60.0, 0.0, 0.0))?;
    Ok(vec![
        path(scenery, config, &[], &[100.0], detector("A straight")?, 1.0)?,
        path(scenery, config, &[], &[100.0], tilted, 0.5)?,
        path(
            scenery,
            config,
            &[lens],
            &[100.0, 250.0],
            detector("C 250 mm behind the lens")?,
            4.0,
        )?,
        path(
            scenery,
            config,
            &[mirror],
            &[100.0, 100.0],
            detector("D behind the 45° mirror")?,
            1.0,
        )?,
        path(
            scenery,
            config,
            &[filter],
            &[100.0, 100.0],
            detector("E behind the filter")?,
            0.5,
        )?,
        path(
            scenery,
            config,
            &[prism],
            &[100.0, 50.0],
            detector("F after total reflection")?,
            1.0,
        )?,
        path(
            scenery,
            config,
            &[clipping_lens],
            &[100.0, 250.0],
            detector("G 250 mm behind the clipping lens")?,
            4.0,
        )?,
    ])
}

/// Return the peak fluence a report gives for the detector with the given name.
fn peak_fluence(reports: &[AnalysisReport], detector: &str) -> Option<Fluence> {
    let report = reports
        .iter()
        .flat_map(AnalysisReport::node_reports)
        .find(|report| report.name() == detector)?;
    match report.properties().get("Peak Fluence").ok()? {
        Proptype::Fluence(fluence) => Some(*fluence),
        _ => None,
    }
}

/// Print the expected next to the computed peak fluence of every path.
fn print_comparison(reports: &[AnalysisReport], cases: &[Case]) {
    let f0 = (joule!(1.0) / (2.0 * PI * millimeter!(SIGMA_MM) * millimeter!(SIGMA_MM)))
        .get::<joule_per_square_centimeter>();
    println!("F0 = {f0:.4} J/cm² (peak fluence of the beam at the source)\n");
    println!(
        "{:<36} {:>17} {:>17} {:>7}",
        "detector", "expected [J/cm²]", "computed [J/cm²]", "ratio"
    );
    for case in cases {
        let expected = case.factor * f0;
        match peak_fluence(reports, &case.detector) {
            Some(peak) => {
                let computed = peak.get::<joule_per_square_centimeter>();
                let ratio = computed / expected;
                let verdict = if (ratio - 1.0).abs() < 1e-3 {
                    "ok"
                } else {
                    "CHECK"
                };
                println!(
                    "{:<36} {expected:>17.4} {computed:>17.4} {ratio:>7.4}  {verdict}",
                    case.detector
                );
            }
            None => println!(
                "{:<36} {expected:>17.4} {:>17} {:>7}  CHECK",
                case.detector, "none", "-"
            ),
        }
    }
}

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Fluence from helper rays");
    let mut config = RayTraceConfig::default();
    let cases = add_paths(&mut scenery, &mut config)?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/fluence_helper_rays.opm",
    ))?;
    print_comparison(&doc.analyze()?, &cases);
    Ok(())
}
