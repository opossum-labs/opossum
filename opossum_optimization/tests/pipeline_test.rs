use std::sync::Arc;

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
    BoxBounds, FreeVariable, MaxTotalLengthOperand, MeritFunction, MetricReader, ObservableAddress, OpticOptimizer, OptimizationAlgorithm, ParameterAddress, SchemaExtractor, TargetValueOperand,
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
            Hexapolar::new(millimeter!(15.0), 5)?.into(),
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
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

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
use opossum_optimization::ParameterCoupling;

#[test]
fn test_optimize_kepler_lens_curvature_coupled() -> OpmResult<()> {
    // 1. Defokussiertes Setup mit FESTEM Detektorabstand von 45 mm (statt ~58 mm)
    let fixed_image_distance = millimeter!(45.0);
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(fixed_image_distance)?;

    // Adressen der beiden Linsenflächen (Krümmungsradien)
    let front_curvature_addr = ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "front curvature".into(),
    };
    let rear_curvature_addr = ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "rear curvature".into(),
    };

    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // 2. High-Level-Optimierung mit Parameterkopplung R_rear = -R_front
    let summary = OpticOptimizer::new(&mut doc)
        // Freie Variable: Vorderflächen-Radius (Startwert ~100 mm aus dem Modell)
        .add_variable(
            FreeVariable::new(front_curvature_addr.clone(), "lens2_front_curvature")
                .with_bounds(BoxBounds::new(0.030, 0.150)?), // 30 mm bis 150 mm
        )?
        // Kopplung: R_rear = -R_front (symmetrische Bikonvexlinse)
        .add_coupling(ParameterCoupling::opposite(
            rear_curvature_addr.clone(),
            front_curvature_addr.clone(),
        ))
        // Zielkriterium: RMS Spotradius minimieren
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .optimize()?;

    // 3. Verifikation
    let best_r_front = summary.value(&front_curvature_addr).unwrap();
    let best_r_rear = summary.value(&rear_curvature_addr).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    // Prüfung der Kopplung: R2 muss exakt -R1 sein
    assert!(
        (best_r_rear - (-best_r_front)).abs() < 1e-12,
        "Coupling violated: R_rear ({best_r_rear}) must equal -R_front (-{best_r_front})"
    );

    // Da der Abstand mit 45 mm kürzer ist als 58 mm, muss die Linse stärker brechen.
    // Ein kleinerer Radius bedeutet stärkere Krümmung / kürzere Brennweite (ca. 70-85 mm statt 100 mm).
    assert!(
        best_r_front < 0.100,
        "Front curvature radius should be smaller than initial 100 mm for shorter focal length: got {:.3} mm",
        best_r_front * 1e3
    );

    // Der Spotradius muss signifikant fokussiert worden sein
    assert!(
        optimized_rms < 1.0e-3,
        "Optimized RMS spot radius must be under 1 mm, got: {:.6} m",
        optimized_rms
    );

    eprintln!("\n=== COUPLED CURVATURE OPTIMIZATION RESULT ===");
    eprintln!("Iterations        : {}", summary.iterations);
    eprintln!("Initial Cost      : {:.4}", summary.initial_cost);
    eprintln!(
        "Final Cost        : {:.4} (-{:.1}%)",
        summary.final_cost,
        summary.relative_improvement() * 100.0
    );
    eprintln!("Optimized R_front : {:8.3} mm", best_r_front * 1e3);
    eprintln!(
        "Optimized R_rear  : {:8.3} mm (R2 = -R1)",
        best_r_rear * 1e3
    );
    eprintln!("Final RMS Spot    : {:8.2} µm", optimized_rms * 1e6);
    eprintln!("==============================================\n");

    Ok(())
}
#[test]
fn test_optimize_kepler_best_form_lens() -> OpmResult<()> {
    // 1. Setup with a fixed image distance of 45 mm
    let fixed_image_distance = millimeter!(45.0);
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(fixed_image_distance)?;

    // Addresses for front and rear surfaces of lens2
    let front_curvature_addr = ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "front curvature".into(),
    };
    let rear_curvature_addr = ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "rear curvature".into(),
    };

    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // 2. High-level 2D optimization (both curvatures as independent free variables)
    let summary = OpticOptimizer::new(&mut doc)
        // Free variable 1: Front curvature (initial value ~100 mm from model)
        .add_variable(
            FreeVariable::new(front_curvature_addr.clone(), "lens2_front_curvature")
                .with_bounds(BoxBounds::new(0.025, 0.250)?), // 25 mm to 250 mm
        )?
        // Free variable 2: Rear curvature (initial value ~ -100 mm from model)
        .add_variable(
            FreeVariable::new(rear_curvature_addr.clone(), "lens2_rear_curvature")
                .with_bounds(BoxBounds::new(-0.250, -0.025)?), // -250 mm to -25 mm
        )?
        // Target: Minimize RMS spot radius
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .with_algorithm(
            OptimizationAlgorithm::nelder_mead()
                .with_max_iterations(150)
                .with_sd_tolerance(1e-7),
        )
        .optimize()?;

    // 3. Verification & Metrics
    let best_r_front = summary.value(&front_curvature_addr).unwrap();
    let best_r_rear = summary.value(&rear_curvature_addr).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    // Calculate Coddington shape factor q = (R2 + R1) / (R2 - R1)
    let shape_factor_q = (best_r_rear + best_r_front) / (best_r_rear - best_r_front);

    assert!(
        optimized_rms < 1.0e-3,
        "Optimized RMS spot radius must be under 1 mm, got: {:.6} m",
        optimized_rms
    );
    assert!(
        summary.final_cost < summary.initial_cost,
        "Cost must improve: initial={}, final={}",
        summary.initial_cost,
        summary.final_cost
    );

    eprintln!("\n=== BEST-FORM LENS OPTIMIZATION RESULT ===");
    eprintln!("Iterations        : {}", summary.iterations);
    eprintln!("Initial Cost      : {:.4}", summary.initial_cost);
    eprintln!(
        "Final Cost        : {:.4} (-{:.1}%)",
        summary.final_cost,
        summary.relative_improvement() * 100.0
    );
    eprintln!("Optimized R_front : {:8.3} mm", best_r_front * 1e3);
    eprintln!("Optimized R_rear  : {:8.3} mm", best_r_rear * 1e3);
    eprintln!("Coddington Factor : {:8.3} (q)", shape_factor_q);
    eprintln!("Final RMS Spot    : {:8.2} µm", optimized_rms * 1e6);
    eprintln!("==========================================\n");

    Ok(())
}

#[test]
fn test_optimize_kepler_with_geometric_length_penalty() -> OpmResult<()> {
    // 1. Setup: Source -> Lens1 (70 mm) -> Lens2 (125 mm) -> Spot (initial 45 mm)
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    // Adressen der beiden Abstände (Linse1 -> Linse2) und (Linse2 -> Spot)
    // lens1_id ermitteln
    let all_nodes = doc.scenery().collect_all_nodes_recursive()?;
    let lens1_node = all_nodes
        .iter()
        .find(|n| n.name() == "75 mm lens")
        .expect("Lens1 must exist");
    let lens1_id = lens1_node.uuid();

    let dist_lens1_to_lens2 = ParameterAddress::ConnectionDistance {
        src_id: lens1_id,
        src_port: "output_1".into(),
    };
    let dist_lens2_to_spot = ParameterAddress::ConnectionDistance {
        src_id: lens2_id,
        src_port: "output_1".into(),
    };

    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // 2. Optimierung mit zwei freien Abständen und einer Längenbegrenzung
    // Ziel: Fokussieren, aber die Summe (d12 + d2_spot) darf maximal 190 mm betragen
    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_lens1_to_lens2.clone(), "distance_lens1_lens2")
                .with_bounds(BoxBounds::new(0.100, 0.150)?),
        )?
        .add_variable(
            FreeVariable::new(dist_lens2_to_spot.clone(), "distance_lens2_spot")
                .with_bounds(BoxBounds::new(0.030, 0.080)?),
        )?
        // Operanden 1: RMS Spot minimieren
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        // Operanden 2: Geometrische Nebenbedingung (Summe beider Abstände <= 185 mm)
        .add_operand(MaxTotalLengthOperand::new(
            "compact_length_limit",
            vec![dist_lens1_to_lens2.clone(), dist_lens2_to_spot.clone()],
            0.185, // max 185 mm Gesamtlänge der Teilstrecken
            50.0,  // Starke Strafe bei Überschreitung
        ))
        .optimize()?;

    let d1 = summary.value(&dist_lens1_to_lens2).unwrap();
    let d2 = summary.value(&dist_lens2_to_spot).unwrap();
    let total_length = d1 + d2;
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    eprintln!("\n=== GEOMETRIC PENALTY OPTIMIZATION RESULT ===");
    eprintln!("Iterations        : {}", summary.iterations);
    eprintln!("Distance L1 -> L2 : {:8.3} mm", d1 * 1e3);
    eprintln!("Distance L2 -> Det: {:8.3} mm", d2 * 1e3);
    eprintln!(
        "Total Length      : {:8.3} mm (Limit: 185.0 mm)",
        total_length * 1e3
    );
    eprintln!("Final RMS Spot    : {:8.2} µm", optimized_rms * 1e6);
    eprintln!("=============================================\n");

    // Sicherstellen, dass das Limit von 185 mm (mit geringer numerischer Toleranz) eingehalten wurde
    assert!(
        total_length <= 0.186,
        "Total length ({:.3} mm) should respect the 185 mm constraint",
        total_length * 1e3
    );

    Ok(())
}
#[test]
fn test_merit_gradient_calculation() -> OpmResult<()> {
    let (doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    let dist_addr = ParameterAddress::ConnectionDistance {
        src_id: lens2_id,
        src_port: "output_1".into(),
    };
    let spot_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    let free_var = FreeVariable::new(dist_addr, "image_distance");
    let operand = TargetValueOperand::new("spot", spot_addr, 0.0, 1.0, 1e-3);

    let merit = MeritFunction::new(
        doc,
        vec![free_var],
        vec![],
        vec![Arc::new(operand)],
    )?;

    // 1. In defokussierter Lage (45 mm): Steigung dPhi/dd muss negativ sein (Kosten sinken bei Vergrößerung)
    let grad_45 = merit.compute_gradient(&[0.045]);
    assert!(
        grad_45[0] < -50.0,
        "Gradient at 45 mm should be strongly negative, got: {}",
        grad_45[0]
    );

    // 2. Nahe dem Fokus (58 mm): Steigung dPhi/dd muss nahe Null sein (lokales Minimum)
    let grad_58 = merit.compute_gradient(&[0.058]);
    assert!(
        grad_58[0].abs() < grad_45[0].abs() * 0.1,
        "Gradient at focus (58 mm) must be much smaller than at 45 mm, got: {}",
        grad_58[0]
    );

    eprintln!("\n=== GRADIENT VERIFICATION ===");
    eprintln!("Gradient at 45 mm: {:10.2}", grad_45[0]);
    eprintln!("Gradient at 58 mm: {:10.2}", grad_58[0]);
    eprintln!("=============================\n");

    Ok(())
}
#[test]
fn test_optimize_kepler_focal_position_lbfgs() -> OpmResult<()> {
    // 1. Defokussiertes Setup: Start bei 45 mm statt ~58 mm
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

    // 2. Optimierung mit L-BFGS (Quasi-Newton-Verfahren mit finiten Differenzen)
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
        .with_algorithm(
            OptimizationAlgorithm::lbfgs()
                .with_max_iterations(25)
        )
        .optimize()?;

    // 3. Verifikation
    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!(
        (best_distance - 0.058).abs() < 0.005,
        "Optimal distance should be near 58 mm, got: {:.4} m",
        best_distance
    );
    assert!(
        optimized_rms < 1.0e-3,
        "Optimized RMS spot radius must be under 1 mm, got: {:.6} m",
        optimized_rms
    );
    assert!(
        summary.final_cost < summary.initial_cost,
        "Cost must improve: initial={}, final={}",
        summary.initial_cost,
        summary.final_cost
    );

    eprintln!("\n=== L-BFGS OPTIMIZATION RESULT ===");
    eprintln!("Iterations        : {}", summary.iterations);
    eprintln!("Initial Cost      : {:.4}", summary.initial_cost);
    eprintln!(
        "Final Cost        : {:.4} (-{:.1}%)",
        summary.final_cost,
        summary.relative_improvement() * 100.0
    );
    eprintln!("Optimized Distance: {:8.3} mm", best_distance * 1e3);
    eprintln!("Final RMS Spot    : {:8.2} µm", optimized_rms * 1e6);
    eprintln!("==================================\n");

    Ok(())
}
#[test]
fn test_optimize_kepler_focal_position_simulated_annealing() -> OpmResult<()> {
    // 1. Weit defokussierter Startzustand: 35 mm (Ziel ist ~58 mm)
    let (mut doc, analyzer_id, lens2_id, spot_id) =
        build_kepler_setup(millimeter!(35.0))?;

    let dist_address = ParameterAddress::ConnectionDistance {
        src_id: lens2_id,
        src_port: "output_1".into(),
    };

    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // 2. Globaler Suchlauf mit Simulated Annealing
    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_address.clone(), "image_drift_distance")
                .with_bounds(BoxBounds::new(0.030, 0.080)?), // Suchraum 30 mm bis 80 mm
        )?
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .with_algorithm(
            OptimizationAlgorithm::simulated_annealing()
                .with_temperature(20.0),
        )
        .optimize()?;

    // 3. Verifikation
    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    // SA findet das globale Minimum um 58 mm
    assert!(
        (best_distance - 0.058).abs() < 0.006,
        "Optimal distance should be near 58 mm, got: {:.4} m",
        best_distance
    );
    assert!(
        optimized_rms < 1.0e-3,
        "Optimized RMS spot radius must be under 1 mm, got: {:.6} m",
        optimized_rms
    );

    eprintln!("\n=== SIMULATED ANNEALING RESULT ===");
    eprintln!("Iterations        : {}", summary.iterations);
    eprintln!("Initial Cost      : {:.4}", summary.initial_cost);
    eprintln!(
        "Final Cost        : {:.4} (-{:.1}%)",
        summary.final_cost,
        summary.relative_improvement() * 100.0
    );
    eprintln!("Optimized Distance: {:8.3} mm", best_distance * 1e3);
    eprintln!("Final RMS Spot    : {:8.2} µm", optimized_rms * 1e6);
    eprintln!("==================================\n");

    Ok(())
}