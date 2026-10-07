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
pub use thin_mirror::ThinMirror;
pub use wavefront::WaveFront;
pub use wavefront::wavefront_data::{WaveFrontData, WaveFrontMap};
pub use wedge::Wedge;

use crate::{
    analyzers::Analyzable,
    core_optics::{NodeAttr, OpticRef},
    error::{OpmResult, OpossumError},
    geometry::body::{CLEAR_APERTURE, default_clear_aperture},
    properties::validator::Validator,
};
use std::{collections::HashSet, sync::LazyLock};

/// Declare the property every node with a physical volume carries: how far its medium extends
/// transversally ([`CLEAR_APERTURE`]).
///
/// Declaring it from one place keeps the node types that call this in sync with each other, and
/// they are exactly those implementing [`Volumetric`](crate::core_optics::Volumetric) — which is
/// what the test in that module checks.
///
/// Creates standard volumetric properties for a node.
///
/// # Arguments
///
/// * `node_attr` - the attributes of the node under construction.
///
/// # Errors
///
/// This function returns an error if the property is already declared.
pub fn create_volume_properties(node_attr: &mut NodeAttr) -> OpmResult<()> {
    node_attr.create_property_with_validator(
        CLEAR_APERTURE,
        "transversal extent of the medium",
        Validator::ApertureDelimitsRegion,
        default_clear_aperture().into(),
    )
}

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
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::{OpticNode, OpticNodeExt, node_attr::NodePositioning},
        degree,
        geometry::SurfaceShape,
        millimeter,
        nodes::test_helper::test_helper::{
            assert_snapshots, body_snapshot, geometry_snapshot, surface_snapshot,
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
        spherical_wavefront.set_reference_surface(SurfaceShape::Sphere {
            radius: millimeter!(100.0),
        })?;
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
                surface_snapshot(&*node)?,
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
            if let Some(volume) = node.as_volume() {
                let frame = node.effective_node_iso().unwrap_or_else(Isometry::identity);
                actual.push((label, body_snapshot(&volume.volume_body()?, &frame)?));
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
                        "box: Opossum Error:Other:the extent of a body bounded by the curved 'cylindric' surface tilted against it is not supported",
                        "chord from (0, 0) at 0 deg: 10.000000",
                        "chord from (0, 0) at 10 deg: 10.154266",
                        "chord from (5, 0) at 0 deg: 9.949999",
                        "chord from (5, 0) at 10 deg: 10.103494",
                        "chord from (0, 5) at 0 deg: 10.000000",
                        "chord from (0, 5) at 10 deg: none",
                        "chord from (-8, 3) at 0 deg: 9.871992",
                        "chord from (-8, 3) at 10 deg: none",
                        "chord from (12.4, 0) at 0 deg: 9.692433",
                        "chord from (12.4, 0) at 10 deg: none",
                        "chord from (12.6, 0) at 0 deg: none",
                        "chord from (12.6, 0) at 10 deg: none",
                        "inside: ...............#####.........#####..#####..#####..#####..#####......................",
                    ],
                ),
                (
                    "inverted lens",
                    &[
                        "frame t=(10.406345, -17.828447, 30.345311) x=(0.964207, 0.204382, 0.168918) z=(-0.136689, -0.162745, 0.977154)",
                        "box: Opossum Error:Other:the extent of a body bounded by the curved 'sphere' surface tilted against it is not supported",
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
                        "box: Opossum Error:Other:the extent of a body bounded by the curved 'sphere' surface tilted against it is not supported",
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
                        "box: Opossum Error:Other:the extent of a body bounded by the curved 'sphere' surface tilted against it is not supported",
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
                .install_geometry(&["input_1"], &["output_1"])
                .is_err()
        );
    }

    /// Place a node (see [`place`]) and take its surface snapshot.
    fn placed_snapshot<T: OpticNode + ?Sized>(node: &mut T) -> OpmResult<Vec<String>> {
        place(node)?;
        surface_snapshot(node)
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
                        "in input_1: cylindric t=(0.000000, 0.000000, 500.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-500.000000, -499.974999, -500.000000) shared=0",
                        "out output_1: cylindric t=(0.000000, 0.000000, -490.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(500.000000, 499.974999, 500.000000) shared=1",
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
                        "in input_1: sphere t=(0.000000, 0.000000, 500.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-500.000000, -499.974999, -499.974999) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, -490.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(500.000000, 499.974999, 499.974999) shared=1",
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
                        "in input_1: sphere t=(0.000000, 0.000000, 50.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-50.000000, -49.749372, -49.749372) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, -75.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(80.000000, 79.843597, 79.843597) shared=1",
                    ],
                ),
                (
                    "plano-convex lens",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 50.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-50.000000, -49.749372, -49.749372) shared=0",
                        "out output_1: plane t=(0.000000, 0.000000, 5.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(0.000000, 0.000000, 0.000000) shared=1",
                    ],
                ),
                (
                    "spherical mirror",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 200.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-200.000000, -199.937490, -199.937490) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 200.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-200.000000, -199.937490, -199.937490) shared=0",
                    ],
                ),
                (
                    "spherical wavefront reference",
                    &[
                        "in input_1: sphere t=(0.000000, 0.000000, 100.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-100.000000, -99.874922, -99.874922) shared=0",
                        "out output_1: sphere t=(0.000000, 0.000000, 100.000000) x=(1.000000, 0.000000, 0.000000) z=(0.000000, 0.000000, 1.000000) sag=(-100.000000, -99.874922, -99.874922) shared=0",
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
