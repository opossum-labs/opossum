mod common;

use common::build_kepler_setup;
use opossum_core::{core_optics::NodeAttrExt, error::OpmResult, millimeter};
use opossum_optimization::{
    BoxBounds, EvaluationContext, FreeVariable, MaxTotalLengthOperand, MeritFunction, MetricReader,
    MinEdgeThicknessOperand, ObservableAddress, Operand, OpticOptimizer, OptimizationAlgorithm,
    ParameterAddress, TargetValueOperand, TransmissionPenaltyOperand,
};
use std::sync::Arc;
use uom::si::length::millimeter;

#[test]
fn test_optimize_kepler_with_geometric_length_penalty() -> OpmResult<()> {
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    let all_nodes = doc.scenery().collect_all_nodes_recursive();
    let lens1 = all_nodes.iter().find(|n| n.name() == "75 mm lens").unwrap();
    let lens1_id = lens1.uuid();

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

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_lens1_to_lens2.clone(), "distance_lens1_lens2")
                .with_bounds(BoxBounds::new(0.100, 0.150)?),
        )?
        .add_variable(
            FreeVariable::new(dist_lens2_to_spot.clone(), "distance_lens2_spot")
                .with_bounds(BoxBounds::new(0.030, 0.080)?),
        )?
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .add_operand(MaxTotalLengthOperand::new(
            "compact_length_limit",
            vec![dist_lens1_to_lens2.clone(), dist_lens2_to_spot.clone()],
            0.185,
            50.0,
        ))
        .optimize()?;

    let d1 = summary.value(&dist_lens1_to_lens2).unwrap();
    let d2 = summary.value(&dist_lens2_to_spot).unwrap();
    let total_length = d1 + d2;
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!(
        total_length <= 0.186,
        "Total length ({:.3} mm) should respect 185 mm constraint",
        total_length * 1e3
    );
    assert!(optimized_rms < 1.0e-3);
    Ok(())
}

#[test]
fn test_optimize_kepler_with_min_edge_thickness() -> OpmResult<()> {
    let fixed_image_distance = millimeter!(45.0);
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(fixed_image_distance)?;

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

    let _summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(front_curvature_addr.clone(), "lens2_front_roc")
                .with_bounds(BoxBounds::new(0.040, 0.200)?),
        )?
        .add_variable(
            FreeVariable::new(rear_curvature_addr.clone(), "lens2_rear_roc")
                .with_bounds(BoxBounds::new(-0.200, -0.040)?),
        )?
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .add_operand(MinEdgeThicknessOperand::new(
            "lens2_min_edge_thickness",
            lens2_id,
            0.0015,
            500.0,
        ))
        .with_algorithm(
            OptimizationAlgorithm::nelder_mead()
                .with_max_iterations(150)
                .with_sd_tolerance(1e-7),
        )
        .optimize()?;

    let (node_ref, _) = doc.scenery().node_recursive(lens2_id)?;
    let edge_thickness = node_ref
        .as_edge_thickness()
        .expect("Lens must implement EdgeThickness")
        .min_edge_thickness()?;

    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!(
        edge_thickness.get::<millimeter>() >= 1.49,
        "Edge thickness ({:.3} mm) violated 1.5 mm constraint",
        edge_thickness.get::<millimeter>()
    );
    assert!(optimized_rms < 1.0e-3);
    Ok(())
}

#[test]
fn test_min_edge_thickness_rejects_incompatible_node() -> OpmResult<()> {
    let (doc, _analyzer_id, _lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    let invalid_operand = MinEdgeThicknessOperand::new("invalid_op", spot_id, 0.005, 1.0);
    let res = MeritFunction::new(doc, vec![], vec![], vec![Arc::new(invalid_operand)]);

    assert!(
        res.is_err(),
        "Must reject applying edge thickness constraint to non-lens node"
    );
    Ok(())
}

#[test]
fn test_transmission_penalty_operand() -> OpmResult<()> {
    let (mut doc, analyzer_id, _lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    doc.analyze_headless()?;

    let valid_rays_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "valid rays".into(),
    };

    let expected_rays = 91;

    let context = EvaluationContext::new(&doc);
    let penalty_operand = TransmissionPenaltyOperand::new(
        "penalty_normal",
        valid_rays_addr.clone(),
        expected_rays,
        50.0,
    );

    penalty_operand.validate(&doc)?;
    let initial_cost = penalty_operand.evaluate(&context)?;
    assert_eq!(
        initial_cost, 0.0,
        "No penalty when all rays are transmitted"
    );

    let strict_operand =
        TransmissionPenaltyOperand::new("penalty_strict", valid_rays_addr, 100, 50.0);
    let penalty_cost = strict_operand.evaluate(&context)?;
    assert!(penalty_cost > 0.0);

    approx::assert_relative_eq!(penalty_cost, 0.405, epsilon = 1e-4);
    Ok(())
}
