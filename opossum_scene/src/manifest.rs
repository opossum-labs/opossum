//! The list of a model's drawable components, without their geometry.

use opossum_core::{
    core_optics::NodeAttrExt,
    error::OpmResult,
    nodes::NodeGroup,
    types::api_types::{SceneManifest, SceneNodeEntry},
};
use uom::si::f64::Length;

use crate::{
    components::{Drawn, for_each_drawable, placement_of},
    hash::Fnv1a,
};

/// List every drawable component of a model with its placement, but without its geometry.
///
/// This is the half of a 3D view that is cheap to repeat. A viewer asks for it whenever the model
/// changed and compares it with what it already shows: a component whose `geometry` is unchanged
/// only needs its placement applied, however far it moved, and only one whose shape really changed
/// has its file fetched again.
///
/// The same walk as [`scene_glb`](crate::scene_glb) decides what is in here, so the manifest never
/// names a component that cannot be drawn.
///
/// # Arguments
///
/// - `placed`: a scenery whose nodes carry their placement
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Returns
///
/// One entry per drawable component, in world coordinates.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked.
// Placement is narrowed to `f32` on the way out. Nothing is computed from it - it positions a
// picture - and `f32` still resolves a metre to well under a micrometre; see
// [`SceneNodeEntry::position`].
#[allow(clippy::cast_possible_truncation)]
pub fn manifest_of(placed: &NodeGroup, wavelength: Length) -> OpmResult<SceneManifest> {
    let mut nodes = Vec::new();
    let skipped = for_each_drawable(placed, wavelength, |drawn| {
        let uid = drawn.node.node_attr().uuid();
        let name = drawn.node.name().to_string();
        let transform = placement_of(drawn.node);
        let geometry = geometry_of(&drawn);
        let position = transform.translation.vector;
        let rotation = transform.rotation.quaternion();
        nodes.push(SceneNodeEntry {
            uid,
            group: placed.parent_group_id_or_self(uid)?,
            name,
            geometry,
            position: [position.x as f32, position.y as f32, position.z as f32],
            rotation: [
                rotation.i as f32,
                rotation.j as f32,
                rotation.k as f32,
                rotation.w as f32,
            ],
        });
        Ok(())
    })?;
    Ok(SceneManifest {
        nodes,
        skipped,
        ..SceneManifest::default()
    })
}

/// What a component looks like, as a short string that changes when its shape or material does.
///
/// Deliberately *not* a hash of the component's exported file. That file carries the node's uuid and
/// name, so two identical lenses would never agree on one, and a component rebuilt with a fresh uuid
/// would look reshaped although nothing about it moved or changed. What a viewer has to decide is
/// whether to fetch the geometry again, and that is a question about shape and material alone.
///
/// # Arguments
///
/// - `drawn`: the component, as it is drawn
///
/// # Returns
///
/// A hexadecimal digest of the component's patches and their materials.
fn geometry_of(drawn: &Drawn<'_>) -> String {
    let mut hash = Fnv1a::new();
    for (mesh, material) in &drawn.patches {
        hash.write_mesh(mesh);
        hash.write_material(material);
    }
    hash.finish()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        appearance::Appearance,
        test_helpers::{a_bit_of_everything, drawn_components, exported, two_lenses},
    };
    use opossum_core::{
        analyzers::{AnalyzerType, RayTraceConfig},
        apertures::{Bore, CircleShape, OpticShape, Outline},
        coatings::CoatingType,
        core_optics::{
            NodeAttr, OpticNodeExt,
            node_attr::{HasNodeAttr, NodePositioning},
        },
        degree,
        geometry::{SurfaceGeometry, body::OPTIC_SHAPE},
        light::lightdata::ray_data_builder::RayDataBuilder,
        material::default_reference_wavelength,
        millimeter,
        nodes::{BoredMirror, Dummy, EnergyMeter, Lens, SourcePort, ThinMirror, Wedge},
        opm_document::OpmDocument,
        utils::geom_transformation::Isometry,
    };

    /// A changed coating changes how a component looks, so the viewer has to fetch it again.
    #[test]
    fn changing_a_coating_changes_the_geometry() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        let wavelength = default_reference_wavelength();
        let appearance = Appearance::of_node_type(mirror.node_type())?.expect("a mirror is drawn");
        let before = geometry_of(&Drawn::new(&mirror, appearance, wavelength)?);
        mirror.set_coating(SurfaceGeometry::FACE, &CoatingType::IdealAR)?;
        let after = geometry_of(&Drawn::new(&mirror, appearance, wavelength)?);
        assert_ne!(before, after);
        Ok(())
    }

    /// The manifest's geometry hash for a surface node reacts to its clear aperture exactly as a
    /// volume node's does to its curvature or thickness - the other half of the property this
    /// module reads to mesh a surface at all.
    #[test]
    fn changing_a_surfaces_aperture_changes_its_geometry() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let before = manifest(&document)?;
        assert_eq!(before.nodes.len(), 1);

        let smaller = OpticShape::Bounded(Outline::new(CircleShape::new(millimeter!(5.0))?, None)?);
        document.scenery_mut().with_node_attr_mut(mirror, |attr| {
            attr.set_property(OPTIC_SHAPE, smaller.into())
        })??;

        let after = manifest(&document)?;
        assert_eq!(after.nodes.len(), 1);
        assert_ne!(
            before.nodes[0].geometry, after.nodes[0].geometry,
            "shrinking the mirror's clear aperture did not change its reported geometry"
        );
        Ok(())
    }

    /// A bored mirror whose bore lies beside the optical axis is drawn like any other component
    /// rather than left out.
    #[test]
    fn a_mirror_with_a_bore_is_drawn() -> OpmResult<()> {
        let bored = OpticShape::Bounded(Outline::new(
            CircleShape::new(millimeter!(12.5))?,
            Some(Bore::new(
                CircleShape::new(millimeter!(2.))?,
                millimeter!(6.0, 0.0),
                degree!(0.),
            )),
        )?);
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(BoredMirror::default())?;
        let detector = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(
            mirror,
            "out2_trans2_refl1",
            detector,
            "input_1",
            millimeter!(50.0),
        )?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        document
            .scenery_mut()
            .with_node_attr_mut(mirror, |attr| attr.set_property(OPTIC_SHAPE, bored.into()))??;

        let manifest = manifest(&document)?;
        assert!(manifest.skipped.is_empty(), "{:?}", manifest.skipped);
        assert!(
            manifest.nodes.iter().any(|node| node.uid == mirror),
            "the mirror with a bore is missing from the scene"
        );
        Ok(())
    }

    /// The list of components of a model, as a viewer would ask for it.
    fn manifest(document: &OpmDocument) -> OpmResult<SceneManifest> {
        manifest_of(
            &document.placed_scenery(None)?,
            default_reference_wavelength(),
        )
    }

    /// The whole point of splitting placement from geometry: a component that only moved keeps its
    /// file, so a viewer showing it has to be told a new position and nothing else. If this breaks,
    /// every drag of a component re-downloads and re-parses its mesh.
    #[test]
    fn moving_a_component_leaves_its_geometry_alone() -> OpmResult<()> {
        let near = manifest(&two_lenses(millimeter!(100.0))?)?;
        let far = manifest(&two_lenses(millimeter!(250.0))?)?;

        let moved = &far.nodes[far.nodes.len() - 1];
        let before = &near.nodes[near.nodes.len() - 1];
        assert_eq!(
            before.geometry, moved.geometry,
            "moving a component changed its geometry, so its mesh would be fetched again"
        );
        let travelled = (0..3)
            .map(|axis| (before.position[axis] - moved.position[axis]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            travelled > 1e-6,
            "the component was supposed to have moved, but shifted by {travelled} m"
        );
        Ok(())
    }

    /// The other half of that bargain: a component whose shape really changed must not keep the
    /// file of its old shape, or the view would silently go stale.
    #[test]
    fn reshaping_a_component_changes_its_geometry() -> OpmResult<()> {
        let plain = manifest(&two_lenses(millimeter!(100.0))?)?;

        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let first = scenery.add_node(Lens::default())?;
        let second = scenery.add_node(Wedge::default())?;
        scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(first, "output_1", second, "input_1", millimeter!(100.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        let reshaped = manifest(&document)?;

        assert_ne!(
            plain.nodes[plain.nodes.len() - 1].geometry,
            reshaped.nodes[reshaped.nodes.len() - 1].geometry,
            "a different component was given the same geometry"
        );
        Ok(())
    }

    /// Components of the same shape share one file, so a setup built from twenty identical lenses
    /// fetches one mesh rather than twenty.
    #[test]
    fn identical_components_share_a_geometry() -> OpmResult<()> {
        let manifest = manifest(&two_lenses(millimeter!(100.0))?)?;

        assert_eq!(manifest.nodes.len(), 2);
        assert_eq!(manifest.nodes[0].geometry, manifest.nodes[1].geometry);
        Ok(())
    }

    /// A component inside a nested group is listed with that group's uuid, not the root's. The 3D
    /// view needs the exact graph a click has to reveal the node in, and a subgroup is the case
    /// [`NodeGroup::parent_group_id_or_self`] has to recurse for rather than answering with the root.
    #[test]
    fn a_component_inside_a_group_is_listed_with_that_group() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;

        let mut inner = NodeGroup::new("inner");
        let lens = inner.add_node(Lens::default())?;
        inner.map_input_port(lens, "input_1", "input_1")?;
        inner.map_output_port(lens, "output_1", "output_1")?;
        let group_id = scenery.add_node(inner)?;

        scenery.connect_nodes(source, "output_1", group_id, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let manifest = manifest(&document)?;

        assert_eq!(manifest.nodes.len(), 1);
        assert_eq!(manifest.nodes[0].group, group_id);
        Ok(())
    }

    /// A drawable component that was not positioned (not connected to any source) is listed in
    /// `skipped` rather than silently absent, so the user knows which component is missing and why.
    #[test]
    fn a_drawable_component_without_placement_is_listed_as_skipped() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let connected = scenery.add_node(Lens::default())?;
        let unconnected = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", connected, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let m = manifest(&document)?;

        assert_eq!(m.nodes.len(), 1, "only the connected lens is drawn");
        assert_eq!(
            m.skipped.len(),
            1,
            "the unconnected lens is reported as skipped"
        );
        assert_eq!(m.skipped[0].uid, unconnected);
        assert!(
            m.skipped[0].reason.contains("no place"),
            "the reason should mention placement: '{}'",
            m.skipped[0].reason
        );
        Ok(())
    }

    /// A component left out of the scene is listed with the graph that contains it, so a viewer can
    /// take the user there: here a lens inside a group no light reaches.
    #[test]
    fn a_skipped_component_inside_a_group_is_listed_with_that_group() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let connected = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", connected, "input_1", millimeter!(50.0))?;
        let mut inner = NodeGroup::new("inner");
        let unreached = inner.add_node(Lens::default())?;
        let group_id = scenery.add_node(inner)?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let m = manifest(&document)?;

        assert_eq!(m.skipped.len(), 1);
        assert_eq!(m.skipped[0].uid, unreached);
        assert_eq!(m.skipped[0].group, group_id);
        Ok(())
    }

    /// A placed component of a type that has no look is listed as skipped with that reason, not
    /// silently left out.
    #[test]
    fn a_component_of_a_type_without_a_look_is_listed_as_skipped() -> OpmResult<()> {
        let mut attr = NodeAttr::new("no such type");
        attr.set_positioning(NodePositioning::Absolute(Isometry::identity()));
        let mut stranger = Dummy::default();
        *stranger.node_attr_mut() = attr;
        let mut scenery = NodeGroup::default();
        let stranger = scenery.add_node(stranger)?;

        let m = manifest_of(&scenery, default_reference_wavelength())?;

        assert!(m.nodes.is_empty(), "{:?}", m.nodes);
        assert_eq!(m.skipped.len(), 1);
        assert_eq!(m.skipped[0].uid, stranger);
        assert!(
            m.skipped[0].reason.contains("no look"),
            "the reason should name the missing look: '{}'",
            m.skipped[0].reason
        );
        Ok(())
    }

    /// The manifest names exactly what the scene draws. If the two disagreed, a viewer would either
    /// ask for a component that cannot be served or quietly leave one out.
    #[test]
    fn the_manifest_names_what_the_scene_draws() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let placed = document.placed_scenery(None)?;
        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);

        let manifest = manifest_of(&placed, default_reference_wavelength())?;

        assert_eq!(manifest.nodes.len(), drawn.len());
        Ok(())
    }
}
