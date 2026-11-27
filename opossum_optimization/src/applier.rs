use opossum_core::{
    error::OpmResult, opm_document::{OpmDocument, optimization::ParameterAddress}, utils::geom_transformation::{AlignmentAxis, Isometry},
};
use uom::si::{angle::radian, f64::{Angle, Length}, length::meter};

pub struct ParameterApplier;

impl ParameterApplier {
    /// Applies a scalar value to the specified target address in the document.
    ///
    /// # Errors
    ///
    /// This function returns an error if the underlying properties or node attributes could not be set.
    pub fn set_value(
        doc: &mut OpmDocument,
        address: &ParameterAddress,
        value: f64,
    ) -> OpmResult<()> {
        match address {
            ParameterAddress::NodeProperty {
                node_id,
                property_name,
            } => doc.scenery_mut().with_node_attr_mut(*node_id, |attr| {
                attr.properties_mut().set_internal_f64(property_name, value)
            })?,

            ParameterAddress::SurfaceCurvature {
                node_id,
                property_name,
            } => {
                // Curvature c [1/m] -> Radius of curvature R [m]
                let radius_m = if value.abs() < 1e-12 {
                    f64::INFINITY
                } else {
                    1.0 / value
                };
                doc.scenery_mut().with_node_attr_mut(*node_id, |attr| {
                    attr.properties_mut()
                        .set_internal_f64(property_name, radius_m)
                })?
            }
            ParameterAddress::ConnectionDistance { src_id, src_port } => {
                let distance = Length::new::<meter>(value);
                // Resolve parent group containing the source node
                let (_, parent_group_id) = doc.scenery().node_recursive(*src_id)?;
                doc.scenery_mut()
                    .with_group_node_mut(parent_group_id, |group| {
                        group
                            .update_connection_distance(*src_id, src_port, distance)
                            .map(|_| ())
                    })?
            }
            ParameterAddress::NodeAlignment { node_id, axis } => {
                doc.scenery_mut().with_node_attr_mut(*node_id, |attr| {
                    let mut alignment = attr.alignment().unwrap_or_else(Isometry::identity);
                    match axis {
                        AlignmentAxis::Translation(t_axis) => {
                            alignment.set_translation_of_axis(*t_axis, Length::new::<meter>(value))?;
                        }
                        AlignmentAxis::Rotation(r_axis) => {
                            alignment.set_rotation_of_axis(*r_axis, Angle::new::<radian>(value))?;
                        }
                    }
                    attr.set_alignment(alignment);
                    Ok(())
                })?
            }
        }
    }

    /// Rebuilds all optical surfaces across the model after parameters have been updated.
    ///
    /// # Errors
    ///
    /// This function returns an error if the underlying node surfaces of the model could not be created.
    pub fn finalize_update(doc: &mut OpmDocument) -> OpmResult<()> {
        doc.scenery_mut().rebuild_surfaces()
    }
}
