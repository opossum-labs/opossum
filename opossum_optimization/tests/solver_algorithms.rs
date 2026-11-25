mod common;

use common::build_kepler_setup;
use opossum_core::{
    error::OpmResult,
    micrometer, millimeter,
    opm_document::optimization::{
        FreeVariable, MetricOperand, ObservableAddress, OperandMode, OptimizationAlgorithm,
        ParameterAddress,
    },
};
use opossum_optimization::{MetricReader, OpticOptimizer};

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
fn test_optimize_kepler_focal_position_dls() -> OpmResult<()> {
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
        .with_algorithm(OptimizationAlgorithm::dls().with_max_iterations(25))
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
        .with_algorithm(
            OptimizationAlgorithm::adaptive_simulated_annealing()
                .with_temperature(1.0)
                .with_max_iterations(50),
        )
        .optimize()?;

    let best_distance = summary.value(&dist_address).unwrap();
    let optimized_rms = MetricReader::read_metric(&doc, &spot_observable_addr)?;

    assert!((best_distance - 0.058).abs() < 0.006);
    assert!(optimized_rms < 1.0e-3);
    Ok(())
}
