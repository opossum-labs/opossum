//! Marker trait for an optical node that can be analyzed

use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    error::{OpmResult, OpossumError},
    optic_node::OpticNode,
    optic_ports::PortType,
    utils::geom_transformation::Isometry,
};
use core::fmt::Debug;
use std::fmt::Display;

/// Marker trait for an optical node that can be analyzed
pub trait Analyzable: OpticNode + AnalysisEnergy + AnalysisRayTrace + AnalysisGhostFocus {
    ///Sets the coating and isometry of this surface
    /// # Errors
    /// This function errors if the coating cannot be accessed
    fn set_surface_iso_and_coating(
        &mut self,
        port_str: &str,
        iso: &Isometry,
        port_type: &PortType,
    ) -> OpmResult<()> {
        let node_attr = self.node_attr().clone();

        let input_surf = self.get_surface_mut(port_str);
        input_surf.set_isometry(iso);
        input_surf.set_coating(
            node_attr
                .ports()
                .coating(port_type, port_str)
                .ok_or_else(|| OpossumError::Other("cannot access coating!".into()))?
                .clone(),
        );
        Ok(())
    }
}
impl Debug for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}
impl Display for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
    }
}
