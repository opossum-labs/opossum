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
pub(crate) mod test_helper;
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
pub use node_group::optic_graph::delta::{GraphDeletionDelta, GraphDelta, RemovedPortMapping};
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
pub use spot_diagram::{SpotDiagram, SpotDiagramData};
pub use thin_mirror::ThinMirror;
use uom::si::f64::Length;
pub use wavefront::WaveFront;
pub use wavefront::wavefront_data::{WaveFrontData, WaveFrontMap};
pub use wavefront::zernike::WavefrontDecomposition;
pub use wedge::Wedge;

use crate::{
    analyzers::Analyzable,
    core_optics::OpticRef,
    error::{OpmResult, OpossumError},
};
use std::{collections::HashSet, sync::LazyLock};

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
        OpticRef::new(Box::new(T::default()))
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
/// The node types that enclose a volume of material, derived from the node registry.
///
/// Every registered type is instantiated once and asked whether it is
/// [`Volumetric`](crate::core_optics::Volumetric). Doing that once is worth a `LazyLock`: it builds
/// one node of every type in the program, while the answer cannot change at runtime.
static VOLUME_NODE_TYPES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    node_types()
        .into_iter()
        .filter(|(node_type, _)| {
            create_node_ref(node_type).is_ok_and(|optic_ref| optic_ref.as_volume().is_some())
        })
        .map(|(node_type, _)| node_type)
        .collect()
});

/// Return whether nodes of the given type enclose a volume of material.
///
/// This is the question [`OpticNode::as_volume`](crate::core_optics::OpticNode::as_volume) answers,
/// for a caller that has only a type name and no node: a user interface deciding whether to offer
/// amplification for a component, for instance. It is derived from the very same capability, so the
/// two cannot drift apart.
///
/// # Arguments
///
/// * `node_type` - the node type name as it appears in the registry and in an `.opm` file.
///
/// # Returns
///
/// `true` if nodes of that type have a volume, `false` for any other type and for a name that is
/// not registered at all.
#[must_use]
pub fn is_volume_node_type(node_type: &str) -> bool {
    VOLUME_NODE_TYPES.contains(node_type)
}
pub trait EdgeThickness: Send + Sync {
    /// Computes the minimal edge thickness of the component at its clear rim.
    fn min_edge_thickness(&self) -> OpmResult<Length>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        coatings::{CoatingType, SplittingConfigBuilder},
        core_optics::{OpticNode, OpticNodeExt, SurfacePorts, node_attr::NodePositioning},
        degree,
        geometry::{Extruded, FaceIndex, Sphere, SurfaceGeometry, SurfaceShape},
        millimeter,
        nodes::test_helper::helper::{
            assert_snapshots, body_of, body_snapshot, geometry_snapshot, hit_snapshot,
            placed_geometry, surface_snapshot,
        },
        refractive_index::RefrIndexConst,
        utils::geom_transformation::Isometry,
    };

    /// One flat surface, placed at the node itself and shared by its input and output port.
    const FLAT_INPUT_OUTPUT: &[&str] = &[
        "in input_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
        "out output_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
    ];

    /// Place a node away from the origin, rotated about all three axes and with an alignment on
    /// top, so that every part of a surface's placement is exercised.
    fn place<T: OpticNode + ?Sized>(node: &mut T) -> OpmResult<()> {
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(10.0, -20.0, 30.0),
            degree!(5.0, -10.0, 15.0),
        )?))?;
        node.set_alignment(millimeter!(1.0, 2.0, 0.0), degree!(2.0, 0.0, -3.0))
    }

    /// Node configurations the default ones do not cover: a tilted exit surface, an off-axis
    /// parabola, an inverted lens, a flat lens surface, a curved mirror and a curved wavefront
    /// reference surface.
    fn node_variants() -> OpmResult<Vec<(&'static str, OpticRef)>> {
        let glass = RefrIndexConst::new(1.5)?;
        let mut inverted_lens = Lens::new(
            "inverted lens",
            millimeter!(50.0),
            millimeter!(-80.0),
            millimeter!(5.0),
            &glass,
        )?;
        inverted_lens.set_inverted(true)?;
        let mut spherical_wavefront = WaveFront::new("wavefront")?;
        spherical_wavefront
            .set_reference_surface(SurfaceShape::Sphere(Sphere::new(millimeter!(100.0))?))?;
        Ok(vec![
            (
                "5 degree wedge",
                OpticRef::new(Box::new(Wedge::new(
                    "wedge",
                    millimeter!(10.0),
                    degree!(5.0),
                    &glass,
                )?)),
            ),
            (
                "90 degree parabola",
                OpticRef::new(Box::new(ParabolicMirror::new_with_off_axis_y(
                    "parabola",
                    millimeter!(100.0),
                    true,
                    degree!(90.0),
                )?)),
            ),
            ("inverted lens", OpticRef::new(Box::new(inverted_lens))),
            (
                "plano-convex lens",
                OpticRef::new(Box::new(Lens::new(
                    "plano-convex lens",
                    millimeter!(50.0),
                    millimeter!(f64::INFINITY),
                    millimeter!(5.0),
                    &glass,
                )?)),
            ),
            (
                "spherical mirror",
                OpticRef::new(Box::new(
                    ThinMirror::new("mirror").with_curvature(millimeter!(200.0))?,
                )),
            ),
            (
                "spherical wavefront reference",
                OpticRef::new(Box::new(spherical_wavefront)),
            ),
        ])
    }

    /// Every node's geometry describes exactly the surfaces the node installs.
    #[test]
    fn every_geometry_describes_the_installed_surfaces() -> OpmResult<()> {
        for (label, node) in placed_configurations()? {
            assert_eq!(
                geometry_snapshot(&*node)?,
                surface_snapshot(&*node),
                "the geometry of '{label}' does not describe its surfaces"
            );
        }
        Ok(())
    }

    /// Everything gain and absorption read from the medium of every volume node configuration.
    ///
    /// Pins the body the medium is made of - frame, bounding box, chords of straight and oblique
    /// rays and which points it contains - for placed, rotated, aligned and inverted nodes.
    #[test]
    fn every_volume_body_is_recorded() -> OpmResult<()> {
        let mut actual = Vec::new();
        for (label, node) in placed_configurations()? {
            if node.as_volume().is_some() {
                let frame = node.effective_node_iso().unwrap_or_else(Isometry::identity);
                actual.push((
                    label,
                    body_snapshot(body_of(&placed_geometry(&*node)?)?, &frame)?,
                ));
            }
        }
        actual.sort_by_key(|(label, _)| *label);
        assert_snapshots(
            &actual,
            &[
                (
                    "5 degree wedge",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-12.500000, 12.500000] y=[-12.500000, 12.500000] z=[0.000000, 11.093608]",
                        "chord from (0, 0) at 0 deg: 10.000000",
                        "chord from (0, 0) at 10 deg: 11.108868",
                        "chord from (5, 0) at 0 deg: 10.000000",
                        "chord from (5, 0) at 10 deg: 11.108868",
                        "chord from (0, 5) at 0 deg: 10.437443",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 10.262466",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 10.000000",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ...............#####.........#####..######.#####..######.#####......................",
                    ],
                ),
                (
                    "cylindric lens",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-10.000000, 10.000000] y=[-15.000000, 15.000000] z=[0.000000, 10.000000]",
                        "chord from (0, 0) at 0 deg: 10.000000",
                        "chord from (0, 0) at 10 deg: 10.154266",
                        "chord from (5, 0) at 0 deg: 9.949999",
                        "chord from (5, 0) at 10 deg: 10.103494",
                        "chord from (0, 5) at 0 deg: 10.000000",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 9.871992",
                        "chord from (-8, 3) at 10 deg: 10.024283",
                        "chord from (12.4, 0) at 0 deg: none",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: .............................#####..#####..#####..#####.............................",
                    ],
                ),
                (
                    "inverted lens",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-12.500000, 12.500000] y=[-12.500000, 12.500000] z=[0.000000, 5.000000]",
                        "chord from (0, 0) at 0 deg: 5.000000",
                        "chord from (0, 0) at 10 deg: 3.668928",
                        "chord from (5, 0) at 0 deg: 4.592969",
                        "chord from (5, 0) at 10 deg: 3.244937",
                        "chord from (0, 5) at 0 deg: 4.592969",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 3.807033",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 2.471159",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ................#............###....###....###.....#......#.........................",
                    ],
                ),
                (
                    "lens",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-12.500000, 12.500000] y=[-12.500000, 12.500000] z=[0.000000, 10.000000]",
                        "chord from (0, 0) at 0 deg: 10.000000",
                        "chord from (0, 0) at 10 deg: 9.961841",
                        "chord from (5, 0) at 0 deg: 9.949999",
                        "chord from (5, 0) at 10 deg: 9.911074",
                        "chord from (0, 5) at 0 deg: 9.949999",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 9.853989",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 9.692433",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ...............#####.........#####..#####..#####..#####..#####......................",
                    ],
                ),
                (
                    "plano-convex lens",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-12.500000, 12.500000] y=[-12.500000, 12.500000] z=[0.000000, 5.000000]",
                        "chord from (0, 0) at 0 deg: 5.000000",
                        "chord from (0, 0) at 10 deg: 4.255460",
                        "chord from (5, 0) at 0 deg: 4.749372",
                        "chord from (5, 0) at 10 deg: 3.988103",
                        "chord from (0, 5) at 0 deg: 4.749372",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 4.264592",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 3.438002",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ................##...........###....###....###.....##.....##........................",
                    ],
                ),
                (
                    "wedge",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box x=[-12.500000, 12.500000] y=[-12.500000, 12.500000] z=[0.000000, 10.000000]",
                        "chord from (0, 0) at 0 deg: 10.000000",
                        "chord from (0, 0) at 10 deg: 10.154266",
                        "chord from (5, 0) at 0 deg: 10.000000",
                        "chord from (5, 0) at 10 deg: 10.154266",
                        "chord from (0, 5) at 0 deg: 10.000000",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 10.000000",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 10.000000",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ...............#####.........#####..#####..#####..#####..#####......................",
                    ],
                ),
            ],
        );
        Ok(())
    }

    /// Where rays meet every installed surface of every placed configuration, and whether the
    /// surface records them there.
    ///
    /// Pins what tracing sees - hit points, normals and misses - independently of how surfaces and
    /// their placement are represented, so it holds unchanged while that representation changes.
    #[test]
    fn every_installed_surface_is_hit_as_recorded() -> OpmResult<()> {
        let mut actual = Vec::new();
        for (label, node) in placed_configurations()? {
            actual.push((label, hit_snapshot(&*node)?));
        }
        actual.sort_by_key(|(label, _)| *label);
        assert_snapshots(
            &actual,
            &[
                (
                    "5 degree wedge",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 10.000000) n=(0.000000, 0.087156, -0.996195) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 10.745384, 10.940099) n=(0.000000, 0.087156, -0.996195) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 10.262466) n=(0.000000, 0.087156, -0.996195) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 10.000000) n=(0.000000, 0.087156, -0.996195) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "90 degree parabola",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, -0.707107, 0.707107) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 10.570953, 9.950856) n=(0.033623, -0.672457, 0.739372) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 2.645019) n=(-0.055739, -0.696733, 0.715162) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, -0.771778) n=(0.087681, -0.707107, 0.701649) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, -0.707107, 0.707107) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 10.570953, 9.950856) n=(0.033623, -0.672457, 0.739372) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 2.645019) n=(-0.055739, -0.696733, 0.715162) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, -0.771778) n=(0.087681, -0.707107, 0.701649) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "beam splitter",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "in input_2 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_2 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_2 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_2 from (0, 5) at 10 deg: miss",
                        "in input_2 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_2 from (12.6, 0) at 0 deg: miss",
                        "out out1_trans1_refl2 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out1_trans1_refl2 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out1_trans1_refl2 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out1_trans1_refl2 from (0, 5) at 10 deg: miss",
                        "out out1_trans1_refl2 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out1_trans1_refl2 from (12.6, 0) at 0 deg: miss",
                        "out out2_trans2_refl1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out2_trans2_refl1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out2_trans2_refl1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out2_trans2_refl1 from (0, 5) at 10 deg: miss",
                        "out out2_trans2_refl1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out out2_trans2_refl1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "cylindric lens",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.820757, 0.025001) n=(0.010000, 0.000000, -0.999950) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.064004) n=(-0.016000, 0.000000, -0.999872) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.4, 0) at 0 deg: miss",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 10.575211, 9.974999) n=(-0.010000, 0.000000, -0.999950) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 9.935996) n=(0.016000, 0.000000, -0.999872) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: miss",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "dummy",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                    ],
                ),
                (
                    "energy meter",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                    ],
                ),
                (
                    "fluence detector",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                    ],
                ),
                ("group", &[]),
                (
                    "ideal filter",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "inverted lens",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 9.005457, 1.072485) n=(0.100000, 0.180109, -0.978550) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.735408) n=(-0.160000, 0.060000, -0.985292) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 1.561998) n=(0.248000, 0.000000, -0.968760) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 5.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 9.568934, 4.268124) n=(-0.062500, -0.119612, -0.990852) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 4.542442) n=(0.100000, -0.037500, -0.994281) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 4.033158) n=(-0.155000, 0.000000, -0.987914) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "lens",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.834521, 0.103059) n=(0.010000, 0.017669, -0.999794) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.073005) n=(-0.016000, 0.006000, -0.999854) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.153784) n=(0.024800, 0.000000, -0.999692) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 10.555561, 9.863562) n=(-0.010000, -0.021111, -0.999727) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 9.926995) n=(0.016000, -0.006000, -0.999854) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 9.846216) n=(-0.024800, 0.000000, -0.999692) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "mirror",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "parabolic mirror",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, 1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.811824, -0.025662) n=(0.002500, 0.004406, 0.999987) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, -0.018250) n=(-0.004000, 0.001500, 0.999991) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, -0.038440) n=(0.006200, 0.000000, 0.999981) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, 1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.811824, -0.025662) n=(0.002500, 0.004406, 0.999987) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, -0.018250) n=(-0.004000, 0.001500, 0.999991) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, -0.038440) n=(0.006200, 0.000000, 0.999981) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "paraxial surface",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "plano-convex lens",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 9.005457, 1.072485) n=(0.100000, 0.180109, -0.978550) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.735408) n=(-0.160000, 0.060000, -0.985292) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 1.561998) n=(0.248000, 0.000000, -0.968760) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 5.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 9.697984, 5.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 5.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 5.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "ray propagation",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                    ],
                ),
                ("reference", &[]),
                (
                    "reflective grating",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "source port",
                    &[
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                    ],
                ),
                (
                    "spectrometer",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                    ],
                ),
                (
                    "spherical mirror",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.862019, 0.259006) n=(0.025000, 0.044310, -0.998705) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.182583) n=(-0.040000, 0.015000, -0.999087) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.384770) n=(0.062000, 0.000000, -0.998076) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.862019, 0.259006) n=(0.025000, 0.044310, -0.998705) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.182583) n=(-0.040000, 0.015000, -0.999087) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.384770) n=(0.062000, 0.000000, -0.998076) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
                (
                    "spherical wavefront reference",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.908601, 0.523184) n=(0.050000, 0.089086, -0.994768) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.365669) n=(-0.080000, 0.030000, -0.996343) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.989749, 0.983401) n=(0.000000, 0.139897, -0.990166) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.771778) n=(0.124000, 0.000000, -0.992282) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.796976) n=(0.126000, 0.000000, -0.992030) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.908601, 0.523184) n=(0.050000, 0.089086, -0.994768) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.365669) n=(-0.080000, 0.030000, -0.996343) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.989749, 0.983401) n=(0.000000, 0.139897, -0.990166) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.771778) n=(0.124000, 0.000000, -0.992282) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.796976) n=(0.126000, 0.000000, -0.992030) not recorded",
                    ],
                ),
                (
                    "spot diagram",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                    ],
                ),
                (
                    "wavefront monitor",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: (0.000000, 13.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: (12.600000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) not recorded",
                    ],
                ),
                (
                    "wedge",
                    &[
                        "in input_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (5, 0) at 10 deg: (5.000000, 8.816349, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (0, 5) at 10 deg: miss",
                        "in input_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 0.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "in input_1 from (12.6, 0) at 0 deg: miss",
                        "out output_1 from (0, 0) at 0 deg: (0.000000, 0.000000, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (5, 0) at 10 deg: (5.000000, 10.579619, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (-8, 3) at 0 deg: (-8.000000, 3.000000, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (0, 5) at 10 deg: miss",
                        "out output_1 from (12.4, 0) at 0 deg: (12.400000, 0.000000, 10.000000) n=(0.000000, 0.000000, -1.000000) recorded",
                        "out output_1 from (12.6, 0) at 0 deg: miss",
                    ],
                ),
            ],
        );
        Ok(())
    }

    /// Every registered node type in its default configuration, the reference node and the
    /// [`node_variants`], each placed (see [`place`]).
    fn placed_configurations() -> OpmResult<Vec<(&'static str, OpticRef)>> {
        let mut nodes = node_variants()?;
        for (node_type, _) in node_types() {
            nodes.push((node_type, create_node_ref(node_type)?));
        }
        nodes.push(("reference", create_node_ref("reference")?));
        for (_, node) in &mut nodes {
            place(&mut **node)?;
        }
        Ok(nodes)
    }

    /// A node without a shape of its own has no surfaces to install.
    #[test]
    fn a_node_without_geometry_cannot_install_surfaces() {
        assert!(
            NodeGroup::default()
                .install_geometry(&SurfacePorts::SINGLE_SURFACE)
                .is_err()
        );
    }

    /// A port sits on a face the geometry has: a lens has two, a single surface one. The error
    /// names the node, the port and the number of faces.
    #[test]
    fn a_port_cannot_sit_on_a_face_the_geometry_does_not_have() {
        const BEHIND_THE_LENS: SurfacePorts = SurfacePorts {
            inputs: &[("input_1", Extruded::FRONT)],
            outputs: &[("output_1", FaceIndex::new(2))],
            splitting: &[],
        };
        const BEHIND_THE_SURFACE: SurfacePorts = SurfacePorts {
            inputs: &[("input_1", SurfaceGeometry::FACE)],
            outputs: &[("output_1", FaceIndex::new(1))],
            splitting: &[],
        };
        let mut lens = Lens::default();
        let error = lens
            .install_geometry(&BEHIND_THE_LENS)
            .unwrap_err()
            .to_string();
        assert!(error.contains("output_1"), "{error}");
        assert!(error.contains("face 2"), "{error}");
        assert!(error.contains("face count: 2"), "{error}");

        let mut dummy = Dummy::default();
        let error = dummy
            .install_geometry(&BEHIND_THE_SURFACE)
            .unwrap_err()
            .to_string();
        assert!(error.contains("face 1"), "{error}");
        assert!(error.contains("face count: 1"), "{error}");
    }

    /// Every surface of every node type takes a separator coating exactly when it splits the
    /// light, and any other coating exactly when it does not. The beam splitter's surface splits.
    #[test]
    fn every_surface_takes_a_separator_exactly_when_it_splits() -> OpmResult<()> {
        let separator: CoatingType = SplittingConfigBuilder::default().into();
        for (node_type, _) in node_types() {
            let mut node = create_node_ref(node_type)?;
            let Some(geometry) = node.create_geometry()? else {
                continue;
            };
            let splitting = node.surface_ports().splitting;
            for face in (0..geometry.kind().face_count()).map(FaceIndex::new) {
                let splits = splitting.contains(&face);
                assert_eq!(
                    node.set_coating(face, &separator).is_ok(),
                    splits,
                    "{node_type}, face {face}"
                );
                assert_eq!(
                    node.set_coating(face, &CoatingType::IdealAR).is_ok(),
                    !splits,
                    "{node_type}, face {face}"
                );
            }
        }
        assert_eq!(
            BeamSplitter::default().surface_ports().splitting,
            [SurfaceGeometry::FACE]
        );
        Ok(())
    }

    /// Place a node (see [`place`]) and take its surface snapshot.
    fn placed_snapshot<T: OpticNode + ?Sized>(node: &mut T) -> OpmResult<Vec<String>> {
        place(node)?;
        Ok(surface_snapshot(node))
    }

    /// The surfaces every registered node type installs in its default configuration.
    ///
    /// Pins today's hand-written `update_surfaces` of every node type, so that describing their
    /// geometry generically cannot change where a surface ends up.
    #[test]
    fn every_node_type_installs_its_recorded_surfaces() -> OpmResult<()> {
        let mut actual = Vec::new();
        for (node_type, _) in node_types() {
            let mut node = create_node_ref(node_type)?;
            actual.push((node_type, placed_snapshot(&mut *node)?));
        }
        actual.sort_by_key(|(node_type, _)| *node_type);
        assert_snapshots(
            &actual,
            &[
                (
                    "beam splitter",
                    &[
                        "in input_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                        "in input_2: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                        "out out1_trans1_refl2: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                        "out out2_trans2_refl1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                    ],
                ),
                (
                    "cylindric lens",
                    &[
                        "in input_1: cylindric t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.025001, 0.000000) shared=0",
                        "out output_1: cylindric t=(0.000000, 0.000000, 10.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, -0.025001, 0.000000) shared=1",
                    ],
                ),
                ("dummy", FLAT_INPUT_OUTPUT),
                ("energy meter", FLAT_INPUT_OUTPUT),
                ("fluence detector", FLAT_INPUT_OUTPUT),
                ("group", &[]),
                ("ideal filter", FLAT_INPUT_OUTPUT),
                (
                    "lens",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.025001, 0.025001) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 10.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, -0.025001, -0.025001) shared=1",
                    ],
                ),
                ("mirror", FLAT_INPUT_OUTPUT),
                (
                    "parabolic mirror",
                    &[
                        "in input_1: parabolic t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, -0.006250, -0.006250) shared=0",
                        "out output_1: parabolic t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, -0.006250, -0.006250) shared=0",
                    ],
                ),
                ("paraxial surface", FLAT_INPUT_OUTPUT),
                ("ray propagation", FLAT_INPUT_OUTPUT),
                ("reflective grating", FLAT_INPUT_OUTPUT),
                (
                    "source port",
                    &[
                        "out output_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                    ],
                ),
                ("spectrometer", FLAT_INPUT_OUTPUT),
                ("spot diagram", FLAT_INPUT_OUTPUT),
                ("wavefront monitor", FLAT_INPUT_OUTPUT),
                (
                    "wedge",
                    &[
                        "in input_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                        "out output_1: plane t=(0.000000, 0.000000, 10.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=1",
                    ],
                ),
            ],
        );
        Ok(())
    }

    /// The surfaces of node configurations the default ones do not cover: a tilted exit surface,
    /// a curved mirror, a flat lens surface, an inverted lens and an off-axis parabola.
    #[test]
    fn node_variants_install_their_recorded_surfaces() -> OpmResult<()> {
        let mut actual = Vec::new();
        for (label, mut node) in node_variants()? {
            actual.push((label, placed_snapshot(&mut *node)?));
        }
        actual.sort_by_key(|(label, _)| *label);
        assert_snapshots(
            &actual,
            &[
                (
                    "5 degree wedge",
                    &[
                        "in input_1: plane t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=0",
                        "out output_1: plane t=(0.000000, 0.000000, 10.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, -0.087156, 0.996195) sag=(0.000000, 0.000000, 0.000000) shared=1",
                    ],
                ),
                (
                    "90 degree parabola",
                    &[
                        "in input_1: parabolic t=(0.000000, -50.000000, -100.000000) x=(0.000000, 0.000000, -1.000000) z=(0.000000, -1.000000, 0.000000) sag=(0.000000, -0.125000, -0.125000) shared=0",
                        "out output_1: parabolic t=(0.000000, -50.000000, -100.000000) x=(0.000000, 0.000000, -1.000000) z=(0.000000, -1.000000, 0.000000) sag=(0.000000, -0.125000, -0.125000) shared=0",
                    ],
                ),
                (
                    "inverted lens",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.250628, 0.250628) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 5.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, -0.156403, -0.156403) shared=1",
                    ],
                ),
                (
                    "plano-convex lens",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.250628, 0.250628) shared=0",
                        "out output_1: plane t=(0.000000, 0.000000, 5.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=1",
                    ],
                ),
                (
                    "spherical mirror",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.062510, 0.062510) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.062510, 0.062510) shared=0",
                    ],
                ),
                (
                    "spherical wavefront reference",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.125078, 0.125078) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 0.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.125078, 0.125078) shared=0",
                    ],
                ),
            ],
        );
        Ok(())
    }
    #[test]
    fn create_node_ref_error() {
        assert!(create_node_ref("test").is_err());
    }
    #[test]
    fn only_nodes_with_a_volume_are_volume_node_types() {
        assert!(is_volume_node_type("lens"));
        assert!(is_volume_node_type("wedge"));
        assert!(is_volume_node_type("cylindric lens"));
        assert!(!is_volume_node_type("dummy"));
        assert!(!is_volume_node_type("does not exist"));
    }
    #[test]
    fn create_node_ref_ok() {
        for (node_type, _) in node_types() {
            assert!(create_node_ref(node_type).is_ok());
        }
    }
}
