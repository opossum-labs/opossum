//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which knows the shape of every component, and the
//! file format from `optoscene`, which knows glTF. This module is only the seam between the two: it
//! decides what of a model gets drawn at all, and which material it is drawn in.

use log::warn;
use nalgebra::{Isometry3, Point3};
use opossum_core::{
    core_optics::{
        Appearance, NodeAttrExt, OpticNodeExt, SurfaceFinish, planar::Planar,
        volumetric::Volumetric,
    },
    error::{OpmResult, OpossumError},
    geometry::{
        SurfaceMesh,
        body::{Body, SurfaceBoundedBody},
    },
    light::Rays,
    nanometer,
    opm_document::OpmDocument,
    types::api_types::{SceneManifest, SceneNodeEntry, SkippedSceneNode},
    utils::geom_transformation::Isometry,
};
use optoscene::{
    Layer, Material, MaterialId, RayStyle, RayTrace, Scene, SceneNode, SceneOptions, SurfacePatch,
    TriMesh,
};
use uom::si::{f64::Length, length::meter};

use crate::helper_functions::parent_group_id_or_self;

/// How finely the rim of a clear aperture is sampled.
///
/// The count carries through the whole mesh: it sets how round a lens looks and, through the
/// triangle size limit, how closely the surfaces themselves are followed.
const RIM_SEGMENTS: usize = 64;

/// The refractive index used for a material that cannot state one at the asked wavelength.
///
/// Drawing the component in plain glass says more than leaving it out of the scene.
const FALLBACK_REFRACTIVE_INDEX: f64 = 1.5;

/// Build a 3D scene of every component of a model that encloses a volume or is one optical surface.
///
/// Every node that encloses a volume, or is a single surface (a mirror, a grating, a filter, a
/// detector, ...), is meshed and put into the scene. Every other component is passed over: a node
/// reference is not a second component but the same one passed through again, a source port is
/// where light begins rather than a thing, and a group is the nodes it contains, not a component of
/// its own. A component that cannot be meshed is reported and skipped rather than failing the whole
/// scene, so one unusual lens does not cost the view of the rest of the setup.
///
/// The model has to be placed already — a component only learns where it is from a positioning run,
/// see [`OpmDocument::positioned_copy`]. Placing it is kept out of here on purpose: that half needs
/// the live document while this one, the longer of the two, does not, so a caller holding a lock on
/// it can let go before the meshing starts.
///
/// # Arguments
///
/// - `placed`: a model whose nodes carry their placement
/// - `wavelength`: the wavelength the glass of a volume node is given its refractive index at
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
    for_each_drawable(placed, |drawn| match drawn {
        Drawn::Volume(node, body) => add_volume_at(
            &mut scene,
            node,
            &body,
            wavelength,
            body.isometry().get_transform(),
        ),
        Drawn::Surface(node, mesh) => add_surface_at(
            &mut scene,
            node,
            &mesh,
            node.effective_node_iso()
                .unwrap_or_else(Isometry::identity)
                .get_transform(),
        ),
    })?;
    Ok(scene)
}

/// Which drawable capability a node presents, before its shape is derived.
///
/// A node answers `as_volume`/`as_surface` for itself (see [`Volumetric`] and [`Planar`]); this is
/// only the two-armed choice between them, made once per node so [`for_each_drawable`] does not ask
/// twice. [`Drawn`] is the sibling that also carries the derived shape, handed to the caller.
enum Drawable<'a> {
    /// The node encloses a volume of material.
    Volume(&'a dyn Volumetric),
    /// The node is one optical surface.
    Surface(&'a dyn Planar),
}

/// A drawable component together with its derived shape, as [`for_each_drawable`] hands it to its
/// caller.
enum Drawn<'a> {
    /// Encloses a volume of material, with the volume already derived.
    Volume(&'a dyn Volumetric, SurfaceBoundedBody),
    /// Is one optical surface, with the surface already meshed.
    Surface(&'a dyn Planar, SurfaceMesh),
}

/// Walk a model and hand every component that can be drawn, with its shape, to `draw`.
///
/// Which components a 3D view shows is one decision, and it is made here rather than twice: the
/// scene and the manifest that lists it have to agree on the set, or a viewer would be told about
/// components it can never fetch. A component that is neither a volume nor a surface is passed
/// over - a node reference is not a second component but the same one passed through again, a
/// source port is where light begins rather than a thing, and a group is the nodes it contains, not
/// a component of its own. A component that cannot be drawn is reported and skipped rather than
/// failing the whole walk, so one unusual component does not cost the view of the rest of the setup.
///
/// The shape is derived once here and handed over as [`Drawn`], because deriving it copies the
/// node's whole port set (see [`Volumetric::volume_body`] and [`Planar::surface_mesh`]) and every
/// caller needs it.
///
/// # Arguments
///
/// - `placed`: a model whose nodes carry their placement
/// - `draw`: called once per drawable component with the component and its shape
///
/// # Returns
///
/// The list of drawable components that were skipped, each with a reason. A caller that builds
/// the scene for export does not need this; a caller that builds the manifest hands it to the
/// viewer so the user can see which components are missing and why.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked. An error from `draw` is reported
/// and skipped, not returned.
fn for_each_drawable<F>(placed: &OpmDocument, mut draw: F) -> OpmResult<Vec<SkippedSceneNode>>
where
    F: FnMut(Drawn<'_>) -> OpmResult<()>,
{
    let mut skipped = Vec::new();
    for node_ref in placed.scenery().collect_all_nodes_recursive()? {
        let drawable = if let Some(volume) = node_ref.as_volume() {
            Drawable::Volume(volume)
        } else if let Some(surface) = node_ref.as_surface() {
            Drawable::Surface(surface)
        } else {
            continue;
        };
        if node_ref.node_attr().effective_position().is_none() {
            warn!("{node_ref} has no place in the setup and is left out of the scene");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                name: node_ref.name().to_string(),
                reason: "has no place in the setup".to_string(),
            });
            continue;
        }
        let drawn = match drawable {
            Drawable::Volume(volume) => volume
                .volume_body()
                .and_then(|body| draw(Drawn::Volume(volume, body))),
            Drawable::Surface(surface) => surface
                .surface_mesh(RIM_SEGMENTS)
                .and_then(|mesh| draw(Drawn::Surface(surface, mesh))),
        };
        if let Err(e) = drawn {
            warn!("{node_ref} cannot be drawn and is left out of the scene: {e}");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                name: node_ref.name().to_string(),
                reason: e.to_string(),
            });
        }
    }
    Ok(skipped)
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

/// Add one component that is a single optical surface to a scene, placed where the caller says.
///
/// The single-surface counterpart of [`add_volume_at`]: a mirror, a grating, a filter or a detector
/// has no second surface and no medium between them, so its mesh becomes a [`TriMesh`] built from
/// [`Planar::surface_mesh`] rather than a volume's `triangulate`.
///
/// It is drawn as **two** patches, one the mirror image of the other (see [`SurfaceMesh::flipped`]),
/// facing opposite ways. A volume's mesh is a closed shell with an outside and an inside, so one
/// patch always faces the camera; a single surface has no "inside" to hide behind, and without a
/// second, oppositely-facing patch it would vanish the moment the camera crossed to its back -
/// `optoscene`'s `Mirror` and `Opaque` presets do not ask the renderer to draw both faces of one
/// patch (unlike `Translucent`, which does), so the geometry has to supply both itself.
///
/// # Arguments
///
/// - `scene`: the scene to add to
/// - `node`: the component to draw
/// - `mesh`: the component's meshed surface, already derived
/// - `placement`: where to put the component
///
/// # Errors
///
/// This function returns an error if `optoscene` rejects the resulting geometry.
fn add_surface_at(
    scene: &mut Scene,
    node: &dyn Planar,
    mesh: &SurfaceMesh,
    placement: Isometry3<f64>,
) -> OpmResult<()> {
    let material = scene.add_material(material_of(node.appearance()));
    let mesh_id = scene
        .add_mesh(TriMesh {
            patches: vec![
                patch_of(mesh, material),
                patch_of(&mesh.clone().flipped(), material),
            ],
        })
        .map_err(|e| {
            OpossumError::Other(format!("the mesh of '{}' was rejected: {e}", node.name()))
        })?;
    scene
        .add_node(SceneNode {
            uid: node.node_attr().uuid().to_string(),
            name: node.name().to_string(),
            mesh: Some(mesh_id),
            transform: placement,
            layer: Layer::Optics,
            data: None,
        })
        .map_err(|e| OpossumError::Other(format!("'{}' could not be placed: {e}", node.name())))?;
    Ok(())
}

/// Map a core [`Appearance`] to the closest [`optoscene::Material`] preset.
///
/// This is infallible and takes no wavelength: none of the resulting materials carry a refractive
/// index. It replaces the former `surface_material_of` that worked from a coarser
/// [`SurfaceKind`](opossum_core::core_optics::SurfaceKind) and hard-coded colour constants;
/// the colour and roughness now come directly from the [`Appearance`] the node reports.
///
/// # Arguments
///
/// - `appearance`: the cosmetic description a [`Planar`] node provides via
///   [`Planar::appearance`]
///
/// # Returns
///
/// The [`Material`] a renderer should draw the surface in.
fn material_of(appearance: Appearance) -> Material {
    let c = appearance.color;
    let rgb = [c[0], c[1], c[2]];
    match appearance.finish {
        SurfaceFinish::Reflective => Material::Mirror {
            color: rgb,
            roughness: appearance.roughness,
        },
        SurfaceFinish::Transmissive => Material::Translucent {
            color: appearance.color,
        },
        SurfaceFinish::Opaque => Material::Opaque {
            color: appearance.color,
            metallic: 0.0,
            roughness: appearance.roughness,
        },
        // `Appearance` is renderer-agnostic and carries no thin-film parameters, so the
        // backend supplies sensible defaults here.
        SurfaceFinish::Iridescent => Material::Iridescent {
            color: rgb,
            roughness: appearance.roughness,
            iridescence: 1.0,
            iridescence_ior: 1.3,
            iridescence_thickness_min: 100.0,
            iridescence_thickness_max: 400.0,
        },
    }
}

/// Build the glTF binary of a single volume component, in the component's own frame.
///
/// The component sits at the coordinate origin rather than where the setup puts it, and that is
/// what makes the file worth fetching on its own: a component that merely moved keeps its file, and
/// a viewer places it from the manifest instead of asking for its mesh again.
///
/// It also needs no positioning run — what a component looks like does not depend on where it ended
/// up. The caller does have to hand over a node whose surfaces match its properties, which is not a
/// given: patching a property does not rebuild them (see `known_issue_update_surfaces.md`).
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

/// Build the glTF binary of a single surface component, in the component's own frame.
///
/// The [`Planar`] counterpart of [`glb_of_node`] - see there for why the component sits at the
/// coordinate origin and needs no positioning run. Unlike it, this takes no wavelength: none of the
/// materials [`material_of`] produces carry a refractive index.
///
/// # Arguments
///
/// - `node`: the component to draw
///
/// # Returns
///
/// The GLB bytes of a scene holding nothing but this component.
///
/// # Errors
///
/// This function returns an error if the component's surface cannot be meshed, or if the scene
/// cannot be written.
pub fn glb_of_surface_node(node: &dyn Planar) -> OpmResult<Vec<u8>> {
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    add_surface_at(
        &mut scene,
        node,
        &node.surface_mesh(RIM_SEGMENTS)?,
        Isometry3::identity(),
    )?;
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// The colour the optical axis is drawn in: a warm amber that stands apart from the wavelength
/// colours real rays are drawn in and from the grey and glass tones of the components.
pub const AXIS_COLOR: [f32; 4] = [1.0, 0.75, 0.1, 1.0];

/// Build the glTF binary of ray bundles, drawn as lines along the paths the rays took.
///
/// Every ray carries its position history from its source on, so a bundle that left a model
/// draws the whole way it came. A bundle is split by wavelength first, so that each part can be
/// drawn in its own colour. Rays that were stopped on the way - by an aperture, say - are drawn
/// up to where they stopped rather than left out: where light is lost is part of the picture.
///
/// The rays are already in world coordinates, so the scene is pinned at the coordinate origin
/// like [`glb_of_node`], and a viewer places the file where the components are without moving it.
///
/// # Arguments
///
/// - `bundles`: the ray bundles to draw
/// - `color`: the colour of every line, or `None` to colour each by its wavelength
/// - `max_lines`: how many rays of each wavelength of a bundle are drawn at most
///
/// # Returns
///
/// The GLB bytes of a scene holding the lines. It is empty but still a valid file if no ray
/// travelled anywhere.
///
/// # Errors
///
/// This function returns an error if a bundle cannot be split by wavelength, or if the scene
/// cannot be built or written.
pub fn rays_glb(bundles: &[Rays], color: Option<[f32; 4]>, max_lines: usize) -> OpmResult<Vec<u8>> {
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    let style = RayStyle {
        max_lines,
        line_color: color,
        ..RayStyle::default()
    };
    for (bundle_nr, bundle) in bundles.iter().enumerate() {
        if bundle.nr_of_rays(false) == 0 {
            continue;
        }
        let (parts, wavelengths) = bundle.split_ray_bundle_by_wavelength(nanometer!(1.0), false)?;
        for (part_nr, (part, wavelength)) in parts.iter().zip(wavelengths).enumerate() {
            let trace = ray_trace_of(part, format!("rays/{bundle_nr}/{part_nr}"), wavelength);
            // A path needs two points to be drawn at all.
            if trace.stations.len() < 2 {
                continue;
            }
            scene
                .add_ray_trace(&trace, &style)
                .map_err(|e| OpossumError::Other(format!("the rays could not be drawn: {e}")))?;
        }
    }
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// Lay the position histories of a bundle out as the stations of an `optoscene` ray trace.
///
/// Station `k` holds the `k`-th point of every ray's path. Paths are not equally long - a ray
/// stopped early has fewer points - so a ray that has none left at a station is missing there,
/// which is how `optoscene` tells a lost ray.
///
/// # Arguments
///
/// - `rays`: the rays whose paths to lay out
/// - `uid`: the identifier the trace is drawn under
/// - `wavelength`: the wavelength the lines are coloured by
///
/// # Returns
///
/// The ray trace, in metres.
fn ray_trace_of(rays: &Rays, uid: String, wavelength: Length) -> RayTrace {
    let paths: Vec<Vec<Point3<f64>>> = rays
        .iter()
        .map(|ray| {
            ray.position_history_with_current()
                .row_iter()
                .map(|point| {
                    Point3::new(
                        point[0].get::<meter>(),
                        point[1].get::<meter>(),
                        point[2].get::<meter>(),
                    )
                })
                .collect()
        })
        .collect();
    let nr_of_stations = paths.iter().map(Vec::len).max().unwrap_or(0);
    RayTrace {
        uid,
        stations: (0..nr_of_stations)
            .map(|k| paths.iter().map(|path| path.get(k).copied()).collect())
            .collect(),
        wavelength: Some(wavelength.get::<meter>()),
    }
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
/// One entry per drawable component, in world coordinates.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked.
// Placement is narrowed to `f32` on the way out. Nothing is computed from it - it positions a
// picture - and `f32` still resolves a metre to well under a micrometre; see
// [`SceneNodeEntry::position`].
#[allow(clippy::cast_possible_truncation)]
pub fn manifest_of(placed: &OpmDocument, wavelength: Length) -> OpmResult<SceneManifest> {
    let mut nodes = Vec::new();
    let skipped = for_each_drawable(placed, |drawn| {
        let (uid, name, geometry, transform) = match drawn {
            Drawn::Volume(node, body) => (
                node.node_attr().uuid(),
                node.name().to_string(),
                geometry_of(node, &body, wavelength)?,
                body.isometry().get_transform(),
            ),
            Drawn::Surface(node, mesh) => (
                node.node_attr().uuid(),
                node.name().to_string(),
                surface_geometry_of(node, &mesh)?,
                node.effective_node_iso()
                    .unwrap_or_else(Isometry::identity)
                    .get_transform(),
            ),
        };
        let position = transform.translation.vector;
        let rotation = transform.rotation.quaternion();
        nodes.push(SceneNodeEntry {
            uid,
            group: parent_group_id_or_self(placed.scenery(), uid)?,
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
    Ok(SceneManifest { nodes, skipped })
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
        hash.write_mesh(face);
    }
    hash.write_material(&glass_of(node, body, wavelength)?);
    Ok(hash.finish())
}

/// The [`Planar`] counterpart of [`geometry_of`].
///
/// # Arguments
///
/// - `node`: the component whose looks are wanted
/// - `mesh`: its meshed surface, already derived
///
/// # Returns
///
/// A hexadecimal digest of the component's mesh and material.
///
/// # Errors
///
/// This function never actually fails — [`material_of`] is infallible — but returns
/// `OpmResult` to read the same as [`geometry_of`] at the one call site both are used from.
fn surface_geometry_of(node: &dyn Planar, mesh: &SurfaceMesh) -> OpmResult<String> {
    let mut hash = Fnv1a::new();
    hash.write_mesh(mesh);
    hash.write_material(&material_of(node.appearance()));
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

    /// Fold one meshed face's points, normals and triangles into the hash.
    fn write_mesh(&mut self, mesh: &SurfaceMesh) {
        for point in mesh.points() {
            self.write_f64(point.x.value);
            self.write_f64(point.y.value);
            self.write_f64(point.z.value);
        }
        for normal in mesh.normals() {
            self.write_f64(normal.x);
            self.write_f64(normal.y);
            self.write_f64(normal.z);
        }
        for triangle in mesh.triangles() {
            for corner in triangle {
                self.write(&corner.to_le_bytes());
            }
        }
    }

    /// Fold a material into the hash.
    ///
    /// Written out variant by variant on purpose: were `optoscene` to grow another material, this
    /// stops compiling rather than quietly giving two different materials the same hash.
    fn write_material(&mut self, material: &Material) {
        // Iridescent carries five numbers, more than the uniform (color, [f32; 2]) shape the
        // other variants share below, so it is folded in on its own path.
        if let Material::Iridescent {
            color,
            roughness,
            iridescence,
            iridescence_ior,
            iridescence_thickness_min,
            iridescence_thickness_max,
        } = material
        {
            self.write(&[5]);
            for value in color.iter().copied().chain([
                *roughness,
                *iridescence,
                *iridescence_ior,
                *iridescence_thickness_min,
                *iridescence_thickness_max,
            ]) {
                self.write(&value.to_le_bytes());
            }
            return;
        }
        // Textured carries image bytes plus five numbers, so it too is folded in on its own path.
        if let Material::Textured {
            image_png,
            tint,
            metallic,
            roughness,
            uv_scale,
        } = material
        {
            self.write(&[6]);
            self.write(image_png);
            for value in tint
                .iter()
                .copied()
                .chain([*metallic, *roughness, *uv_scale])
            {
                self.write(&value.to_le_bytes());
            }
            return;
        }
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
            // Handled above by the early returns; these arms keep the match exhaustive, so a future
            // material variant still fails to compile here rather than colliding hashes.
            Material::Iridescent { .. } | Material::Textured { .. } => return,
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
        color: node.glass_color(),
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
        apertures::{ApertureShape, CircleShape},
        core_optics::{OpticNode, node_attr::NodePositioning},
        degree,
        geometry::body::CLEAR_APERTURE,
        joule,
        light::{Ray, lightdata::ray_data_builder::RayDataBuilder},
        material::default_reference_wavelength,
        millimeter, nanometer,
        nodes::{
            CylindricLens, EnergyMeter, Lens, NodeGroup, NodeReference, ReflectiveGrating,
            SourcePort, ThinMirror, Wedge, round_collimated_ray_builder,
        },
    };
    use uuid::Uuid;

    /// A model holding one of everything: three components that enclose a volume, one that is a
    /// single surface (a detector), a source port that is neither, and a reference, which is the
    /// same lens passed through a second time rather than another component.
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

    /// Only what encloses a volume or is one optical surface is drawn. A source port has no shape
    /// of its own, and a reference is the very lens already in the scene, met a second time.
    #[test]
    fn only_the_volumes_and_surfaces_are_drawn() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        let drawn = drawn_components(&scene);
        // The lens, the wedge and the cylindric lens enclose a volume; the detector is one surface.
        assert_eq!(drawn.len(), 4);
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

    /// A mirror is drawn reflective and a detector is drawn as a plain opaque surface - the two
    /// categories the plan calls for, told apart in the exported glTF by the material name
    /// `optoscene` gives each preset (`"mirror"`, `"opaque"`).
    #[test]
    fn a_mirror_and_a_detector_get_reflective_and_opaque_materials() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let detector = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(mirror, "output_1", detector, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let material_names: Vec<&str> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .map(|material| material["name"].as_str().expect("a named material"))
            .collect();
        assert!(
            material_names.contains(&"mirror"),
            "the mirror was not drawn reflective: {material_names:?}"
        );
        assert!(
            material_names.contains(&"opaque"),
            "the detector was not drawn as a plain opaque surface: {material_names:?}"
        );
        Ok(())
    }

    /// A reflective grating overrides its default appearance to look iridescent, and the exported
    /// glTF must therefore carry the `KHR_materials_iridescence` extension.
    #[test]
    fn a_grating_is_drawn_iridescent() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        // The default grating (1740 lines/mm, order -1) diffracts 1000 nm light at its Littrow
        // angle; orient it exactly at Littrow (zero degrees off) for that wavelength so the order
        // is a valid, non-evanescent reflection and the grating is placed and drawn. The source
        // emits 1000 nm to match (`round_collimated_ray_builder`).
        let grating =
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))?;
        let grating = scenery.add_node(grating)?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        // The grating needs actual rays to define its diffraction up-direction; use a small
        // collimated bundle rather than the empty default builder.
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 1)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let material_names: Vec<&str> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .map(|material| material["name"].as_str().expect("a named material"))
            .collect();
        assert!(
            material_names.contains(&"iridescent"),
            "the grating was not drawn iridescent: {material_names:?}"
        );
        assert!(
            scene["extensionsUsed"]
                .as_array()
                .expect("the extensions are listed")
                .iter()
                .any(|used| used == "KHR_materials_iridescence"),
            "KHR_materials_iridescence was not declared in extensionsUsed"
        );
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
            "a surface node must draw both faces, not just the one `Planar::surface_mesh` returns"
        );
        Ok(())
    }

    /// The single-component endpoint works for a surface node exactly as it does for a volume one.
    #[test]
    fn a_surface_nodes_own_file_is_a_valid_glb() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let surface = mirror.as_surface().expect("a mirror is one surface");
        let glb = glb_of_surface_node(surface)?;
        assert!(glb.starts_with(b"glTF"));
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

        let smaller: ApertureShape = CircleShape::new(millimeter!(5.0))?.into();
        document.scenery_mut().with_node_attr_mut(mirror, |attr| {
            attr.set_property(CLEAR_APERTURE, smaller.into())
        })??;

        let after = manifest(&document)?;
        assert_eq!(after.nodes.len(), 1);
        assert_ne!(
            before.nodes[0].geometry, after.nodes[0].geometry,
            "shrinking the mirror's clear aperture did not change its reported geometry"
        );
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
    ///
    /// `a_bit_of_everything`'s scene now also draws a detector, whose material is a plain opaque
    /// one and rightly carries no index of refraction at all - this only inspects the materials
    /// that state one in the first place, and separately pins down that there is at least one.
    #[test]
    fn glass_is_exported_with_its_refractive_index() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        let materials_with_ior: Vec<_> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .filter(|material| !material["extensions"]["KHR_materials_ior"].is_null())
            .collect();
        assert!(
            !materials_with_ior.is_empty(),
            "the scene has three glass components, but none stated a refractive index"
        );
        for material in materials_with_ior {
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
    /// `parent_group_id_or_self` has to recurse for rather than answering with the root.
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

    /// The file a component's endpoint serves has to follow an edit to that component's shape.
    ///
    /// The trap this pins down: a property is patched straight onto the live document, but the
    /// surfaces a volume is derived from are rebuilt by `update_surfaces()`, which that patch does
    /// not run. Deriving geometry from the live document therefore yields the *old* shape, while the
    /// manifest - built from a `positioned_copy`, and so from a document that was serialized and
    /// read back - reports a new hash. The viewer then dutifully refetches and gets the old mesh
    /// back, and changing a lens thickness appears to do nothing at all.
    #[test]
    fn reshaping_a_component_changes_the_file_its_endpoint_serves() -> OpmResult<()> {
        let mut document = two_lenses(millimeter!(100.0))?;
        let lens = document
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .find(|node| node.as_volume().is_some())
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

    /// Refreshing a component's own copy has to draw the same thing as placing the whole model.
    ///
    /// The manifest hashes the shape it finds in a placed copy, while the component's endpoint
    /// serves the shape it derives on its own. If the two derivations disagreed by so much as a
    /// rounding step, every manifest would report a hash for bytes nobody serves, and the viewer
    /// would refetch a component that never stops looking changed.
    #[test]
    fn a_refreshed_component_draws_the_same_as_a_placed_one() -> OpmResult<()> {
        let document = two_lenses(millimeter!(100.0))?;
        let lens = document
            .scenery()
            .collect_all_nodes_recursive()?
            .into_iter()
            .find(|node| node.as_volume().is_some())
            .expect("a model with lenses has something to draw")
            .node_attr()
            .uuid();

        let placed = document.positioned_copy(None)?;
        let from_placed = glb_of_node(
            placed
                .scenery()
                .node_recursive(lens)?
                .0
                .as_volume()
                .expect("the component encloses a volume"),
            default_reference_wavelength(),
        )?;

        // The lookup hands out a deep clone, so refreshing it leaves the document alone.
        let mut own_copy = document.scenery().node_recursive(lens)?.0;
        own_copy.update_surfaces()?;
        let from_refreshed = glb_of_node(
            own_copy
                .as_volume()
                .expect("the component encloses a volume"),
            default_reference_wavelength(),
        )?;

        assert_eq!(
            from_placed, from_refreshed,
            "placing the model and refreshing one component produce different geometry"
        );
        Ok(())
    }

    /// The bytes `GET /scene/node/{uid}.glb` would answer with for one component of a document.
    fn served_geometry_of(document: &OpmDocument, uid: Uuid) -> OpmResult<Vec<u8>> {
        let mut node = document.scenery().node_recursive(uid)?.0;
        node.update_surfaces()?;
        let volume = node.as_volume().expect("the component encloses a volume");
        glb_of_node(volume, default_reference_wavelength())
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

    /// Nothing to draw still makes a file, so a viewer can show "no rays" like any other state.
    #[test]
    fn no_rays_are_still_a_file() -> OpmResult<()> {
        assert!(rays_glb(&[], Some(AXIS_COLOR), 10)?.starts_with(b"glTF"));
        assert!(rays_glb(&[Rays::default()], None, 10)?.starts_with(b"glTF"));
        Ok(())
    }

    /// Rays of one bundle whose paths are not equally long - one stopped early - are drawn up to
    /// where each of them got, as one set of line segments per path leg.
    #[test]
    fn rays_are_drawn_as_line_segments_along_their_paths() -> OpmResult<()> {
        let mut stopped_early = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?;
        let mut went_on = stopped_early.clone();
        stopped_early.propagate(millimeter!(10.0))?;
        went_on.propagate(millimeter!(10.0))?;
        went_on.propagate(millimeter!(10.0))?;
        let mut bundle = Rays::default();
        bundle.add_ray(stopped_early);
        bundle.add_ray(went_on);

        let glb = rays_glb(&[bundle], None, 10)?;
        let scene = glb_json(&glb);
        let primitive = &scene["meshes"][0]["primitives"][0];
        assert_eq!(primitive["mode"], 1, "drawn as LINES");
        let indices = primitive["indices"].as_u64().expect("lines are indexed");
        let index_count =
            &scene["accessors"][usize::try_from(indices).unwrap_or(usize::MAX)]["count"];
        assert_eq!(
            index_count,
            2 * (1 + 2),
            "one leg for the first ray, two for the second"
        );
        Ok(())
    }

    /// The same rays give the same bytes, so a viewer can tell an unchanged file from a new one.
    #[test]
    fn the_same_rays_give_the_same_file() -> OpmResult<()> {
        let axes = two_lenses(millimeter!(100.0))?.positioning_rays(None)?;
        assert!(!axes.is_empty(), "the model has an open output");
        assert_eq!(
            rays_glb(&axes, Some(AXIS_COLOR), 10)?,
            rays_glb(&axes, Some(AXIS_COLOR), 10)?
        );
        Ok(())
    }
}
