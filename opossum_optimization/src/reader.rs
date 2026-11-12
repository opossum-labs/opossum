use crate::address::{ObservableAddress, ParameterAddress};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

pub struct MetricReader;

impl MetricReader {
    /// Reads a scalar metric produced by a specific analyzer on a detector node.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the detector node in the given [`ObservableAddress`] was not found.
    /// - the given detector observable name was not found.
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

    /// Reads a vector of residual metrics produced by a specific analyzer on a detector node.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the detector node in the given [`ObservableAddress`] was not found.
    /// - the given detector observable name produced no vector data.
    pub fn read_metric_vector(
        doc: &OpmDocument,
        address: &ObservableAddress,
    ) -> OpmResult<Vec<f64>> {
        // 1. Primary: Fast lookup in VectorMetricStore by (analyzer_id, node_id, metric_name)
        let key = (
            address.analyzer_id,
            address.node_id,
            address.metric_name.clone(),
        );
        if let Some(vec_val) = doc.vector_metric_store().get(&key) {
            return Ok(vec_val.clone());
        }

        // Prevent reading stale/overwritten light_data from another analyzer when multiple
        // analyzers were executed during headless analysis.
        if doc.analyzers().len() > 1 && !doc.metric_store().is_empty() {
            return Err(OpossumError::Other(format!(
                "Vector metric '{}' not available on node {} for analyzer {}",
                address.metric_name, address.node_id, address.analyzer_id
            )));
        }

        // 2. Fallback: Direct inspection on the detector node in the scenery graph
        let (node_ref, _) = doc.scenery().node_recursive(address.node_id).map_err(|e| {
            OpossumError::Other(format!(
                "Failed to find detector node {}: {}",
                address.node_id, e
            ))
        })?;

        node_ref
            .read_observable_vector(&address.metric_name)
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "Metric '{}' produced no vector data on node {} for analyzer {}",
                    address.metric_name, address.node_id, address.analyzer_id
                ))
            })
    }

    /// Reads the current scalar value of any parameter address from the document.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the node defined in [`ParameterAddress`] cannot be found.
    /// - the connection between nodes defined in [`ParameterAddress`] cannot be found.
    pub fn read_parameter_value(doc: &OpmDocument, address: &ParameterAddress) -> OpmResult<f64> {
        match address {
            ParameterAddress::NodeProperty {
                node_id,
                property_name,
            } => {
                let (node_ref, _) = doc.scenery().node_recursive(*node_id)?;
                node_ref.node_attr().properties().get_f64(property_name)
            }

            ParameterAddress::SurfaceCurvature {
                node_id,
                property_name,
            } => {
                let (node_ref, _) = doc.scenery().node_recursive(*node_id)?;
                let radius_m = node_ref.node_attr().properties().get_f64(property_name)?;
                if !radius_m.is_finite() {
                    Ok(0.0) // Flat surface: R = inf -> c = 0.0
                } else if radius_m.abs() < 1e-12 {
                    Ok(1e12) // Guard against division by zero
                } else {
                    Ok(1.0 / radius_m)
                }
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
