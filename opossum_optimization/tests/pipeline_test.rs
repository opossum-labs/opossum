use std::sync::Arc;

use argmin::core::Executor;
use argmin::solver::neldermead::NelderMead;

use opossum_core::{
    apertures::CircleShape,
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
    error::OpmResult,
    geometry::body::OPTIC_SHAPE,
    joule, millimeter, nanometer,
    prelude::*,
};
use opossum_optimization::{
    BoxBounds, FreeVariable, MeritFunction, MetricReader, ObservableAddress, Operand,
    ParameterAddress, ParameterApplier, SchemaExtractor, TargetValueOperand,
};
use uom::si::{f64::Length, length::meter};
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
fn test_optimize_kepler_focal_position() -> OpmResult<()> {
    // 1. Initial defocused state: 45 mm instead of ~58 mm
    let initial_dist_m = 0.045;
    let (mut doc, analyzer_id, lens2_id, spot_id) =
        build_kepler_setup(Length::new::<meter>(initial_dist_m))?;

    let root_group_id = doc.scenery().node_attr().uuid();

    // 2. Formulate free parameter: drift distance from lens2 to spot detector
    let dist_address = ParameterAddress::ConnectionDistance {
        parent_group_id: root_group_id,
        src_id: lens2_id,
        src_port: "output_1".into(),
    };

    let free_var = FreeVariable::new(dist_address.clone(), "image_drift_distance", initial_dist_m)
        .with_bounds(BoxBounds::new(0.030, 0.080)?); // 30 mm to 80 mm

    // 3. Formulate operand: target RMS beam radius = 0.0 m
    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    let operand = TargetValueOperand::new(
        "minimize_rms_spot",
        spot_observable_addr.clone(),
        0.0,    // Target: 0 µm spot
        1.0,    // Weight
        1.0e-3, // Scale: 1 mm
    );

    let free_vars = vec![free_var];
    let couplings = vec![];
    let operands: Vec<Arc<dyn Operand>> = vec![Arc::new(operand)];

    // 4. Create MeritFunction
    let merit = MeritFunction::new(doc.clone(), free_vars, couplings, operands)?;

    // Measure initial defocused cost
    let initial_cost = merit.evaluate(&[initial_dist_m]);
    assert!(
        initial_cost > 0.0,
        "Initial defocused configuration must yield non-zero cost"
    );
    // 5. Optimize using Nelder-Mead simplex solver
    // Initial simplex around 45 mm and 50 mm
    let initial_simplex = vec![vec![0.045], vec![0.050]];
    let solver = NelderMead::new(initial_simplex)
        .with_sd_tolerance(1e-6)
        .map_err(|e| OpossumError::Analysis(format!("optimization failed: {e}")))?;

    let res = Executor::new(merit, solver)
        .configure(|state| state.max_iters(40).target_cost(1e-8))
        .run();

    assert!(res.is_ok(), "Nelder-Mead solver execution failed");
    let result = res.unwrap();

    let best_distance = result.state().best_param.as_ref().unwrap()[0];
    let final_cost = result.state().best_cost;

    // The optimal image distance for this Kepler configuration is close to 58 mm
    assert!(
        (best_distance - 0.058).abs() < 0.005,
        "Optimal distance should be near 58 mm (0.058 m), got: {:.4} m",
        best_distance
    );
    assert!(
        final_cost < initial_cost,
        "Optimization must significantly improve initial cost: initial={}, final={}",
        initial_cost,
        final_cost
    );

    // 6. Apply optimized parameter back into the primary document
    ParameterApplier::set_value(&mut doc, &dist_address, best_distance)?;
    ParameterApplier::finalize_update(&mut doc)?;

    // 7. Verify simulation on modified model
    doc.analyze_headless()?;

    // Read initial and optimized metrics for comparison
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    eprintln!("\n================ OPTIMIZATION RESULT ================");
    eprintln!("Initial image distance : {:8.3} mm", initial_dist_m * 1e3);
    eprintln!("Optimized image distance: {:8.3} mm", best_distance * 1e3);
    eprintln!("Initial merit cost      : {:8.4}", initial_cost);
    eprintln!("Final merit cost        : {:8.4}", final_cost);
    eprintln!("Final RMS spot radius   : {:8.2} µm ({:.6} mm)", optimized_rms * 1e6, optimized_rms * 1e3);
    eprintln!("=====================================================\n");
  
    // Verify directly with MetricReader that the spot radius decreased
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;
    assert!(
        optimized_rms < 1.0e-3,
        "Optimized RMS spot radius must be under 1 mm, got: {:.6} m",
        optimized_rms
    );

    Ok(())
}
