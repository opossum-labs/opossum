mod common;

use common::build_kepler_setup;
use opossum_core::{
    core_optics::NodeAttrExt,
    error::OpmResult,
    micrometer, millimeter,
    opm_document::optimization::{
        FreeVariable, MetricOperand, ObservableAddress, OperandMode, OptimizationAlgorithm,
        ParameterAddress,
    },
};
use opossum_optimization::{MeritFunction, MetricReader, OpticOptimizer};
use std::sync::Arc;

#[test]
fn test_optimize_kepler_focal_position_nelder_mead() -> OpmResult<()> {
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

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_address.clone())
                .with_length_bounds(millimeter!(30.0), millimeter!(80.0))?,
        )?
        .add_operand(MetricOperand::for_spot_radius(
            spot_observable_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_algorithm(OptimizationAlgorithm::nelder_mead())
        .optimize()?;

    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!((best_distance - 0.058).abs() < 0.005);
    assert!(optimized_rms < 1.0e-3);
    assert!(summary.final_cost < summary.initial_cost);
    Ok(())
}

#[test]
fn test_optimize_kepler_focal_position_lbfgs() -> OpmResult<()> {
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

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(dist_address.clone())
                .with_length_bounds(millimeter!(30.0), millimeter!(80.0))?,
        )?
        // Millimeter scale matches the initial ~0.5 mm defocused spot size
        .add_operand(MetricOperand::for_length(
            spot_observable_addr.clone(),
            millimeter!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_algorithm(OptimizationAlgorithm::lbfgs().with_max_iterations(25))
        .optimize()?;

    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!((best_distance - 0.058).abs() < 0.005);
    assert!(optimized_rms < 1.0e-3);
    assert!(summary.final_cost < summary.initial_cost);
    Ok(())
}

#[test]
fn test_optimize_kepler_focal_position_simulated_annealing() -> OpmResult<()> {
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(35.0))?;

    let dist_address = ParameterAddress::ConnectionDistance {
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
            FreeVariable::new(dist_address.clone())
                .with_length_bounds(millimeter!(30.0), millimeter!(80.0))?,
        )?
        .add_operand(MetricOperand::for_spot_radius(
            spot_observable_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_algorithm(OptimizationAlgorithm::simulated_annealing().with_temperature(1.0))
        .optimize()?;

    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!((best_distance - 0.058).abs() < 0.006);
    assert!(optimized_rms < 1.0e-3);
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

    let free_var = FreeVariable::new(dist_addr);
    let operand =
        MetricOperand::for_spot_radius(spot_addr, micrometer!(0.0), OperandMode::Equal, 1.0);

    let merit = MeritFunction::new(doc, vec![free_var], vec![], vec![Arc::new(operand)])?;

    let grad_45 = merit.compute_gradient(&[0.045]);
    assert!(
        grad_45[0] < -50.0,
        "Gradient at 45 mm should be strongly negative, got: {}",
        grad_45[0]
    );

    let grad_58 = merit.compute_gradient(&[0.058]);
    assert!(
        grad_58[0].abs() < grad_45[0].abs() * 0.1,
        "Gradient at focus (58 mm) must be much smaller than at 45 mm, got: {}",
        grad_58[0]
    );
    Ok(())
}

#[test]
fn test_parallel_gradient_thread_execution() -> OpmResult<()> {
    let (doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    let all_nodes = doc.scenery().collect_all_nodes_recursive();
    let lens1 = all_nodes.iter().find(|n| n.name() == "75 mm lens").unwrap();
    let lens1_id = lens1.uuid();

    let dist1 = FreeVariable::new(ParameterAddress::ConnectionDistance {
        src_id: lens1_id,
        src_port: "output_1".into(),
    });
    let dist2 = FreeVariable::new(ParameterAddress::ConnectionDistance {
        src_id: lens2_id,
        src_port: "output_1".into(),
    });
    let r1 = FreeVariable::new(ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "front curvature".into(),
    });
    let r2 = FreeVariable::new(ParameterAddress::NodeProperty {
        node_id: lens2_id,
        property_name: "rear curvature".into(),
    });

    let spot_observable = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };
    let operand =
        MetricOperand::for_spot_radius(spot_observable, micrometer!(0.0), OperandMode::Equal, 1.0);

    let merit = MeritFunction::new(
        doc,
        vec![dist1, dist2, r1, r2],
        vec![],
        vec![Arc::new(operand)],
    )?;

    let x0 = vec![0.125, 0.045, 0.100, -0.100];
    let grad = merit.compute_gradient(&x0);

    assert_eq!(grad.len(), 4);
    assert!(grad.iter().all(|g| !g.is_nan()));
    Ok(())
}
