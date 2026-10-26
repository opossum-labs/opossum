use opossum_core::{
    opm_document::OpmDocument,
    error::{OpmResult, OpossumError},
};
use crate::address::ObservableAddress;

pub struct MetricReader;

impl MetricReader {
    /// Reads a scalar metric directly from a node inside an OpmDocument.
    pub fn read_metric(
        doc: &OpmDocument,
        address: &ObservableAddress,
    ) -> OpmResult<f64> {
        let (node_ref, _) = doc
            .scenery()
            .node_recursive(address.node_id)
            .map_err(|e| {
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
}