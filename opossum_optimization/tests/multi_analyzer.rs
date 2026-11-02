mod common;

use common::build_dual_analyzer_kepler_setup;
use opossum_core::error::OpmResult;
use opossum_optimization::{
    BoxBounds, FreeVariable, MetricReader, ObservableAddress, OpticOptimizer,
    OptimizationAlgorithm, ParameterAddress, TargetValueOperand,
};

#[test]
fn test_optimize_kepler_multi_analyzer() -> OpmResult<()> {
    let (mut doc, analyzer_wf_id, analyzer_spot_id, lens1_id, wf_id, spot_id) =
        build_dual_analyzer_kepler_setup()?;

    let distance_1 = ParameterAddress::ConnectionDistance {
        src_id: lens1_id,
        src_port: "output_1".into(),
    };
    let distance_2 = ParameterAddress::ConnectionDistance {
        src_id: wf_id,
        src_port: "output_1".into(),
    };

    let wf_rms_addr = ObservableAddress {
        analyzer_id: analyzer_wf_id,
        node_id: wf_id,
        metric_name: "Wavefront RMS".into(),
    };
    let spot_rms_addr = ObservableAddress {
        analyzer_id: analyzer_spot_id,
        node_id: spot_id,
        metric_name: "rms beam radius".into(),
    };

    let summary = OpticOptimizer::new(&mut doc)
        .add_variable(
            FreeVariable::new(distance_1.clone(), "l1 <-> l2")
                .with_bounds(BoxBounds::new(0.02, 0.3)?),
        )?
        .add_variable(
            FreeVariable::new(distance_2.clone(), "wf <-> sd")
                .with_bounds(BoxBounds::new(0.02, 0.3)?),
        )?
        .add_operand(TargetValueOperand::new(
            "minimize_wavefront_rms",
            wf_rms_addr.clone(),
            0.0,
            1.0,
            0.02,
        ))
        .add_operand(TargetValueOperand::new(
            "minimize_spot_radius",
            spot_rms_addr.clone(),
            0.0,
            1.0,
            5.0e-5,
        ))
        .with_algorithm(OptimizationAlgorithm::lbfgs().with_max_iterations(30))
        .optimize()?;

    let final_wf_rms = MetricReader::read_metric(&doc, &wf_rms_addr)?;
    let _final_spot_rms = MetricReader::read_metric(&doc, &spot_rms_addr)?;

    assert!(summary.final_cost < summary.initial_cost);
    assert!(
        final_wf_rms < 0.25,
        "Wavefront RMS should improve significantly"
    );
    Ok(())
}
