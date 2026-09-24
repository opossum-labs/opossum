//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which knows the shape of every component, and the
//! file format from `optoscene`, which knows glTF. This module is only the seam between the two: it
//! decides what of a model gets drawn at all, and which material it is drawn in.

use log::warn;
use nalgebra::{Isometry3, Point3};
use opossum_core::{
    core_optics::{NodeAttrExt, volumetric::Volumetric},
    error::{OpmResult, OpossumError},
    geometry::{
        SurfaceMesh,
        body::{Body, SurfaceBoundedBody},
    },
    opm_document::OpmDocument,
    types::api_types::{SceneManifest, SceneNodeEntry},
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
    for_each_drawable(placed, |node, body| {
        add_volume_at(
            &mut scene,
            node,
            body,
            wavelength,
            body.isometry().get_transform(),
        )
    })?;
    Ok(scene)
}

/// Walk a model and hand every component that can be drawn, with its volume, to `draw`.
///
/// Which components a 3D view shows is one decision, and it is made here rather than twice: the
/// scene and the manifest that lists it have to agree on the set, or a viewer would be told about
/// components it can never fetch. Components without a volume are passed over - a detector has no
/// shape of its own yet, and a node reference is not a second component but the same one passed
/// through again. A component that cannot be drawn is reported and skipped rather than failing the
/// whole walk, so one unusual lens does not cost the view of the rest of the setup.
///
/// The volume is derived once here and handed over, because deriving it copies the node's whole
/// port set (see [`Volumetric::volume_body`]) and both callers need it.
///
/// # Arguments
///
/// - `placed`: a model whose nodes carry their placement
/// - `draw`: called once per drawable component with the component and its volume
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked. An error from `draw` is reported
/// and skipped, not returned.
fn for_each_drawable<F>(placed: &OpmDocument, mut draw: F) -> OpmResult<()>
where
    F: FnMut(&dyn Volumetric, &SurfaceBoundedBody) -> OpmResult<()>,
{
    for node_ref in placed.scenery().collect_all_nodes_recursive()? {
        let Some(volume) = node_ref.as_volume() else {
            continue;
        };
        if node_ref.node_attr().effective_position().is_none() {
            warn!("{node_ref} has no place in the setup and is left out of the scene");
            continue;
        }
        let drawn = volume.volume_body().and_then(|body| draw(volume, &body));
        if let Err(e) = drawn {
            warn!("{node_ref} cannot be drawn and is left out of the scene: {e}");
        }
    }
    Ok(())
}

/// Add one component that encloses a volume to a scene, placed where the caller says.
///
/// The placement is a parameter rather than taken from the volume, because the two views of a
/// component need different ones: the scene of a whole setup puts it where the positioning run
/// found it, while the component's own file holds it at the coordinate origin so that the same
/// geometry serves it wherever it later moves.
///
/// # Arguments
///
/// - `scene`: the scene to add to
/// - `node`: the component to draw
/// - `body`: the component's volume, already derived
/// - `wavelength`: the wavelength the glass is given its refractive index at
/// - `placement`: where to put the component
///
/// # Errors
///
/// This function returns an error if the component's volume cannot be meshed, or if `optoscene`
/// rejects the resulting geometry.
fn add_volume_at(
    scene: &mut Scene,
    node: &dyn Volumetric,
    body: &SurfaceBoundedBody,
    wavelength: Length,
    placement: Isometry3<f64>,
) -> OpmResult<()> {
    let mesh = body.triangulate(RIM_SEGMENTS)?;
    let material = scene.add_material(glass_of(node, body, wavelength)?);
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
/// a viewer places it from the manifest instead of asking for its mesh again. It also means this
/// needs no positioning run — what a component looks like does not depend on where it ended up.
///
/// # Arguments
///
/// - `node`: the component to draw
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Returns
///
/// The GLB bytes of a scene holding nothing but this component.
///
/// # Errors
///
/// This function returns an error if the component's volume cannot be derived or meshed, or if the
/// scene cannot be written.
pub fn glb_of_node(node: &dyn Volumetric, wavelength: Length) -> OpmResult<Vec<u8>> {
    // Pinned at the coordinate origin. Left to itself the exporter subtracts the centre of the
    // bounding box of whatever it holds, and the file would then no longer be the component in its
    // own frame - it would be shifted by an amount that depends on the component's own size.
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    add_volume_at(
        &mut scene,
        node,
        &node.volume_body()?,
        wavelength,
        Isometry3::identity(),
    )?;
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// List every drawable component of a model with its placement, but without its geometry.
///
/// This is the half of a 3D view that is cheap to repeat. A viewer asks for it whenever the model
/// changed and compares it with what it already shows: a component whose `geometry` is unchanged
/// only needs its placement applied, however far it moved, and only one whose shape really changed
/// has its file fetched again.
///
/// The same walk as [`scene_of`] decides what is in here, so the manifest never names a component
/// that cannot be drawn.
///
/// # Arguments
///
/// - `placed`: a model whose nodes carry their placement
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Returns
///
/// One entry per drawable component, with positions relative to the manifest's own origin.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked.
pub fn manifest_of(placed: &OpmDocument, wavelength: Length) -> OpmResult<SceneManifest> {
    let mut nodes = Vec::new();
    for_each_drawable(placed, |node, body| {
        let placement = body.isometry().get_transform();
        let rotation = placement.rotation.quaternion();
        nodes.push(SceneNodeEntry {
            uid: node.node_attr().uuid(),
            name: node.name().to_string(),
            geometry: geometry_of(node, body, wavelength)?,
            position: placement.translation.vector.into(),
            rotation: [rotation.i, rotation.j, rotation.k, rotation.w],
        });
        Ok(())
    })?;
    let origin = center_of(&nodes);
    for node in &mut nodes {
        node.position = [
            node.position[0] - origin[0],
            node.position[1] - origin[1],
            node.position[2] - origin[2],
        ];
    }
    Ok(SceneManifest { origin, nodes })
}

/// The center of the axis-aligned box around a set of component positions.
///
/// Positions are handed to a viewer relative to this point, because a renderer works in `f32` and a
/// setup far from the coordinate origin would lose precision there. It moves when the extent of the
/// setup changes, which costs every component a new placement but never a new mesh — the geometry
/// files do not know about it.
///
/// # Returns
///
/// The center, or the coordinate origin if there are no components.
fn center_of(nodes: &[SceneNodeEntry]) -> [f64; 3] {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for node in nodes {
        for axis in 0..3 {
            min[axis] = min[axis].min(node.position[axis]);
            max[axis] = max[axis].max(node.position[axis]);
        }
    }
    if nodes.is_empty() {
        return [0.0; 3];
    }
    [
        f64::midpoint(min[0], max[0]),
        f64::midpoint(min[1], max[1]),
        f64::midpoint(min[2], max[2]),
    ]
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
/// - `node`: the component whose looks are wanted
/// - `body`: its volume, already derived
/// - `wavelength`: the wavelength the glass is given its refractive index at
///
/// # Returns
///
/// A hexadecimal digest of the component's mesh and material.
///
/// # Errors
///
/// This function returns an error if the volume cannot be meshed, or if the component states no
/// usable material.
fn geometry_of(
    node: &dyn Volumetric,
    body: &SurfaceBoundedBody,
    wavelength: Length,
) -> OpmResult<String> {
    let mesh = body.triangulate(RIM_SEGMENTS)?;
    let mut hash = Fnv1a::new();
    for face in mesh.faces() {
        for point in face.points() {
            hash.write_f64(point.x.value);
            hash.write_f64(point.y.value);
            hash.write_f64(point.z.value);
        }
        for normal in face.normals() {
            hash.write_f64(normal.x);
            hash.write_f64(normal.y);
            hash.write_f64(normal.z);
        }
        for triangle in face.triangles() {
            for corner in triangle {
                hash.write(&corner.to_le_bytes());
            }
        }
    }
    hash.write_material(&glass_of(node, body, wavelength)?);
    Ok(hash.finish())
}

/// A running 64 bit FNV-1a hash.
///
/// Nothing cryptographic is called for — the only question is "the same as last time?" — but the
/// answer has to mean the same thing from one run to the next, which rules out `DefaultHasher`,
/// whose output is not promised to be stable between Rust releases. FNV is a dozen lines, so it
/// costs no new dependency either.
struct Fnv1a(u64);

impl Fnv1a {
    /// Start a hash.
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// Fold bytes into the hash.
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Fold a number into the hash, little-endian so the result does not depend on the machine.
    fn write_f64(&mut self, value: f64) {
        self.write(&value.to_le_bytes());
    }

    /// Fold a material into the hash.
    ///
    /// Written out variant by variant on purpose: were `optoscene` to grow another material, this
    /// stops compiling rather than quietly giving two different materials the same hash.
    fn write_material(&mut self, material: &Material) {
        let (tag, color, numbers): (u8, &[f32], [f32; 2]) = match material {
            Material::Glass {
                color,
                ior,
                thickness,
            } => (0, color, [*ior, *thickness]),
            Material::Mirror { color, roughness } => (1, color, [*roughness, 0.0]),
            Material::Opaque {
                color,
                metallic,
                roughness,
            } => (2, color, [*metallic, *roughness]),
            Material::Unlit { color } => (3, color, [0.0; 2]),
            Material::Translucent { color } => (4, color, [0.0; 2]),
        };
        self.write(&[tag]);
        for value in color.iter().chain(numbers.iter()) {
            self.write(&value.to_le_bytes());
        }
    }

    /// The hash so far, as hexadecimal.
    fn finish(&self) -> String {
        format!("{:016x}", self.0)
    }
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

    /// Two lenses a given distance apart, so that moving one is a one-line change.
    fn two_lenses(spacing: Length) -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let first = scenery.add_node(Lens::default())?;
        let second = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(first, "output_1", second, "input_1", spacing)?;

        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok(document)
    }

    /// The list of components of a model, as a viewer would ask for it.
    fn manifest(document: &OpmDocument) -> OpmResult<SceneManifest> {
        manifest_of(
            &document.positioned_copy(None)?,
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
            .fold(0.0_f64, f64::max);
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

    /// The manifest names exactly what the scene draws. If the two disagreed, a viewer would either
    /// ask for a component that cannot be served or quietly leave one out.
    #[test]
    fn the_manifest_names_what_the_scene_draws() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let placed = document.positioned_copy(None)?;
        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);

        let manifest = manifest_of(&placed, default_reference_wavelength())?;

        assert_eq!(manifest.nodes.len(), drawn.len());
        Ok(())
    }

    /// A component's own file holds it at the origin: that is what lets the same file serve the
    /// component wherever the setup later puts it.
    #[test]
    fn a_components_own_file_holds_it_at_the_origin() -> OpmResult<()> {
        let document = two_lenses(millimeter!(100.0))?;
        let placed = document.positioned_copy(None)?;
        let far = placed
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .filter_map(|node| {
                node.as_volume()
                    .map(|v| glb_of_node(v, default_reference_wavelength()))
            })
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
}
