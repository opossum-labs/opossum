use opossum_core::opm_document::optimization::{
    MetricSource, ModelOptimizationSchema, ParameterAddress, StandardOperand,
};
use uuid::Uuid;

/// UI-specific representation of a merit function operand row.
#[derive(Clone, PartialEq, Debug)]
pub struct UiOperandRow {
    pub id: Uuid,
    pub operand: StandardOperand,
    pub group_id: Option<Uuid>,
    pub is_enabled: bool,
}

/// Resolves the `NodeGroup` UUID for an operand based on its referenced node in the schema.
#[must_use]
pub fn resolve_operand_group_id(
    schema: Option<&ModelOptimizationSchema>,
    operand: &StandardOperand,
) -> Option<Uuid> {
    let schema = schema?;
    match operand {
        StandardOperand::Metric(m) => match &m.source {
            MetricSource::Observable(obs) => schema
                .observable_metrics
                .iter()
                .find(|o| o.address.node_id == obs.node_id)
                .and_then(|o| o.group_id),
            MetricSource::Parameter(_) => None,
        },
        StandardOperand::MinEdgeThickness(t) => schema
            .controllable_variables
            .iter()
            .find(|v| match &v.address {
                ParameterAddress::SurfaceCurvature { node_id, .. } => *node_id == t.node_id,
                _ => false,
            })
            .and_then(|v| v.group_id),
        StandardOperand::TransmissionPenalty(t) => schema
            .observable_metrics
            .iter()
            .find(|m| m.address.node_id == t.address.node_id)
            .and_then(|m| m.group_id),
        StandardOperand::MaxTotalLength(_) => None,
    }
}

/// Formats an operand into `(badge_text, detail_text, operator_symbol)` for display in the table.
#[must_use]
pub fn describe_operand(
    schema: Option<&ModelOptimizationSchema>,
    operand: &StandardOperand,
) -> (String, String, &'static str) {
    let analyzer_name = |analyzer_id: Uuid| -> String {
        schema
            .and_then(|s| s.analyzers.iter().find(|a| a.id == analyzer_id))
            .map_or_else(|| "Analyzer".to_string(), |a| a.name.clone())
    };

    match operand {
        StandardOperand::Metric(m) => {
            let symbol = m.mode().symbol();
            match &m.source {
                MetricSource::Observable(obs) => {
                    let aname = analyzer_name(obs.analyzer_id);
                    let nname = schema
                        .and_then(|s| {
                            s.observable_metrics
                                .iter()
                                .find(|entry| entry.address.node_id == obs.node_id)
                                .map(|entry| entry.node_name.clone())
                        })
                        .unwrap_or_else(|| "Detector".to_string());
                    (aname, format!("{nname} → {}", obs.metric_name), symbol)
                }
                MetricSource::Parameter(p) => ("Parameter".to_string(), p.to_string(), symbol),
            }
        }
        StandardOperand::MinEdgeThickness(t) => {
            let node_name = schema
                .and_then(|s| {
                    s.edge_thickness_nodes
                        .iter()
                        .find(|n| n.node_id == t.node_id)
                        .map(|n| n.node_name.clone())
                })
                .unwrap_or_else(|| t.node_id.to_string());
            (
                "Geometry".to_string(),
                format!("{node_name} → Min Edge"),
                "≥",
            )
        }
        StandardOperand::MaxTotalLength(_) => (
            "Geometry".to_string(),
            "Total Track Length".to_string(),
            "≤",
        ),
        StandardOperand::TransmissionPenalty(t) => {
            let aname = analyzer_name(t.address.analyzer_id);
            let nname = schema
                .and_then(|s| {
                    s.observable_metrics
                        .iter()
                        .find(|entry| {
                            entry.address == t.address || entry.address.node_id == t.address.node_id
                        })
                        .map(|entry| entry.node_name.clone())
                })
                .unwrap_or_else(|| "Detector".to_string());
            (aname, format!("{nname} → Transmission"), "≥")
        }
    }
}
