use opossum_core::{
    opm_document::OpmDocument,
    error::OpmResult,
};
use uom::si::{f64::Length, length::meter};
use super::address::ParameterAddress;

pub struct ParameterApplier;

impl ParameterApplier {
    /// Applies a scalar value to the specified target address in the document.
    pub fn set_value(
        doc: &mut OpmDocument,
        address: &ParameterAddress,
        value: f64,
    ) -> OpmResult<()> {
        match address {
            ParameterAddress::NodeProperty {
                node_id,
                property_name,
            } => {
                doc.scenery_mut().with_node_attr_mut(*node_id, |attr| {
                    attr.properties_mut().set_internal_f64(property_name, value)
                })?
            }
            ParameterAddress::ConnectionDistance {
                parent_group_id,
                src_id,
                src_port,
            } => {
                let distance = Length::new::<meter>(value);
                doc.scenery_mut().with_group_node_mut(*parent_group_id, |group| {
                    group
                        .update_connection_distance(*src_id, src_port, distance)
                        .map(|_| ())
                })?
            }
        }
    }

    /// Rebuilds all optical surfaces across the model after parameters have been updated.
    pub fn finalize_update(doc: &mut OpmDocument) -> OpmResult<()> {
        doc.scenery_mut().rebuild_surfaces()
    }
}