#![warn(missing_docs)]
//! This module contains the concrete node types (lenses, filters, etc...)

mod beam_splitter;
mod cylindric_lens;
mod dummy;
mod energy_meter;
pub mod fluence_detector;
mod ideal_filter;
mod lens;
mod node_attr;
mod node_group;
mod parabolic_mirror;
mod paraxial_surface;
pub mod ray_propagation_visualizer;
mod reference;
pub mod reflective_grating;
mod source;
mod source_helper;
mod spectrometer;
mod spot_diagram;
mod test_helper;
mod thin_mirror;
mod wavefront;
mod wedge;
pub use beam_splitter::BeamSplitter;
pub use cylindric_lens::CylindricLens;
pub use dummy::Dummy;
pub use energy_meter::{EnergyMeter, Metertype};
pub use fluence_detector::FluenceDetector;
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::Lens;
pub use node_attr::NodeAttr;
pub use node_group::{NodeGroup, OpticGraph};
pub use parabolic_mirror::ParabolicMirror;
pub use paraxial_surface::ParaxialSurface;
pub use ray_propagation_visualizer::RayPropagationVisualizer;
pub use reference::NodeReference;
pub use reflective_grating::ReflectiveGrating;
pub use spectrometer::{Spectrometer, SpectrometerType};
pub use thin_mirror::ThinMirror;
pub use wavefront::{WaveFront, WaveFrontData, WaveFrontErrorMap};

pub use source::Source;
pub use source_helper::{
    collimated_line_ray_source, point_ray_source, round_collimated_ray_source,
};
pub use spot_diagram::SpotDiagram;
use std::sync::{Arc, Mutex};
pub use wedge::Wedge;

use crate::{
    error::{OpmResult, OpossumError},
    optic_ref::OpticRef,
};
/// Factory function creating a new reference of an optical node of the given type.
///
/// If a uuid is given, the optical node is created using this id. Otherwise a new (random) id is generated. This
/// function is used internally during deserialization of an `OpticGraph`.
///
/// # Errors
///
/// This function will return an [`OpossumError`] if there is no node with the given type.
#[allow(clippy::too_many_lines)]
pub fn create_node_ref(node_type: &str) -> OpmResult<OpticRef> {
    match node_type {
        "dummy" => Ok(OpticRef::new(Arc::new(Mutex::new(Dummy::default())), None)),
        "beam splitter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(BeamSplitter::default())),
            None,
        )),
        "energy meter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(EnergyMeter::default())),
            None,
        )),
        "group" => Ok(OpticRef::new(
            Arc::new(Mutex::new(NodeGroup::default())),
            None,
        )),
        "ideal filter" => Ok(OpticRef::new(
            Arc::new(Mutex::new(IdealFilter::default())),
            None,
        )),
        "reflective grating" => Ok(OpticRef::new(
            Arc::new(Mutex::new(ReflectiveGrating::default())),
            None,
        )),
        "reference" => Ok(OpticRef::new(
            Arc::new(Mutex::new(NodeReference::default())),
            None,
        )),
        "lens" => Ok(OpticRef::new(Arc::new(Mutex::new(Lens::default())), None)),
        "cylindric lens" => Ok(OpticRef::new(
            Arc::new(Mutex::new(CylindricLens::default())),
            None,
        )),
        "source" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Source::default())),
            None,
        )),
        "spectrometer" => Ok(OpticRef::new(
            Arc::new(Mutex::new(Spectrometer::default())),
            None,
        )),
        "spot diagram" => Ok(OpticRef::new(
            Arc::new(Mutex::new(SpotDiagram::default())),
            None,
        )),
        "wavefront monitor" => Ok(OpticRef::new(
            Arc::new(Mutex::new(WaveFront::default())),
            None,
        )),
        "paraxial surface" => Ok(OpticRef::new(
            Arc::new(Mutex::new(ParaxialSurface::default())),
            None,
        )),
        "ray propagation" => Ok(OpticRef::new(
            Arc::new(Mutex::new(RayPropagationVisualizer::default())),
            None,
        )),
        "fluence detector" => Ok(OpticRef::new(
            Arc::new(Mutex::new(FluenceDetector::default())),
            None,
        )),
        "wedge" => Ok(OpticRef::new(Arc::new(Mutex::new(Wedge::default())), None)),
        "mirror" => Ok(OpticRef::new(
            Arc::new(Mutex::new(ThinMirror::default())),
            None,
        )),
        "parabolic mirror" => Ok(OpticRef::new(
            Arc::new(Mutex::new(ParabolicMirror::default())),
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
        assert!(create_node_ref("test").is_err());
    }
    #[test]
    fn create_node_ref_ok() {
        let node_types = vec![
            "dummy",
            "beam splitter",
            "energy meter",
            "group",
            "ideal filter",
            "reflective grating",
            "reference",
            "lens",
            "cylindric lens",
            "source",
            "spectrometer",
            "spot diagram",
            "wavefront monitor",
            "paraxial surface",
            "ray propagation",
            "fluence detector",
            "wedge",
            "mirror",
            "parabolic mirror",
        ];
        for node_type in node_types {
            assert!(create_node_ref(node_type).is_ok());
        }
    }
}
