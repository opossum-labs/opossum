//! Meshing the components of a model and writing them out as glTF.

use log::warn;
use nalgebra::{Isometry3, Point3};
use opossum_core::{
    analyzers::Analyzable,
    core_optics::{NodeAttrExt, OpticNodeExt},
    error::{OpmResult, OpossumError},
    geometry::{Geometry, SurfaceMesh},
    nodes::NodeGroup,
    types::api_types::SkippedSceneNode,
    utils::geom_transformation::Isometry,
};
use optoscene::{
    Layer, Material, MaterialId, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh,
};
use uom::si::f64::Length;

use crate::{
    appearance::Appearance,
    material::{material_of, refractive_index_of},
};

/// How finely the rim of a clear aperture is sampled.
///
/// The count carries through the whole mesh: it sets how round a lens looks and, through the
/// triangle size limit, how closely the surfaces themselves are followed.
const RIM_SEGMENTS: usize = 64;

/// Build the glTF binary of every component of a model whose type is drawn.
///
/// Every node whose type has a look is meshed and put into the scene; every other one is passed
/// over. A component that cannot be meshed is reported and skipped rather than failing the whole
/// scene, so one unusual lens does not cost the view of the rest of the setup.
///
/// The model has to be placed already — a component only learns where it is from a positioning run,
/// see [`OpmDocument::placed_scenery`](opossum_core::opm_document::OpmDocument::placed_scenery).
/// Placing it is kept out of here on purpose: that half needs the live document while this one,
/// the longer of the two, does not, so a caller holding a lock on it can let go before the meshing
/// starts.
///
/// # Arguments
///
/// - `placed`: a scenery whose nodes carry their placement
/// - `wavelength`: the wavelength the glass of a volume node is given its refractive index at
///
/// # Returns
///
/// The GLB bytes of a scene holding one node per drawable component.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked, or if the scene cannot be written.
pub fn scene_glb(placed: &NodeGroup, wavelength: Length) -> OpmResult<Vec<u8>> {
    let mut scene = Scene::new(SceneOptions::default());
    for_each_drawable(placed, wavelength, |drawn| {
        let placement = placement_of(drawn.node);
        add_at(&mut scene, drawn, placement)
    })?;
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// Where a component is drawn: where its node sits, which is where its geometry is traced.
///
/// # Arguments
///
/// - `node`: the placed component
pub fn placement_of(node: &(impl OpticNodeExt + ?Sized)) -> Isometry3<f64> {
    node.effective_node_iso()
        .unwrap_or_else(Isometry::identity)
        .get_transform()
}

/// A component as it is drawn: its patches, each in the material of the face it lies on.
pub struct Drawn<'a> {
    /// The component.
    pub node: &'a dyn Analyzable,
    /// The component's patches, meshed in its own frame and ready to draw, with their materials.
    pub patches: Vec<(SurfaceMesh, Material)>,
}

impl<'a> Drawn<'a> {
    /// Derive how a component is drawn: its geometry, meshed, every patch in the material its
    /// appearance asks for under the coating of the face it lies on (see [`Appearance::coated`]).
    ///
    /// The lateral face of a solid has no coating and is drawn as the appearance asks. So is a face
    /// whose coating cannot be evaluated, with a warning: drawing it says more than leaving it out.
    ///
    /// The patches of a solid form a closed shell, so one of them always faces the camera. A single
    /// surface has no far side of its own: every patch of it is drawn twice, the second turned over
    /// (see [`SurfaceMesh::flipped`]), as `optoscene`'s `Mirror` and `Opaque` presets do not ask the
    /// renderer to draw both faces of one patch.
    ///
    /// # Arguments
    ///
    /// - `node`: the component
    /// - `appearance`: what the component asks to be drawn in
    /// - `wavelength`: the wavelength glass is given its refractive index and coatings are
    ///   evaluated at
    ///
    /// # Errors
    ///
    /// This function returns an error if the component cannot be meshed, if the medium of its solid
    /// cannot be told (see [`refractive_index_of`]), or if a material cannot be described (see
    /// [`material_of`]).
    pub fn new(
        node: &'a dyn Analyzable,
        appearance: Appearance,
        wavelength: Length,
    ) -> OpmResult<Self> {
        let geometry = node.create_geometry()?;
        let kind = geometry.as_ref().map(Geometry::kind);
        let body = kind.and_then(|kind| kind.as_body());
        // Behind a face lies the medium of the solid, or vacuum behind a single surface.
        let ior = body.map_or(Ok(1.0), |body| refractive_index_of(node, body, wavelength))?;
        let coated = |face| -> OpmResult<Appearance> {
            let name = kind
                .and_then(|kind| kind.face_name(face))
                .ok_or_else(|| OpossumError::Other(format!("there is no face {face}")))?;
            appearance.coated(node.surface_config(&name)?.coating(), wavelength, ior)
        };
        let mut patches = Vec::new();
        for patch in node.geometry_mesh(RIM_SEGMENTS)? {
            let look = patch.face.map_or(appearance, |face| {
                coated(face).unwrap_or_else(|e| {
                    warn!("a face of '{}' is drawn uncoated: {e}", node.name());
                    appearance
                })
            });
            let material = || material_of(node, look, body, ior);
            let turned_over = body.is_none().then(|| patch.mesh.clone().flipped());
            patches.push((patch.mesh, material()?));
            if let Some(turned_over) = turned_over {
                patches.push((turned_over, material()?));
            }
        }
        Ok(Self { node, patches })
    }
}

/// Walk a model and hand every component that can be drawn, as it is drawn, to `draw`.
///
/// Which components a 3D view shows is one decision, and it is made here rather than twice: the
/// scene and the manifest that lists it have to agree on the set, or a viewer would be told about
/// components it can never fetch. A component whose type is not drawn is passed over. A component
/// that cannot be drawn, including one whose type has no look at all (see
/// [`Appearance::of_node_type`]), is reported and skipped rather than failing the whole walk, so one
/// unusual component does not cost the view of the rest of the setup.
///
/// # Arguments
///
/// - `placed`: a scenery whose nodes carry their placement
/// - `wavelength`: the wavelength glass is given its refractive index at
/// - `draw`: called once per drawable component, as it is drawn
///
/// # Returns
///
/// The list of drawable components that were skipped, each with a reason. A caller that builds
/// the scene for export does not need this; a caller that builds the manifest hands it to the
/// viewer so the user can see which components are missing and why.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked, or if the graph of a skipped
/// component cannot be found. An error from `draw` is reported and skipped, not returned.
pub fn for_each_drawable<F>(
    placed: &NodeGroup,
    wavelength: Length,
    mut draw: F,
) -> OpmResult<Vec<SkippedSceneNode>>
where
    F: FnMut(Drawn<'_>) -> OpmResult<()>,
{
    let mut skipped = Vec::new();
    for node_ref in placed.collect_all_nodes_recursive()? {
        let Some(appearance) = Appearance::of_node_type(node_ref.node_type()).transpose() else {
            continue;
        };
        if node_ref.node_attr().effective_position().is_none() {
            warn!("{node_ref} has no place in the setup and is left out of the scene");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                group: placed.parent_group_id_or_self(node_ref.uuid())?,
                name: node_ref.name().to_string(),
                reason: "has no place in the setup".to_string(),
            });
            continue;
        }
        let drawn = appearance
            .and_then(|appearance| Drawn::new(&*node_ref, appearance, wavelength))
            .and_then(&mut draw);
        if let Err(e) = drawn {
            warn!("{node_ref} cannot be drawn and is left out of the scene: {e}");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                group: placed.parent_group_id_or_self(node_ref.uuid())?,
                name: node_ref.name().to_string(),
                reason: e.to_string(),
            });
        }
    }
    Ok(skipped)
}

/// Add one drawn component to a scene, placed where the caller says.
///
/// The placement is a parameter rather than taken from the component, because the two views of a
/// component need different ones: the scene of a whole setup puts it where the positioning run
/// found it, while the component's own file holds it at the coordinate origin so that the same
/// geometry serves it wherever it later moves.
///
/// # Arguments
///
/// - `scene`: the scene to add to
/// - `drawn`: the component, as it is drawn
/// - `placement`: where to put the component
///
/// # Errors
///
/// This function returns an error if `optoscene` rejects the resulting geometry.
fn add_at(scene: &mut Scene, drawn: Drawn<'_>, placement: Isometry3<f64>) -> OpmResult<()> {
    let Drawn { node, patches } = drawn;
    // One patch per face rather than one for all, so that the hard edge where the faces meet stays
    // hard: patches do not share vertices. Equal materials are registered once.
    let patches = patches
        .into_iter()
        .map(|(mesh, material)| patch_of(&mesh, scene.add_material(material)))
        .collect();
    let mesh_id = scene.add_mesh(TriMesh { patches }).map_err(|e| {
        OpossumError::Other(format!("the mesh of '{}' was rejected: {e}", node.name()))
    })?;
    scene
        .add_node(SceneNode {
            uid: node.node_attr().uuid().to_string(),
            name: node.name().to_string(),
            mesh: Some(mesh_id),
            // The mesh was built in the component's own frame and the placement is handed over
            // beside it, so moving the component later costs a transform and not a new mesh.
            transform: placement,
            layer: Layer::Optics,
            data: None,
        })
        .map_err(|e| OpossumError::Other(format!("'{}' could not be placed: {e}", node.name())))?;
    Ok(())
}

/// Build the glTF binary of a single component, in the component's own frame.
///
/// The component sits at the coordinate origin rather than where the setup puts it, and that is
/// what makes the file worth fetching on its own: a component that merely moved keeps its file, and
/// a viewer places it from the manifest instead of asking for its mesh again.
///
/// It also needs no positioning run — what a component looks like does not depend on where it ended
/// up — and no rebuild of the node's surfaces: the shape is derived from its properties as they are
/// now.
///
/// # Arguments
///
/// - `node`: the component to draw
/// - `wavelength`: the wavelength glass is given its refractive index at
///
/// # Returns
///
/// The GLB bytes of a scene holding nothing but this component, or `None` if its type is not drawn
/// at all.
///
/// # Errors
///
/// This function returns an error if no look is defined for the component's type, if the component
/// cannot be drawn (see [`Drawn::new`]), or if the scene cannot be written.
pub fn glb_of_node(node: &dyn Analyzable, wavelength: Length) -> OpmResult<Option<Vec<u8>>> {
    let Some(appearance) = Appearance::of_node_type(node.node_type())? else {
        return Ok(None);
    };
    // Pinned at the coordinate origin. Left to itself the exporter subtracts the centre of the
    // bounding box of whatever it holds, and the file would then no longer be the component in its
    // own frame - it would be shifted by an amount that depends on the component's own size.
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    add_at(
        &mut scene,
        Drawn::new(node, appearance, wavelength)?,
        Isometry3::identity(),
    )?;
    scene
        .to_glb()
        .map(Some)
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// Turn one meshed face into a patch `optoscene` can export.
fn patch_of(face: &SurfaceMesh, material: MaterialId) -> SurfacePatch {
    SurfacePatch {
        name: None,
        positions: face
            .points()
            .iter()
            .map(|point| Point3::new(point.x.value, point.y.value, point.z.value))
            .collect(),
        normals: Some(face.normals().to_vec()),
        colors: None,
        indices: face.triangles().to_vec(),
        material,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::{
        a_bit_of_everything, drawn_components, exported, glb_json, two_lenses,
    };
    use opossum_core::{
        analyzers::{AnalyzerType, RayTraceConfig},
        core_optics::{OpticNode, node_attr::NodePositioning},
        light::lightdata::ray_data_builder::RayDataBuilder,
        material::default_reference_wavelength,
        millimeter,
        nodes::{Lens, SourcePort, ThinMirror, create_node_ref, node_types},
        opm_document::OpmDocument,
        properties::Proptype,
    };
    use uuid::Uuid;

    /// Only what encloses a volume or is one optical surface is drawn. A source port has no shape
    /// of its own, and a reference is the very lens already in the scene, met a second time.
    #[test]
    fn only_the_volumes_and_surfaces_are_drawn() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);
        // The lens, the wedge and the cylindric lens enclose a volume; the mirror and the detector
        // are one surface each.
        assert_eq!(drawn.len(), 5);
        // Every one of them names the component it stands for, so a viewer can point back at it.
        for component in drawn {
            let uid = component["extras"]["uid"]
                .as_str()
                .expect("a drawn component names itself");
            assert!(
                document
                    .scenery()
                    .node_recursive(uid.parse().expect("the uid is a node uuid"))
                    .is_ok(),
                "the scene names a component the model does not have: {uid}"
            );
        }
        Ok(())
    }

    /// A surface node is meshed as two patches facing opposite ways, so it stays visible when the
    /// camera crosses to its back - unlike a volume's closed shell, it has no far side of its own to
    /// keep facing the camera.
    #[test]
    fn a_surface_is_meshed_to_be_visible_from_either_side() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);
        assert_eq!(drawn.len(), 1);
        let mesh_index = drawn[0]["mesh"].as_u64().expect("the mirror has a mesh");
        let primitives = scene["meshes"][mesh_index as usize]["primitives"]
            .as_array()
            .expect("a mesh has primitives");
        assert_eq!(
            primitives.len(),
            2,
            "a surface node must draw both faces, not just the one its geometry is meshed as"
        );
        Ok(())
    }
    /// A solid is drawn as its closed shell, once: front, rear and lateral face. One of them always
    /// faces the camera, so nothing is drawn twice.
    #[test]
    fn a_solid_is_drawn_as_its_closed_shell_once() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let glb = glb_of_node(&lens, default_reference_wavelength())?.expect("a lens is drawn");
        let scene = glb_json(&glb);
        let drawn = drawn_components(&scene);
        assert_eq!(drawn.len(), 1);
        let mesh_index = drawn[0]["mesh"].as_u64().expect("the lens has a mesh");
        let primitives = scene["meshes"][mesh_index as usize]["primitives"]
            .as_array()
            .expect("a mesh has primitives");
        assert_eq!(primitives.len(), 3);
        Ok(())
    }

    /// The single-component endpoint works for a surface node exactly as it does for a volume one.
    #[test]
    fn a_surface_nodes_own_file_is_a_valid_glb() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let glb = glb_of_node(&mirror, default_reference_wavelength())?.expect("a mirror is drawn");
        assert!(glb.starts_with(b"glTF"));
        Ok(())
    }
    /// A component that states no appearance has no file of its own: the endpoint answers it with
    /// "not found" rather than an empty scene.
    #[test]
    fn a_component_that_is_not_drawn_has_no_file() -> OpmResult<()> {
        let mut source = SourcePort::default();
        source.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        assert!(glb_of_node(&source, default_reference_wavelength())?.is_none());
        Ok(())
    }

    /// Drawing a setup must not decide anything about the setup itself — least of all freeze the
    /// placements this run worked out into the model.
    #[test]
    fn drawing_a_model_leaves_it_alone() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let before = document.to_opm_file_string()?;
        scene_glb(
            &document.placed_scenery(None)?,
            default_reference_wavelength(),
        )?;
        assert_eq!(document.to_opm_file_string()?, before);
        Ok(())
    }

    /// Two components of the same kind and size have the same shape, so a viewer should be handed
    /// that shape once and told to draw it twice.
    #[test]
    fn identical_components_share_one_mesh() -> OpmResult<()> {
        let scene = exported(&two_lenses(millimeter!(50.0))?)?;
        assert_eq!(drawn_components(&scene).len(), 2);
        assert_eq!(
            scene["meshes"]
                .as_array()
                .expect("a scene has meshes")
                .len(),
            1,
            "the same shape was sent twice instead of being instanced"
        );
        Ok(())
    }

    /// The file a component's endpoint serves has to follow an edit to that component's shape.
    ///
    /// The trap this pins down: a property is patched straight onto the live document without
    /// rebuilding the node's surfaces. A shape read off those surfaces would therefore be the *old*
    /// one, while the manifest - built from a `placed_scenery` - reports a new hash. The viewer
    /// would then refetch and get the old mesh back, and changing a lens thickness would appear to
    /// do nothing at all.
    #[test]
    fn reshaping_a_component_changes_the_file_its_endpoint_serves() -> OpmResult<()> {
        let mut document = two_lenses(millimeter!(100.0))?;
        let lens = document
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .find(|node| node.node_type() == "lens")
            .expect("a model with lenses has something to draw")
            .node_attr()
            .uuid();
        let before = served_geometry_of(&document, lens)?;

        document.scenery_mut().with_node_attr_mut(lens, |attr| {
            attr.set_property("center thickness", millimeter!(40.0).into())
        })??;

        assert_ne!(
            before,
            served_geometry_of(&document, lens)?,
            "the component's endpoint kept serving its old shape after the property changed"
        );
        Ok(())
    }

    /// The same for a component that is one surface: a flat mirror bent after it was built.
    #[test]
    fn bending_a_mirror_changes_the_file_its_endpoint_serves() -> OpmResult<()> {
        let mut document = a_bit_of_everything()?;
        let mirror = document
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .find(|node| node.properties().get("curvature").is_ok())
            .expect("the model has a mirror")
            .node_attr()
            .uuid();
        let before = served_geometry_of(&document, mirror)?;

        document.scenery_mut().with_node_attr_mut(mirror, |attr| {
            attr.set_property("curvature", Proptype::Curvature(millimeter!(100.0)))
        })??;

        assert_ne!(
            before,
            served_geometry_of(&document, mirror)?,
            "the mirror's endpoint kept serving its old shape after the property changed"
        );
        Ok(())
    }

    /// A component drawn from the live document has to look the same as one from a placed model.
    ///
    /// The manifest hashes the shape it finds in a placed copy, while the component's endpoint
    /// serves the shape it derives on its own. If the two derivations disagreed by so much as a
    /// rounding step, every manifest would report a hash for bytes nobody serves, and the viewer
    /// would refetch a component that never stops looking changed.
    #[test]
    fn a_component_draws_the_same_from_the_document_as_from_a_placed_model() -> OpmResult<()> {
        let document = two_lenses(millimeter!(100.0))?;
        let lens = document
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .find(|node| node.node_type() == "lens")
            .expect("a model with lenses has something to draw")
            .node_attr()
            .uuid();

        let placed = document.placed_scenery(None)?;
        let from_placed = glb_of_node(
            &*placed.node_recursive(lens)?.0,
            default_reference_wavelength(),
        )?;

        assert_eq!(
            from_placed,
            served_geometry_of(&document, lens)?,
            "placing the model and drawing one component produce different geometry"
        );
        Ok(())
    }

    /// The bytes `GET /scene/node/{uid}.glb` would answer with for one component of a document.
    fn served_geometry_of(document: &OpmDocument, uid: Uuid) -> OpmResult<Option<Vec<u8>>> {
        glb_of_node(
            &*document.scenery().node_recursive(uid)?.0,
            default_reference_wavelength(),
        )
    }

    /// A component's own file holds it at the origin: that is what lets the same file serve the
    /// component wherever the setup later puts it.
    #[test]
    fn a_components_own_file_holds_it_at_the_origin() -> OpmResult<()> {
        let document = two_lenses(millimeter!(100.0))?;
        let placed = document.placed_scenery(None)?;
        let far = placed
            .collect_all_nodes_recursive()?
            .into_iter()
            .filter_map(|node| glb_of_node(&*node, default_reference_wavelength()).transpose())
            .next_back()
            .expect("a model with lenses has something to draw")?;

        let scene = glb_json(&far);
        for node in scene["nodes"].as_array().expect("a scene has nodes") {
            let offset: Vec<f64> = node["translation"].as_array().map_or_else(
                || vec![0.0; 3],
                |axes| axes.iter().map(|a| a.as_f64().unwrap_or(0.0)).collect(),
            );
            assert!(
                offset.iter().all(|axis| axis.abs() < 1e-9),
                "a component's own file placed it away from the origin: {node}"
            );
        }
        Ok(())
    }

    /// Every node type that is drawn can be drawn in its default state: its geometry can be meshed,
    /// and every patch holds triangles.
    ///
    /// A node type is registered once and then shows up in the 3D view without anything being added
    /// for it, so whether its default geometry can actually be meshed has to be settled here rather
    /// than found out later by a viewer left with a hole in the scene.
    #[test]
    fn every_drawn_node_can_be_meshed_in_its_default_state() -> OpmResult<()> {
        for (node_type, _) in node_types() {
            if Appearance::of_node_type(node_type)?.is_none() {
                continue;
            }
            let meshes = create_node_ref(node_type)?.geometry_mesh(32).map_err(|e| {
                OpossumError::Other(format!("node type '{node_type}' cannot be meshed: {e}"))
            })?;
            assert!(
                !meshes.is_empty(),
                "node type '{node_type}' came out without a patch"
            );
            for patch in &meshes {
                assert!(
                    !patch.mesh.triangles().is_empty(),
                    "a patch of node type '{node_type}' came out without any triangles"
                );
            }
        }
        Ok(())
    }
}
