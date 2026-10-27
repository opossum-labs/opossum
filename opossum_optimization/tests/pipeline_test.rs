use opossum_core::{
    apertures::CircleShape,
    core_optics::NodeAttrExt,
    distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
    error::OpmResult,
    geometry::body::OPTIC_SHAPE,
    joule, millimeter, nanometer,
    prelude::*,
};
use opossum_optimization::{
    BoxBounds, FreeVariable, MetricReader, ObservableAddress, OpticOptimizer, ParameterAddress,
    SchemaExtractor, TargetValueOperand,
};
use uom::si::f64::Length;
use uuid::Uuid;

/// Constructs a Keplerian optical system based on workshop_04_kepler_imaging_point.
///
/// Returns the initialized `OpmDocument` together with the UUID of the image spot diagram
/// and the lens2 ID for addressing the image distance connection.
fn build_kepler_setup(
    defocused_image_distance: Length,
) -> OpmResult<(OpmDocument, Uuid, Uuid, Uuid)> {
    let mut scenery = NodeGroup::new("Kepler optimization setup");

    // 1. Define point light source
    let i_src = scenery.add_node(SourcePort::new("point source"))?;

    // 2. Define wavelength-dependent refractive index for HZF52 glass
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.267_600_58,
        -2.053_845_66e-2,
        3.515_076_72e-2,
        7.701_513_48e-3,
        -9.081_398_17e-4,
        7.526_495_55e-5,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;

    // 3. Define first lens: 75 mm focal length, 10 mm thickness, 25 mm radius aperture
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    lens1.set_property(
        OPTIC_SHAPE,
        OpticShape::from(CircleShape::new(millimeter!(25.0))?).into(),
    )?;
    let i_pl1 = scenery.add_node(lens1)?;

    // 4. Define second lens: 50 mm focal length, 10 mm thickness, 25 mm radius aperture
    let mut lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    lens2.set_property(
        OPTIC_SHAPE,
        OpticShape::from(CircleShape::new(millimeter!(25.0))?).into(),
    )?;
    let i_pl2 = scenery.add_node(lens2)?;

    // 5. Define spot diagram detector at image position
    let i_sd_img = scenery.add_node(SpotDiagram::new("spot at image")?)?;

    // 6. Connect optical path: point source -> lens1 -> lens2 -> spot detector
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(
        i_pl2,
        "output_1",
        i_sd_img,
        "input_1",
        defocused_image_distance,
    )?;

    // 7. Configure point-source ray-tracing analysis
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(15.0), 8)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(70.0),
        )?)
        .into(),
    );

    // 8. Add analyzer to document
    let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(config));

    Ok((doc, analyzer_id, i_pl2, i_sd_img))
}

#[test]
fn test_schema_discovery_on_kepler_model() -> OpmResult<()> {
    // Build system with initial 45 mm image distance
    let (doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    // Step 1: Run schema extraction
    let schema = SchemaExtractor::extract(&doc)?;

    // Step 2: Verify controllable parameters (3 connection distances + lens properties)
    assert!(
        schema.controllable_variables.len() >= 3,
        "Expected at least 3 propagation distances in the model"
    );

    // Verify presence of the image plane distance
    let has_image_distance = schema.controllable_variables.iter().any(|var| {
        matches!(
            &var.address,
            ParameterAddress::ConnectionDistance { src_id, src_port, .. }
            if *src_id == lens2_id && src_port == "output_1"
        )
    });
    assert!(
        has_image_distance,
        "Connection distance lens2 -> detector must be recognized as controllable"
    );

    // Step 3: Verify observable detector metrics
    let spot_metrics: Vec<_> = schema
        .observable_metrics
        .iter()
        .filter(|m| m.address.analyzer_id == analyzer_id && m.address.node_id == spot_id)
        .collect();

    assert_eq!(
        spot_metrics.len(),
        4,
        "SpotDiagram must provide 4 scalar observables"
    );
    assert!(
        spot_metrics
            .iter()
            .any(|m| m.descriptor.name == "rms beam radius")
    );
    assert!(
        spot_metrics
            .iter()
            .any(|m| m.descriptor.name == "geo beam radius")
    );

    Ok(())
}

#[test]
fn test_optimize_kepler_focal_position_compact() -> OpmResult<()> {
    // 1. Defocused setup: 45 mm instead of ~58 mm
    let (mut doc, analyzer_id, lens2_id, spot_id) =
        build_kepler_setup(millimeter!(45.0))?;

    let dist_address = ParameterAddress::ConnectionDistance {
        src_id: lens2_id,
        src_port: "output_1".into(),
    };

    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // 2. High-level optimization in one concise block
    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_address.clone(), "image_drift_distance")
                .with_bounds(BoxBounds::new(0.030, 0.080)?),
        )?
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .optimize()?;

    // 3. Verification
    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!((best_distance - 0.058).abs() < 0.005);
    assert!(optimized_rms < 1.0e-3);

    eprintln!("\n=== COMPACT OPTIMIZATION RESULT ===");
    eprintln!("Iterations: {}", summary.iterations);
    eprintln!("Distance  : {:.3} mm", best_distance * 1e3);
    eprintln!("RMS Spot  : {:.2} µm", optimized_rms * 1e6);
    eprintln!(
        "Cost      : {:.4} -> {:.4} (-{:.1}%)",
        summary.initial_cost,
        summary.final_cost,
        summary.relative_improvement() * 100.0
    );
    eprintln!("===================================\n");

    Ok(())
}
