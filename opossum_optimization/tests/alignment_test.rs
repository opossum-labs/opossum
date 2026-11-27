use approx::assert_abs_diff_eq;
use opossum_core::{
    analyzers::{AnalyzerType, RayTraceConfig},
    core_optics::{OpticNode, node_attr::NodePositioning},
    degree,
    distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
    error::OpmResult,
    joule, micrometer, millimeter, nanometer,
    nodes::{NodeGroup, SourcePort, SpotDiagram, ThinMirror},
    opm_document::{
        OpmDocument,
        optimization::{
            AlignmentAxis, BoxBounds, FreeVariable, MetricOperand, ObservableAddress, OperandMode,
            OptimizationAlgorithm, ParameterAddress, RotationAxis,
        },
    },
    prelude::{PointSrc, RayDataSource},
    utils::geom_transformation::Isometry,
};
use opossum_optimization::{MetricReader, OpticOptimizer};

#[test]
fn test_optimize_folding_mirror_alignment() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Folding mirror alignment setup");
    let src_id = scenery.add_node(SourcePort::new("source"))?;
    let mirror_id = scenery.add_node(ThinMirror::new("folding mirror"))?;

    // Place the detector at a fixed laboratory pose: (0, 100 mm, 100 mm), facing the -y direction
    // (local +z aligned with global +y via Pitch = -90 deg).
    let mut detector = SpotDiagram::new("fixed detector")?;
    detector.set_positioning(NodePositioning::Absolute(Isometry::new(
        millimeter!(0.0, 100.0, 100.0),
        degree!(-90.0, 0.0, 0.0),
    )?))?;
    let spot_id = scenery.add_node(detector)?;

    scenery.connect_nodes(src_id, "output_1", mirror_id, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(
        mirror_id,
        "output_1",
        spot_id,
        "input_1",
        millimeter!(100.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        src_id,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(5.0), 3)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(100.0),
        )?)
        .into(),
    );
    let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Nominal 90-degree deflection requires Pitch = 45 deg and Yaw = 0 deg.
    // Perturb both angles by +2 deg so the beam misses the detector center in both X and Y.
    let mut perturbed_iso = Isometry::identity();
    perturbed_iso.set_rotation_of_axis(RotationAxis::Pitch, degree!(47.0))?;
    perturbed_iso.set_rotation_of_axis(RotationAxis::Yaw, degree!(2.0))?;
    doc.scenery_mut()
        .with_node_attr_mut(mirror_id, |attr| attr.set_alignment(perturbed_iso))?;

    let pitch_addr = ParameterAddress::NodeAlignment {
        node_id: mirror_id,
        axis: AlignmentAxis::Rotation(RotationAxis::Pitch),
    };
    let yaw_addr = ParameterAddress::NodeAlignment {
        node_id: mirror_id,
        axis: AlignmentAxis::Rotation(RotationAxis::Yaw),
    };
    let centroid_x_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "centroid x".into(),
    };
    let centroid_y_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "centroid y".into(),
    };

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(pitch_addr.clone())
                .with_bounds(BoxBounds::new(35_f64.to_radians(), 55_f64.to_radians())?),
        )?
        .add_variable(
            FreeVariable::new(yaw_addr.clone())
                .with_bounds(BoxBounds::new(-10_f64.to_radians(), 10_f64.to_radians())?),
        )?
        .add_operand(MetricOperand::for_spot_radius(
            centroid_x_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .add_operand(MetricOperand::for_spot_radius(
            centroid_y_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_algorithm(OptimizationAlgorithm::dls().with_max_iterations(25))
        .optimize()?;

    let optimized_pitch = summary.value(&pitch_addr).unwrap();
    let optimized_yaw = summary.value(&yaw_addr).unwrap();
    let final_cx = MetricReader::read_metric(&doc, &centroid_x_addr)?;
    let final_cy = MetricReader::read_metric(&doc, &centroid_y_addr)?;

    assert_abs_diff_eq!(optimized_pitch, 45_f64.to_radians(), epsilon = 1e-5);
    assert_abs_diff_eq!(optimized_yaw, 0.0, epsilon = 1e-5);
    assert!(
        final_cx.abs() < 1e-6,
        "centroid x should be near 0, got {final_cx}"
    );
    assert!(
        final_cy.abs() < 1e-6,
        "centroid y should be near 0, got {final_cy}"
    );
    Ok(())
}
