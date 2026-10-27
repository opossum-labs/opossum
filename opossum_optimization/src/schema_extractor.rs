use super::{
    address::{ObservableAddress, ParameterAddress},
    discovery::{DiscoveredObservable, DiscoveredVariable, ModelOptimizationSchema},
};
use opossum_core::{
    core_optics::NodeAttrExt, error::OpmResult, nodes::NodeGroup, opm_document::OpmDocument,
};

pub struct SchemaExtractor;

impl SchemaExtractor {
    /// Extracts all controllable variables and observable metrics from an `OpmDocument`.
    pub fn extract(doc: &OpmDocument) -> OpmResult<ModelOptimizationSchema> {
        let mut schema = ModelOptimizationSchema::default();

        // 1. Extract controllable variables from the scenery recursively
        Self::extract_from_group(doc.scenery(), &mut schema)?;

        // 2. Extract observable metrics across all configured analyzers
        let all_nodes = doc.scenery().collect_all_nodes_recursive()?;
        for (analyzer_id, analyzer_info) in doc.analyzers() {
            let analyzer_kind = analyzer_info.kind();

            for node_ref in &all_nodes {
                let metrics = node_ref.observable_metrics(analyzer_kind);
                if metrics.is_empty() {
                    continue;
                }

                let node_id = node_ref.uuid();
                let node_name = node_ref.name().to_string();

                for descriptor in metrics {
                    schema.observable_metrics.push(DiscoveredObservable {
                        address: ObservableAddress {
                            analyzer_id,
                            node_id,
                            metric_name: descriptor.name.clone(),
                        },
                        node_name: node_name.clone(),
                        descriptor,
                    });
                }
            }
        }

        Ok(schema)
    }

    /// Recursively inspects a `NodeGroup` for controllable properties and connection distances.
    fn extract_from_group(
        group: &NodeGroup,
        schema: &mut ModelOptimizationSchema,
    ) -> OpmResult<()> {
        // Collect edge connection distances inside this group
        for conn in group.connections() {
            schema.controllable_variables.push(DiscoveredVariable {
                address: ParameterAddress::ConnectionDistance {
                    src_id: conn.src_id,
                    src_port: conn.src_port.clone(),
                },
                node_name: format!("Distance {} -> {}", conn.src_id, conn.target_id),
                display_name: format!("{}:{} distance", conn.src_id, conn.src_port),
                initial_value: conn.distance.value,
                is_read_only: false,
            });
        }

        // Collect scalar node properties
        for node_ref in group.nodes() {
            let node_id = node_ref.uuid();
            let node_name = node_ref.name().to_string();
            let attr = node_ref.node_attr();

            for (prop_name, prop) in attr.properties() {
                // Only consider properties that can be represented as an f64 scalar
                if let Some(val) = prop.prop().as_f64() {
                    schema.controllable_variables.push(DiscoveredVariable {
                        address: ParameterAddress::NodeProperty {
                            node_id,
                            property_name: prop_name.clone(),
                        },
                        node_name: node_name.clone(),
                        display_name: prop_name.clone(),
                        initial_value: val,
                        is_read_only: prop.is_read_only(),
                    });
                }
            }

            // Recurse into nested sub-groups
            if let Some(sub_group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                Self::extract_from_group(sub_group, schema)?;
            }
        }

        Ok(())
    }
}
