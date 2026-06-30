#![warn(missing_docs)]
//! # Optical nodes (e.g. lenses. filters, source, etc.)
//!
//! This module defines the core types and logic for optical nodes in the system.
//! Nodes represent optical elements, references to optical elements, or groups, and are organized
//! hierarchically. This modules provides creation, manipulation, and serialization of nodes.
//! Nodes can have input/output ports and are identified by UUIDs.
//!
//! To simplify the creation of new (custom) node types, the `OpmNode` derive macro is provided.
//! This macro automatically implements the `Analyzable`, `Alignable` and `LIDT` traits for the annotated struct.
//! Furthermore it allows to specify the color of the node in the `dot` file by using the `opm_node` attribute.
//!
//! # Example
//!
//! ```ignore
//! use opm_macros_lib::OpmNode;
//! use opossum_core::nodes::NodeAttr;
//!  
//! #[derive(OpmNode)]
//! #[opm_node("red")]
//! pub struct MyOpticNode {
//!    node_attr: NodeAttr
//! }
//! ```
mod beam_splitter;
mod cylindric_lens;
mod dummy;
mod energy_meter;
pub mod fluence_detector;
pub mod ideal_filter;
mod lens;
pub mod node_group;
mod parabolic_mirror;
mod paraxial_surface;
pub mod ray_propagation_visualizer;
mod reference;
pub mod reflective_grating;
mod source_helper;
mod source_port;
mod spectrometer;
mod spot_diagram;
mod test_helper;
mod thin_mirror;
mod wavefront;
mod wedge;
pub use beam_splitter::{BeamSplitter, SplittingConfig, SplittingConfigBuilder};
pub use cylindric_lens::CylindricLens;
pub use dummy::Dummy;
pub use energy_meter::{EnergyMeter, Metertype};
pub use fluence_detector::FluenceDetector;
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::Lens;
pub use node_group::{ConnectionInfo, NodeGroup, OpticGraph};
pub use parabolic_mirror::ParabolicMirror;
pub use paraxial_surface::ParaxialSurface;
pub use ray_propagation_visualizer::RayPropagationVisualizer;
pub use reference::NodeReference;
pub use reflective_grating::ReflectiveGrating;
pub use source_helper::{
    collimated_line_ray_builder, point_ray_builder, round_collimated_ray_builder,
};
pub use source_port::SourcePort;
pub use spectrometer::{Spectrometer, SpectrometerType};
pub use spot_diagram::SpotDiagram;
use std::sync::{Arc, Mutex};
pub use thin_mirror::ThinMirror;
pub use wavefront::WaveFront;
pub use wavefront::wavefront_data::{WaveFrontData, WaveFrontMap};
pub use wedge::Wedge;

use crate::{
    analyzers::Analyzable,
    core_optics::OpticRef,
    error::{OpmResult, OpossumError},
};

/// Struct to hold all info about a node type
pub struct NodeRegistration {
    name: &'static str,
    description: &'static str,
    constructor: fn() -> OpticRef,
}

impl NodeRegistration {
    /// Create a new node registration
    #[must_use]
    pub const fn new<T>(name: &'static str, description: &'static str) -> Self
    where
        T: Analyzable + Default + 'static,
    {
        Self {
            name,
            description,
            constructor: Self::build_node_wrapper::<T>,
        }
    }
    fn build_node_wrapper<T: Analyzable + Default + 'static>() -> OpticRef {
        OpticRef::new(Arc::new(Mutex::new(T::default())), None)
    }
}

inventory::collect!(NodeRegistration);

/// Factory function creating a new reference of an optical node of the given type.
///
/// If a uuid is given, the optical node is created using this id. Otherwise a new (random) id is generated. This
/// function is used internally during deserialization of an `OpticGraph`.
///
/// # Errors
///
/// This function will return an [`OpossumError`] if there is no node with the given type.
pub fn create_node_ref(node_type: &str) -> OpmResult<OpticRef> {
    // Wir iterieren durch das Inventory und suchen den passenden Namen.
    inventory::iter::<NodeRegistration>
        .into_iter()
        .find(|info| info.name == node_type)
        .map(|info| (info.constructor)())
        .ok_or_else(|| OpossumError::Other(format!("cannot create node type <{node_type}>")))
}
/// Return a list of all available node types.
///
/// Returns a vector of tuples containing the name and the description of all
/// available nodes in OPOSSUM.
/// **Note**: This function does not return he node type `reference` since there is a
/// separate endpoint for adding reference nodes.
#[must_use]
pub fn node_types() -> Vec<(&'static str, &'static str)> {
    inventory::iter::<NodeRegistration>
        .into_iter()
        .filter(|info| info.name != "reference")
        .map(|info| (info.name, info.description))
        .collect()
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
        for (node_type, _) in node_types() {
            assert!(create_node_ref(node_type).is_ok());
        }
    }
}
