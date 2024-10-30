#![warn(missing_docs)]
//! This module contains the concrete node types (lenses, filters, etc...)

mod beam_splitter;
mod cylindric_lens;
mod detector;
mod dummy;
mod energy_meter;
mod ideal_filter;
mod lens;
mod node_attr;
mod node_group;
mod parabolic_mirror;
mod paraxial_surface;
mod reference;
mod source;
mod source_helper;
mod spectrometer;
mod spot_diagram;
mod test_helper;
mod thin_mirror;
mod wavefront;
mod wedge;

pub mod fluence_detector;
pub mod ray_propagation_visualizer;
pub mod reflective_grating;
pub use beam_splitter::BeamSplitter;
pub use cylindric_lens::CylindricLens;
pub use detector::Detector;
pub use dummy::Dummy;
pub use energy_meter::{EnergyMeter, Metertype};
pub use fluence_detector::{FluenceData, FluenceDetector};
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::Lens;
pub use node_attr::NodeAttr;
pub use node_group::{NodeGroup, OpticGraph};
pub use parabolic_mirror::ParabolicMirror;
pub use paraxial_surface::ParaxialSurface;
pub use ray_propagation_visualizer::RayPropagationVisualizer;
pub use reference::NodeReference;
pub use reflective_grating::ReflectiveGrating;
pub use source::Source;
pub use source_helper::{
    collimated_line_ray_source, point_ray_source, round_collimated_ray_source,
};
pub use spectrometer::{Spectrometer, SpectrometerType};
pub use spot_diagram::SpotDiagram;
use std::{cell::RefCell, rc::Rc};
pub use thin_mirror::ThinMirror;
pub use wavefront::{WaveFront, WaveFrontData, WaveFrontErrorMap};
pub use wedge::Wedge;

use crate::{
    error::{OpmResult, OpossumError},
    optic_ref::OpticRef,
};
use uuid::Uuid;
/// Factory function creating a new reference of an optical node of the given type.
///
/// If a uuid is given, the optical node is created using this id. Otherwise a new (random) id is generated. This
/// function is used internally during deserialization of an `OpticGraph`.
///
/// # Errors
///
/// This function will return an [`OpossumError`] if there is no node with the given type.
#[allow(clippy::too_many_lines)]
pub fn create_node_ref(node_type: &str, uuid: Option<Uuid>) -> OpmResult<OpticRef> {
    match node_type {
        "dummy" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Dummy::default())),
            uuid,
            None,
        )),
        "detector" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Detector::default())),
            uuid,
            None,
        )),
        "beam splitter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(BeamSplitter::default())),
            uuid,
            None,
        )),
        "energy meter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(EnergyMeter::default())),
            uuid,
            None,
        )),
        "group" => Ok(OpticRef::new(
            Rc::new(RefCell::new(NodeGroup::default())),
            uuid,
            None,
        )),
        "ideal filter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(IdealFilter::default())),
            uuid,
            None,
        )),
        "reflective grating" => Ok(OpticRef::new(
            Rc::new(RefCell::new(ReflectiveGrating::default())),
            uuid,
            None,
        )),
        "reference" => Ok(OpticRef::new(
            Rc::new(RefCell::new(NodeReference::default())),
            uuid,
            None,
        )),
        "lens" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Lens::default())),
            uuid,
            None,
        )),
        "cylindric lens" => Ok(OpticRef::new(
            Rc::new(RefCell::new(CylindricLens::default())),
            uuid,
            None,
        )),
        "source" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Source::default())),
            uuid,
            None,
        )),
        "spectrometer" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Spectrometer::default())),
            uuid,
            None,
        )),
        "spot diagram" => Ok(OpticRef::new(
            Rc::new(RefCell::new(SpotDiagram::default())),
            uuid,
            None,
        )),
        "wavefront monitor" => Ok(OpticRef::new(
            Rc::new(RefCell::new(WaveFront::default())),
            uuid,
            None,
        )),
        "paraxial surface" => Ok(OpticRef::new(
            Rc::new(RefCell::new(ParaxialSurface::default())),
            uuid,
            None,
        )),
        "ray propagation" => Ok(OpticRef::new(
            Rc::new(RefCell::new(RayPropagationVisualizer::default())),
            uuid,
            None,
        )),
        "fluence detector" => Ok(OpticRef::new(
            Rc::new(RefCell::new(FluenceDetector::default())),
            uuid,
            None,
        )),
        "wedge" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Wedge::default())),
            uuid,
            None,
        )),
        "mirror" => Ok(OpticRef::new(
            Rc::new(RefCell::new(ThinMirror::default())),
            uuid,
            None,
        )),
        "parabolic mirror" => Ok(OpticRef::new(
            Rc::new(RefCell::new(ParabolicMirror::default())),
            uuid,
            None,
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
