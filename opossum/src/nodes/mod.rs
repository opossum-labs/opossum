#![warn(missing_docs)]
//! This module contains the concrete node types (lenses, filters, etc...)

mod node_attr;
mod test_helper;
mod beam_splitter;
mod detector;
mod dummy;
mod energy_meter;
mod fluence_detector;
mod group;
mod ideal_filter;
mod lens;
mod paraxial_surface;
mod propagation;
pub mod ray_propagation_visualizer;
mod reference;
mod source;
mod source_helper;
mod spectrometer;
mod spot_diagram;
mod wavefront;

pub use beam_splitter::BeamSplitter;
pub use detector::Detector;
pub use dummy::Dummy;
pub use group::{NodeGroup, PortMap};
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::Lens;
pub use paraxial_surface::ParaxialSurface;
pub use propagation::Propagation;
pub use reference::NodeReference;
pub use source::Source;
pub use source_helper::{
    collimated_line_ray_source, point_ray_source, round_collimated_ray_source,
};

pub use energy_meter::{EnergyMeter, Metertype};
pub use spectrometer::{Spectrometer, SpectrometerType};
pub use fluence_detector::{FluenceData, FluenceDetector};
pub use spot_diagram::SpotDiagram;
pub use ray_propagation_visualizer::RayPropagationVisualizer;
pub use wavefront::{WaveFront, WaveFrontData, WaveFrontErrorMap};
pub use node_attr::NodeAttr;
use uuid::Uuid;
use crate::{error::{OpmResult, OpossumError}, optic_ref::OpticRef};
use std::sync::{Arc, Mutex};

/// Factory function creating a new reference of an optical node of the given type.
///
/// If a uuid is given, the optical node is created using this id. Otherwise a new (random) id is generated. This
/// function is used internally during deserialization of an `OpticGraph`.
///
/// # Errors
///
/// This function will return an [`OpossumError`] if there is no node with the given type.
pub fn create_node_ref(node_type: &str, uuid: Option<Uuid>) -> OpmResult<OpticRef> {
    match node_type {
        "dummy" => Ok(OpticRef::new(Arc::new(Mutex::new(Dummy::default())), uuid)),
        "detector" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Detector::default())),
            uuid,
        )),
        "beam splitter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(BeamSplitter::default())),
            uuid,
        )),
        "energy meter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(EnergyMeter::default())),
            uuid,
        )),
        "group" => Ok(OpticRef::new(
            Arc::new(Mutex::new(NodeGroup::default())),
            uuid,
        )),
        "ideal filter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(IdealFilter::default())),
            uuid,
        )),
        "reference" => Ok(OpticRef::new(
            Arc::new(Mutex::new(NodeReference::default())),
            uuid,
        )),
        "lens" => Ok(OpticRef::new(Arc::new(Mutex::new(Lens::default())), uuid)),
        "light source" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Source::default())),
            uuid,
        )),
        "spectrometer" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Spectrometer::default())),
            uuid,
        )),
        "spot diagram" => Ok(OpticRef::new(
            Arc::new(Mutex::new(SpotDiagram::default())),
            uuid,
        )),
        "Wavefront monitor" => Ok(OpticRef::new(
            Arc::new(Mutex::new(WaveFront::default())),
            uuid,
        )),
        "propagation" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Propagation::default())),
            uuid,
        )),
        "paraxial" => Ok(OpticRef::new(
            Arc::new(Mutex::new(ParaxialSurface::default())),
            uuid,
        )),
        "ray propagation" => Ok(OpticRef::new(
            Arc::new(Mutex::new(RayPropagationVisualizer::default())),
            uuid,
        )),
        "fluence detector" => Ok(OpticRef::new(
            Arc::new(Mutex::new(FluenceDetector::default())),
            uuid,
        )),
        _ => Err(OpossumError::Other(format!(
            "cannot create node type <{node_type}>"
        ))),
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn create_node_ref_error() {
        assert!(create_node_ref("test", None).is_err());
    }
    #[test]
    fn create_dummy() {
        assert!(create_node_ref("dummy", None).is_ok());
        let id = Uuid::new_v4();
        let node = create_node_ref("dummy", Some(id));
        assert!(node.is_ok());
        let node = node.unwrap();
        assert_eq!(node.uuid(), id);
    }
}
