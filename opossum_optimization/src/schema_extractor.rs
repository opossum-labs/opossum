use std::collections::HashMap;
use uuid::Uuid;

use opossum_core::{
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    nodes::{NodeGroup, NodeReference},
    opm_document::{
        OpmDocument,
        optimization::{
            DiscoveredAnalyzer, DiscoveredEdgeThicknessNode, DiscoveredObservable,
            DiscoveredVariable, ModelOptimizationSchema, ObservableAddress, ParameterAddress,
            ParameterUnit,
        },
    },
    properties::Proptype,
    utils::geom_transformation::{AlignmentAxis, Isometry},
};

pub struct SchemaExtractor;

impl SchemaExtractor {
    /// Resolves physical UI unit metadata from a [`Proptype`] variant.
    #[must_use]
    fn unit_for_proptype(prop: &Proptype) -> Option<ParameterUnit> {
        match prop {
            Proptype::Length(_) | Proptype::RadiusOfCurvature(_) => Some(ParameterUnit::length()),
            Proptype::Angle(_) => Some(ParameterUnit::angle_deg()),
            Proptype::Energy(_) => Some(ParameterUnit::energy()),
            Proptype::Fluence(_) => Some(ParameterUnit::fluence()),
            Proptype::LinearDensity(_) => Some(ParameterUnit::linear_density()),
            _ => None,
        }
    }

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

    /// Recursively inspects a `NodeGroup` for controllable properties, local alignments, and connection distances.
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

        Self::extract_connection_distances(
            group,
            group_id,
            &current_group_name,
            node_names,
            schema,
        );

        for node_ref in group.nodes() {
            let node_id = node_ref.uuid();
            let node_name = node_ref.name().to_string();
            let attr = node_ref.node_attr();

            if let Some(sub_group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                Self::extract_from_group(sub_group, &current_group_name, node_names, schema);
                continue;
            }
            let is_reference_node = node_ref.as_any().is::<NodeReference>();

            for (prop_name, prop) in attr.properties() {
                match prop.prop() {
                    Proptype::RadiusOfCurvature(radius_len) => {
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
                            unit: Some(ParameterUnit::length()),
                        });
                    }

                    other => {
                        if let Some(val) = other.as_f64() {
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
                                unit: Self::unit_for_proptype(other),
                            });
                        }
                    }
                }
            }

            if !is_reference_node {
                Self::extract_node_alignment(
                    node_id,
                    &node_name,
                    *attr.alignment(),
                    group_id,
                    &current_group_name,
                    schema,
                );
            }
        }
    }

    /// Collects edge connection distances inside a `NodeGroup` with resolved node names.
    fn extract_connection_distances(
        group: &NodeGroup,
        group_id: Uuid,
        current_group_name: &str,
        node_names: &HashMap<Uuid, String>,
        schema: &mut ModelOptimizationSchema,
    ) {
        for conn in group.connections() {
            let src_name = node_names
                .get(&conn.src_id)
                .cloned()
                .unwrap_or_else(|| conn.src_id.to_string());
            let target_name = node_names
                .get(&conn.target_id)
                .cloned()
                .unwrap_or_else(|| conn.target_id.to_string());

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
                group_name: current_group_name.to_string(),
                node_name: format!("{src_name} → {target_name}"),
                display_name,
                initial_value: conn.distance.value,
                is_read_only: false,
                unit: Some(ParameterUnit::length()),
            });
        }
    }

    /// Exposes all six local alignment degrees of freedom for a regular optical node.
    fn extract_node_alignment(
        node_id: Uuid,
        node_name: &str,
        alignment_opt: Option<Isometry>,
        group_id: Uuid,
        current_group_name: &str,
        schema: &mut ModelOptimizationSchema,
    ) {
        let alignment = alignment_opt.unwrap_or_else(Isometry::identity);
        for axis in AlignmentAxis::ALL {
            let (initial_value, unit, label) = match axis {
                AlignmentAxis::Translation(t_axis) => (
                    alignment.translation_of_axis(t_axis).value,
                    Some(ParameterUnit::length()),
                    format!("Decenter {t_axis}"),
                ),
                AlignmentAxis::Rotation(r_axis) => (
                    alignment.rotation_of_axis(r_axis).value,
                    Some(ParameterUnit::angle_deg()),
                    format!("Tilt {r_axis}"),
                ),
            };
            schema.controllable_variables.push(DiscoveredVariable {
                address: ParameterAddress::NodeAlignment { node_id, axis },
                group_id: Some(group_id),
                group_name: current_group_name.to_string(),
                node_name: node_name.to_string(),
                display_name: label,
                initial_value,
                is_read_only: false,
                unit,
            });
        }
    }
}
