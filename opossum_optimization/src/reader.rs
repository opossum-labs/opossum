use crate::address::{ObservableAddress, ParameterAddress};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

pub struct MetricReader;

impl MetricReader {
    /// Reads a scalar metric produced by a specific analyzer on a detector node.
    pub fn read_metric(doc: &OpmDocument, address: &ObservableAddress) -> OpmResult<f64> {
        // 1. Primary: Fast lookup in MetricStore by (analyzer_id, node_id, metric_name)
        let key = (
            address.analyzer_id,
            address.node_id,
            address.metric_name.clone(),
        );
        if let Some(&val) = doc.metric_store().get(&key) {
            return Ok(val);
        }

        // 2. Fallback: Direct inspection on the detector node in the scenery graph
        let (node_ref, _) = doc.scenery().node_recursive(address.node_id).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to find detector node {}: {}",
                address.node_id, e
            ))
        })?;

        node_ref
            .read_observable(&address.metric_name)
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "Metric '{}' produced no data on node {} for analyzer {}",
                    address.metric_name, address.node_id, address.analyzer_id
                ))
            })
    }

    /// Reads the current scalar value of any parameter address from the document.
    pub fn read_parameter_value(doc: &OpmDocument, address: &ParameterAddress) -> OpmResult<f64> {
        match address {
            ParameterAddress::NodeProperty {
                node_id,
                property_name,
            } => {
                let (node_ref, _) = doc.scenery().node_recursive(*node_id)?;
                node_ref.node_attr().properties().get_f64(property_name)
            }
            ParameterAddress::ConnectionDistance { src_id, src_port } => {
                let (_, parent_group_id) = doc.scenery().node_recursive(*src_id)?;
                doc.scenery().with_group_node(parent_group_id, |group| {
                    group
                        .connections()
                        .into_iter()
                        .find(|c| c.src_id == *src_id && c.src_port == *src_port)
                        .map(|c| c.distance.value)
                        .ok_or_else(|| {
                            OpossumError::Other(format!(
                                "Connection starting from {src_id}:{src_port} not found"
                            ))
                        })
                })?
            }
        }
    }
}
