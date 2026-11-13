use std::collections::HashMap;
use uuid::Uuid;

use opossum_core::{
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    nodes::NodeGroup,
    opm_document::{
        OpmDocument,
        optimization::{
            DiscoveredAnalyzer, DiscoveredEdgeThicknessNode, DiscoveredObservable,
            DiscoveredVariable, ModelOptimizationSchema, ObservableAddress, ParameterAddress,
        },
    },
    properties::Proptype,
};

pub struct SchemaExtractor;

impl SchemaExtractor {
    /// Extracts all controllable variables, observable metrics, and constraint candidates from an `OpmDocument`.
    #[must_use]
    pub fn extract(doc: &OpmDocument) -> ModelOptimizationSchema {
        let mut schema = ModelOptimizationSchema::default();

        // 1. Build complete name and group lookup maps for all nodes and groups across the scenery
        let mut node_names = HashMap::new();
        Self::collect_all_node_names(doc.scenery(), &mut node_names);

        let mut node_groups = HashMap::new();
        Self::collect_node_groups(doc.scenery(), "", &mut node_groups);

        // 2. Extract controllable variables from the scenery recursively
        Self::extract_from_group(doc.scenery(), "", &node_names, &mut schema);

        let all_nodes = doc.scenery().collect_all_nodes_recursive();

        // 3. Discover nodes implementing the EdgeThickness constraint trait
        for node_ref in &all_nodes {
            if node_ref.as_edge_thickness().is_some() {
                schema
                    .edge_thickness_nodes
                    .push(DiscoveredEdgeThicknessNode {
                        node_id: node_ref.uuid(),
                        node_name: node_ref.name().to_string(),
                        node_type: node_ref.node_type().to_string(),
                    });
            }
        }

        // 4. Extract analyzers and observable metrics across all configured analyzers
        for (analyzer_id, analyzer_info) in doc.analyzers() {
            let analyzer_kind = analyzer_info.analyzer_type().to_string();
            let analyzer_name = analyzer_info.name().unwrap_or(&analyzer_kind).to_string();

            schema.analyzers.push(DiscoveredAnalyzer {
                id: analyzer_id,
                name: analyzer_name.clone(),
                kind: analyzer_kind.clone(),
            });

            for node_ref in &all_nodes {
                let metrics = node_ref.observable_metrics(analyzer_info.kind());
                if metrics.is_empty() {
                    continue;
                }
                let node_id = node_ref.uuid();
                let node_name = node_ref.name().to_string();
                let (group_id, group_name) = node_groups
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| (None, "Main System".to_string()));

                for descriptor in metrics {
                    schema.observable_metrics.push(DiscoveredObservable {
                        address: ObservableAddress {
                            analyzer_id,
                            node_id,
                            metric_name: descriptor.name.clone(),
                        },
                        group_id,
                        group_name: group_name.clone(),
                        analyzer_name: analyzer_name.clone(),
                        analyzer_kind: analyzer_kind.clone(),
                        node_name: node_name.clone(),
                        descriptor,
                    });
                }
            }
        }

        schema.saved_config = doc.optimization_config().cloned();

        schema
    }

    /// Recursively collects human-readable names for all nodes and groups in the graph.
    fn collect_all_node_names(group: &NodeGroup, map: &mut HashMap<Uuid, String>) {
        map.insert(
            group.node_attr().uuid(),
            group.node_attr().name().to_string(),
        );
        for node_ref in group.nodes() {
            map.insert(node_ref.uuid(), node_ref.name().to_string());
            if let Some(sub_group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                Self::collect_all_node_names(sub_group, map);
            }
        }
    }

    /// Recursively maps each node to its parent group ID and formatted group path.
    fn collect_node_groups(
        group: &NodeGroup,
        parent_path: &str,
        map: &mut HashMap<Uuid, (Option<Uuid>, String)>,
    ) {
        let group_id = group.node_attr().uuid();
        let raw_title = group.node_attr().name();
        let current_group_name = if parent_path.is_empty() {
            if raw_title.is_empty() || raw_title == "scenery" {
                "Main System".to_string()
            } else {
                raw_title.to_string()
            }
        } else {
            format!("{parent_path} / {raw_title}")
        };

        for node_ref in group.nodes() {
            map.insert(
                node_ref.uuid(),
                (Some(group_id), current_group_name.clone()),
            );
            if let Some(sub_group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                Self::collect_node_groups(sub_group, &current_group_name, map);
            }
        }
    }

    /// Recursively inspects a `NodeGroup` for controllable properties and connection distances.
    fn extract_from_group(
        group: &NodeGroup,
        parent_path: &str,
        node_names: &HashMap<Uuid, String>,
        schema: &mut ModelOptimizationSchema,
    ) {
        let group_id = group.node_attr().uuid();
        let raw_title = group.node_attr().name();
        let current_group_name = if parent_path.is_empty() {
            if raw_title.is_empty() || raw_title == "scenery" {
                "Main System".to_string()
            } else {
                raw_title.to_string()
            }
        } else {
            format!("{parent_path} / {raw_title}")
        };

        // Collect edge connection distances inside this group with resolved node names
        for conn in group.connections() {
            let src_name = node_names
                .get(&conn.src_id)
                .cloned()
                .unwrap_or_else(|| conn.src_id.to_string());
            let target_name = node_names
                .get(&conn.target_id)
                .cloned()
                .unwrap_or_else(|| conn.target_id.to_string());

            // Omit redundant port names if standard linear ports are connected
            let display_name = if conn.src_port == "output_1" && conn.target_port == "input_1" {
                "Distance".to_string()
            } else {
                format!("Distance ({} → {})", conn.src_port, conn.target_port)
            };

            schema.controllable_variables.push(DiscoveredVariable {
                address: ParameterAddress::ConnectionDistance {
                    src_id: conn.src_id,
                    src_port: conn.src_port.clone(),
                },
                group_id: Some(group_id),
                group_name: current_group_name.clone(),
                node_name: format!("{src_name} → {target_name}"),
                display_name,
                initial_value: conn.distance.value,
                is_read_only: false,
            });
        }

        // Collect node properties
        for node_ref in group.nodes() {
            let node_id = node_ref.uuid();
            let node_name = node_ref.name().to_string();
            let attr = node_ref.node_attr();

            // Recurse into nested sub-groups
            if let Some(sub_group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                Self::extract_from_group(sub_group, &current_group_name, node_names, schema);
                continue;
            }

            for (prop_name, prop) in attr.properties() {
                match prop.prop() {
                    Proptype::Curvature(radius_len) => {
                        let radius_m = radius_len.value;
                        let initial_c = if !radius_m.is_finite() || radius_m.abs() < 1e-12 {
                            0.0
                        } else {
                            1.0 / radius_m
                        };

                        schema.controllable_variables.push(DiscoveredVariable {
                            address: ParameterAddress::SurfaceCurvature {
                                node_id,
                                property_name: prop_name.clone(),
                            },
                            group_id: Some(group_id),
                            group_name: current_group_name.clone(),
                            node_name: node_name.clone(),
                            display_name: format!("{prop_name} (curvature)"),
                            initial_value: initial_c,
                            is_read_only: prop.is_read_only(),
                        });
                    }

                    _ => {
                        if let Some(val) = prop.prop().as_f64() {
                            schema.controllable_variables.push(DiscoveredVariable {
                                address: ParameterAddress::NodeProperty {
                                    node_id,
                                    property_name: prop_name.clone(),
                                },
                                group_id: Some(group_id),
                                group_name: current_group_name.clone(),
                                node_name: node_name.clone(),
                                display_name: prop_name.clone(),
                                initial_value: val,
                                is_read_only: prop.is_read_only(),
                            });
                        }
                    }
                }
            }
        }
    }
}
