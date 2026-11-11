mod common;

use common::build_kepler_setup;
use opossum_core::{
    error::{OpmResult, OpossumError},
    micrometer, millimeter,
};
use opossum_optimization::{
    FreeVariable, MetricOperand, MinEdgeThicknessOperand, ObservableAddress, OperandMode,
    OptimizationAlgorithm, OptimizationConfig, ParameterAddress, SchemaExtractor,
    TransmissionPenaltyOperand,
};
use uuid::Uuid;

#[test]
fn test_schema_discovery_on_kepler_model() -> OpmResult<()> {
    let (doc, analyzer_id, lens2_id, spot_id) = build_kepler_setup(millimeter!(45.0))?;

    let schema = SchemaExtractor::extract(&doc);

    assert!(
        schema.controllable_variables.len() >= 3,
        "Expected at least 3 propagation distances in the model"
    );

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

    let spot_metrics: Vec<_> = schema
        .observable_metrics
        .iter()
        .filter(|m| m.address.analyzer_id == analyzer_id && m.address.node_id == spot_id)
        .collect();

    assert_eq!(
        spot_metrics.len(),
        5,
        "SpotDiagram must provide 5 scalar observables"
    );

    assert!(
        schema
            .edge_thickness_nodes
            .iter()
            .any(|n| n.node_id == lens2_id),
        "Lens 2 must be recognized as supporting edge thickness"
    );

    Ok(())
}

#[test]
fn test_optimization_config_serde_roundtrip() -> OpmResult<()> {
    let node_id = Uuid::new_v4();
    let analyzer_id = Uuid::new_v4();

    let config = OptimizationConfig::new()
        .with_variable(
            FreeVariable::new(ParameterAddress::NodeProperty {
                node_id,
                property_name: "front curvature".into(),
            })
            .with_length_bounds(millimeter!(50.0), millimeter!(200.0))?,
        )
        .with_operand(MetricOperand::for_spot_radius(
            ObservableAddress {
                analyzer_id,
                node_id,
                metric_name: "rms beam radius".into(),
            },
            micrometer!(0.0),
            OperandMode::Equal,
            1.0,
        ))
        .with_operand(MinEdgeThicknessOperand::for_thickness(
            node_id,
            millimeter!(2.0),
            100.0,
        ))
        .with_operand(TransmissionPenaltyOperand::new(
            ObservableAddress {
                analyzer_id,
                node_id,
                metric_name: "valid rays".into(),
            },
            91,
            50.0,
        ))
        .with_algorithm(OptimizationAlgorithm::lbfgs());

    let json = ron::to_string(&config).map_err(|e| OpossumError::Other(e.to_string()))?;
    let deserialized: OptimizationConfig =
        ron::from_str(&json).map_err(|e| OpossumError::Other(e.to_string()))?;

    assert_eq!(config, deserialized);
    Ok(())
}
