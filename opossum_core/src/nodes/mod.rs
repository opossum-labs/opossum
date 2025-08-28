#![warn(missing_docs)]
//! This module contains the concrete node types (lenses, filters, etc...)
//!
//! To simplify the creation of new node types, the `OpmNode` derive macro is provided.
//! This macro automatically implements the `Analyzable`, `Alignable` and `LIDT` traits for the annotated struct.
//! Furthermore it allows to specify the color of the node in the `dot` file by using the `opm_node` attribute.
//!
//! # Example
//!
//! ```ignore
//! use opm_macros_lib::OpmNode;
//! use opossum::nodes::NodeAttr;
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
pub use beam_splitter::{BeamSplitter, SplittingConfig, SplittingConfigBuilder};
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
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};
pub use wedge::Wedge;

use crate::{
    error::{OpmResult, OpossumError},
    optic_ref::OpticRef,
};
// A type alias for the node constructor function
type NodeConstructor = Box<dyn Fn() -> OpticRef + Send + Sync>;

// Struct to hold all info about a node type
struct NodeInfo {
    constructor: NodeConstructor,
    description: &'static str,
}

// Create a node factory as single point of truth.
// Create a lazily-initialized static HashMap.
static NODE_FACTORY: LazyLock<HashMap<&'static str, NodeInfo>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    // A little helper macro to reduce boilerplate when adding nodes.
    macro_rules! register_node {
        ($map:expr, $name:expr, $type:ty, $desc:expr) => {
            $map.insert(
                $name,
                NodeInfo {
                    constructor: Box::new(|| {
                        OpticRef::new(Arc::new(Mutex::new(<$type>::default())), None)
                    }),
                    description: $desc,
                },
            );
        };
    }

    register_node!(map, "dummy", Dummy, "dummy node");
    register_node!(map, "beam splitter", BeamSplitter, "ideal beam splitter");
    register_node!(map, "energy meter", EnergyMeter, "ideal energy meter");
    register_node!(
        map,
        "group",
        NodeGroup,
        "group node containing other nodes or groups"
    );
    register_node!(map, "ideal filter", IdealFilter, "ideal filter");
    register_node!(
        map,
        "reflective grating",
        ReflectiveGrating,
        "reflective optical grating"
    );
    register_node!(map, "reference", NodeReference, "reference to another node");
    register_node!(map, "lens", Lens, "spherical lens");
    register_node!(map, "cylindric lens", CylindricLens, "cylindric lens");
    register_node!(map, "source", Source, "light source");
    register_node!(map, "spectrometer", Spectrometer, "ideal spectrometer");
    register_node!(map, "spot diagram", SpotDiagram, "spot diagram detector");
    register_node!(map, "wavefront monitor", WaveFront, "wavefront detector");
    register_node!(map, "paraxial surface", ParaxialSurface, "ideal thin lens");
    register_node!(
        map,
        "ray propagation",
        RayPropagationVisualizer,
        "ray propagation plotter"
    );
    register_node!(map, "fluence detector", FluenceDetector, "fluence detector");
    register_node!(map, "wedge", Wedge, "wedged substrate (prism)");
    register_node!(map, "mirror", ThinMirror, "ideal flat / spherical mirror");
    register_node!(map, "parabolic mirror", ParabolicMirror, "parabolic mirror");

    map
});

/// Factory function creating a new reference of an optical node of the given type.
///
/// If a uuid is given, the optical node is created using this id. Otherwise a new (random) id is generated. This
/// function is used internally during deserialization of an `OpticGraph`.
///
/// # Errors
///
/// This function will return an [`OpossumError`] if there is no node with the given type.
pub fn create_node_ref(node_type: &str) -> OpmResult<OpticRef> {
    NODE_FACTORY
        .get(node_type)
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
    NODE_FACTORY
        .iter()
        .filter(|(name, _)| **name != "reference") // Filter out "reference" as in the original
        .map(|(name, info)| (*name, info.description))
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
        // Test against the keys in our factory map, which is now the single source of truth.
        for node_type in NODE_FACTORY.keys() {
            assert!(create_node_ref(node_type).is_ok());
        }
    }
}
