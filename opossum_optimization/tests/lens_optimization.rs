mod common;

use common::build_kepler_setup;
use opossum_core::{error::OpmResult, millimeter};
use opossum_optimization::{
    BoxBounds, FreeVariable, MetricReader, ObservableAddress, OpticOptimizer,
    OptimizationAlgorithm, ParameterAddress, ParameterCoupling, TargetValueOperand,
};

#[test]
fn test_optimize_kepler_lens_curvature_coupled() -> OpmResult<()> {
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

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(front_curvature_addr.clone(), "lens2_front_curvature")
                .with_bounds(BoxBounds::new(0.030, 0.150)?),
        )?
        .add_coupling(ParameterCoupling::opposite(
            rear_curvature_addr.clone(),
            front_curvature_addr.clone(),
        ))
        .add_operand(TargetValueOperand::new(
            "minimize_rms_spot",
            spot_observable_addr.clone(),
            0.0,
            1.0,
            1.0e-3,
        ))
        .optimize()?;

    let best_r_front = summary.value(&front_curvature_addr).unwrap();
    let best_r_rear = summary.value(&rear_curvature_addr).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!(
        (best_r_rear - (-best_r_front)).abs() < 1e-12,
        "Coupling violated: R_rear must equal -R_front"
    );
    assert!(best_r_front < 0.100);
    assert!(optimized_rms < 1.0e-3);
    Ok(())
}

#[test]
fn test_optimize_kepler_best_form_lens() -> OpmResult<()> {
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

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(front_curvature_addr.clone(), "lens2_front_curvature")
                .with_bounds(BoxBounds::new(0.025, 0.250)?),
        )?
        .add_variable(
            FreeVariable::new(rear_curvature_addr.clone(), "lens2_rear_curvature")
                .with_bounds(BoxBounds::new(-0.250, -0.025)?),
        )?
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

    let best_r_front = summary.value(&front_curvature_addr).unwrap();
    let best_r_rear = summary.value(&rear_curvature_addr).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;
    let shape_factor_q = (best_r_rear + best_r_front) / (best_r_rear - best_r_front);

    assert!(optimized_rms < 1.0e-3);
    assert!(summary.final_cost < summary.initial_cost);
    assert!(shape_factor_q.is_finite());
    Ok(())
}
