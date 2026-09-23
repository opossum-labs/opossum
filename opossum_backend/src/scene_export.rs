//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which knows the shape of every component, and the
//! file format from `optoscene`, which knows glTF. This module is only the seam between the two: it
//! decides what of a model gets drawn at all, and which material it is drawn in.

use log::warn;
use nalgebra::Point3;
use opossum_core::{
    core_optics::{NodeAttrExt, volumetric::Volumetric},
    error::{OpmResult, OpossumError},
    geometry::{SurfaceMesh, body::Body},
    opm_document::OpmDocument,
};
use optoscene::{
    Layer, Material, MaterialId, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh,
};
use uom::si::f64::Length;

/// How finely the rim of a clear aperture is sampled.
///
/// The count carries through the whole mesh: it sets how round a lens looks and, through the
/// triangle size limit, how closely the surfaces themselves are followed.
const RIM_SEGMENTS: usize = 64;

/// The colour glass is drawn in — a barely tinted white, so that what shapes it is the refraction
/// rather than the colour.
const GLASS_COLOUR: [f32; 3] = [0.9, 0.95, 1.0];

/// The refractive index used for a material that cannot state one at the asked wavelength.
///
/// Drawing the component in plain glass says more than leaving it out of the scene.
const FALLBACK_REFRACTIVE_INDEX: f64 = 1.5;

/// Build a 3D scene of every component of a model that encloses a volume.
///
/// Every node that encloses a volume is meshed and put into the scene. Components without a volume
/// are passed over: a detector has no shape of its own yet, and a node reference is not a second
/// component but the same one passed through again. A component that cannot be meshed is reported
/// and skipped rather than failing the whole scene, so one unusual lens does not cost the view of
/// the rest of the setup.
///
/// The model has to be placed already — a component only learns where it is from a positioning run,
/// see [`OpmDocument::positioned_copy`]. Placing it is kept out of here on purpose: that half needs
/// the live document while this one, the longer of the two, does not, so a caller holding a lock on
/// it can let go before the meshing starts.
///
/// # Arguments
///
/// - `placed`: a model whose nodes carry their placement
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Returns
///
/// A scene holding one node per drawable component.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked.
pub fn scene_of(placed: &OpmDocument, wavelength: Length) -> OpmResult<Scene> {
    let mut scene = Scene::new(SceneOptions::default());
    for node_ref in placed.scenery().collect_all_nodes_recursive()? {
        let Some(volume) = node_ref.as_volume() else {
            continue;
        };
        if node_ref.node_attr().effective_position().is_none() {
            warn!("{node_ref} has no place in the setup and is left out of the scene");
            continue;
        }
        if let Err(e) = add_volume_node(&mut scene, volume, wavelength) {
            warn!("{node_ref} cannot be drawn and is left out of the scene: {e}");
        }
    }
    Ok(scene)
}

/// Add one component that encloses a volume to a scene.
///
/// Kept apart from the walk over the model so that a view of a single component — a preview beside
/// its settings, say — can ask for just this one.
///
/// # Arguments
///
/// - `scene`: the scene to add to
/// - `node`: the component to draw
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Errors
///
/// This function returns an error if the component's volume cannot be derived or meshed, or if
/// `optoscene` rejects the resulting geometry.
pub fn add_volume_node(
    scene: &mut Scene,
    node: &dyn Volumetric,
    wavelength: Length,
) -> OpmResult<()> {
    let body = node.volume_body()?;
    let mesh = body.triangulate(RIM_SEGMENTS)?;
    let material = scene.add_material(glass_of(node, &body, wavelength)?);
    let mesh_id = scene
        .add_mesh(TriMesh {
            // Three patches rather than one, so that the hard edge where the faces meet stays hard:
            // patches do not share vertices.
            patches: mesh.faces().map(|face| patch_of(face, material)).into(),
        })
        .map_err(|e| {
            OpossumError::Other(format!("the mesh of '{}' was rejected: {e}", node.name()))
        })?;
    scene
        .add_node(SceneNode {
            uid: node.node_attr().uuid().to_string(),
            name: node.name().to_string(),
            mesh: Some(mesh_id),
            // The mesh was built in the component's own frame and the placement is handed over
            // beside it, so moving the component later costs a transform and not a new mesh.
            transform: body.isometry().get_transform(),
            layer: Layer::Optics,
            data: None,
        })
        .map_err(|e| OpossumError::Other(format!("'{}' could not be placed: {e}", node.name())))?;
    Ok(())
}

/// Describe the glass a component is made of, as far as a renderer needs it.
///
/// # Arguments
///
/// - `node`: the component whose material is wanted
/// - `body`: its volume, which the thickness is measured from
/// - `wavelength`: the wavelength to evaluate the refractive index at
///
/// # Errors
///
/// This function returns an error if the component states no material, or if its volume has no
/// extent to measure a thickness from.
// A refractive index and a thickness in millimetres are nowhere near the range of an f32.
#[allow(clippy::cast_possible_truncation)]
fn glass_of(node: &dyn Volumetric, body: &impl Body, wavelength: Length) -> OpmResult<Material> {
    let material = node.material()?;
    let ior = material.refractive_index(wavelength).unwrap_or_else(|e| {
        warn!(
            "'{}' states no refractive index at this wavelength ({e}), drawing it as plain glass",
            node.name()
        );
        FALLBACK_REFRACTIVE_INDEX
    });
    let thickness = body.bounding_box()?.z_range();
    Ok(Material::Glass {
        color: GLASS_COLOUR,
        ior: ior as f32,
        thickness: (thickness.end - thickness.start).value as f32,
    })
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
    use opossum_core::{
        analyzers::{AnalyzerType, RayTraceConfig},
        light::lightdata::ray_data_builder::RayDataBuilder,
        material::default_reference_wavelength,
        millimeter,
        nodes::{CylindricLens, EnergyMeter, Lens, NodeGroup, NodeReference, SourcePort, Wedge},
    };

    /// A model holding one of everything: three components that enclose a volume, a source port and
    /// a detector that do not, and a reference, which is the same lens passed through a second time
    /// rather than another component.
    fn a_bit_of_everything() -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let wedge = scenery.add_node(Wedge::default())?;
        let cylinder = scenery.add_node(CylindricLens::default())?;
        let lens_again =
            scenery.add_node(NodeReference::from_node(&scenery.node_recursive(lens)?.0)?)?;
        let detector = scenery.add_node(EnergyMeter::default())?;
        let hop = millimeter!(50.0);
        scenery.connect_nodes(source, "output_1", lens, "input_1", hop)?;
        scenery.connect_nodes(lens, "output_1", wedge, "input_1", hop)?;
        scenery.connect_nodes(wedge, "output_1", cylinder, "input_1", hop)?;
        scenery.connect_nodes(cylinder, "output_1", lens_again, "input_1", hop)?;
        scenery.connect_nodes(lens_again, "output_1", detector, "input_1", hop)?;

        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok(document)
    }

    /// Read the JSON chunk out of a binary glTF file.
    ///
    /// A GLB is a 12 byte header followed by length-prefixed chunks, the first of which describes
    /// the scene. Everything a test wants to know about an exported scene is in there.
    fn glb_json(glb: &[u8]) -> serde_json::Value {
        let length_bytes: [u8; 4] = glb[12..16].try_into().expect("a glTF header");
        let json_length = u32::from_le_bytes(length_bytes) as usize;
        serde_json::from_slice(&glb[20..20 + json_length]).expect("readable glTF JSON")
    }

    /// Export a model and read back what the exported scene says about itself.
    fn exported(document: &OpmDocument) -> OpmResult<serde_json::Value> {
        let placed = document.positioned_copy(None)?;
        let glb = scene_of(&placed, default_reference_wavelength())?
            .to_glb()
            .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))?;
        assert!(glb.starts_with(b"glTF"));
        Ok(glb_json(&glb))
    }

    /// The components a scene actually draws.
    ///
    /// `optoscene` wraps them in a root node and one group per layer, and neither of those carries
    /// any geometry — counting all nodes would count the scaffolding too.
    fn drawn_components(scene: &serde_json::Value) -> Vec<&serde_json::Value> {
        scene["nodes"]
            .as_array()
            .expect("a scene has nodes")
            .iter()
            .filter(|node| node.get("mesh").is_some())
            .collect()
    }

    /// Only what encloses a volume is drawn. A source port and a detector have no shape of their
    /// own, and a reference is the very lens already in the scene, met a second time.
    #[test]
    fn only_the_components_with_a_volume_are_drawn() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);
        assert_eq!(drawn.len(), 3);
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

    /// Drawing a setup must not decide anything about the setup itself — least of all freeze the
    /// placements this run worked out into the model.
    #[test]
    fn drawing_a_model_leaves_it_alone() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let before = document.to_opm_file_string()?;
        scene_of(
            &document.positioned_copy(None)?,
            default_reference_wavelength(),
        )?;
        assert_eq!(document.to_opm_file_string()?, before);
        Ok(())
    }

    /// Two components of the same kind and size have the same shape, so a viewer should be handed
    /// that shape once and told to draw it twice.
    #[test]
    fn identical_components_share_one_mesh() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let first = scenery.add_node(Lens::default())?;
        let second = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(first, "output_1", second, "input_1", millimeter!(50.0))?;
        let document = OpmDocument::new(scenery);

        let scene = exported(&document)?;
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

    /// Glass is only glass to a renderer if it carries an index of refraction.
    #[test]
    fn glass_is_exported_with_its_refractive_index() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        for material in scene["materials"]
            .as_array()
            .expect("a scene has materials")
        {
            let ior = &material["extensions"]["KHR_materials_ior"]["ior"];
            assert!(
                ior.as_f64().is_some_and(|index| index >= 1.0),
                "a material was exported without a usable refractive index: {material}"
            );
        }
        assert!(
            scene["extensionsUsed"]
                .as_array()
                .expect("the extensions are listed")
                .iter()
                .any(|used| used == "KHR_materials_ior")
        );
        Ok(())
    }
}
