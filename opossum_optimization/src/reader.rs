use crate::address::{ObservableAddress, ParameterAddress};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

pub struct MetricReader;

impl MetricReader {
    /// Reads a scalar metric directly from a node inside an `OpmDocument`.
    pub fn read_metric(doc: &OpmDocument, address: &ObservableAddress) -> OpmResult<f64> {
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
                    "Metric '{}' produced no data on node {}",
                    address.metric_name, address.node_id
                ))
            })
    }

    /// Reads the current scalar value of any parameter address from the document[cite: 4, 8, 9].
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
