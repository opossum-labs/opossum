mod common;

use common::build_kepler_setup;
use opossum_core::{
    error::OpmResult,
    micrometer, millimeter,
    opm_document::optimization::{
        BoxBounds, FreeVariable, MetricOperand, ObservableAddress, OperandMode,
        OptimizationAlgorithm, ParameterAddress, ParameterCoupling,
    },
};
use opossum_optimization::{MetricReader, OpticOptimizer};

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
            FreeVariable::new(front_curvature_addr.clone())
                .with_length_bounds(millimeter!(30.0), millimeter!(150.0))?,
        )?
        // R2 = -R1
        .add_coupling(ParameterCoupling::opposite(
            rear_curvature_addr.clone(),
            front_curvature_addr.clone(),
        ))
        .add_operand(MetricOperand::for_spot_radius(
            spot_observable_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
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
fn test_optimize_kepler_best_form_lens_curvature() -> OpmResult<()> {
    let fixed_image_distance = millimeter!(45.0);
    let (mut doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(fixed_image_distance)?;

    // Use SurfaceCurvature in [1/m]
    let front_c_addr = ParameterAddress::SurfaceCurvature {
        node_id: lens2_id,
        property_name: "front curvature".into(),
    };
    let rear_c_addr = ParameterAddress::SurfaceCurvature {
        node_id: lens2_id,
        property_name: "rear curvature".into(),
    };
    let spot_observable_addr = ObservableAddress {
        analyzer_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    // R in [25 mm, 250 mm] corresponds to c in [4.0 m^-1, 40.0 m^-1]
    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(front_c_addr.clone()).with_bounds(BoxBounds::new(4.0, 40.0)?),
        )?
        .add_variable(
            FreeVariable::new(rear_c_addr.clone()).with_bounds(BoxBounds::new(-40.0, -4.0)?),
        )?
        .add_operand(MetricOperand::for_spot_radius(
            spot_observable_addr.clone(),
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_algorithm(OptimizationAlgorithm::lbfgs().with_max_iterations(30))
        .optimize()?;

    let best_c1 = summary.value(&front_c_addr).unwrap();
    let best_c2 = summary.value(&rear_c_addr).unwrap();

    // R = 1 / c
    let r_front = 1.0 / best_c1;
    let r_rear = 1.0 / best_c2;

    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    // 1. Kosten müssen sich verbessert haben
    assert!(summary.final_cost < summary.initial_cost);

    // 2. RMS-Spotgröße muss makroskopisch deutlich unter 1 mm liegen (liegt typischerweise bei < 100 µm)
    assert!(optimized_rms < 1.0e-3);

    // 3. Krümmungsradien müssen physikalisch finite Werte annehmen
    assert!(r_front.is_finite() && r_rear.is_finite());

    // 4. Formfaktor q = (R2 + R1) / (R2 - R1) muss endlich sein
    let shape_factor_q = (r_rear + r_front) / (r_rear - r_front);
    assert!(shape_factor_q.is_finite());

    Ok(())
}
