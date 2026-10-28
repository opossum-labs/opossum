//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which knows the shape of every component, and the
//! file format from `optoscene`, which knows glTF. This module is only the seam between the two: it
//! decides what of a model gets drawn at all, and which material it is drawn in.

use log::warn;
use nalgebra::{Isometry3, Point3};
use opossum_core::{
    analyzers::Analyzable,
    core_optics::{Appearance, NodeAttrExt, OpticNodeExt, SurfaceFinish},
    error::{OpmResult, OpossumError},
    geometry::{Geometry, SurfaceMesh, body::Body},
    light::Rays,
    nanometer,
    nodes::NodeGroup,
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

/// How opaque glass is drawn where nothing mirrors it.
///
/// A renderer such as three.js shows only opaque objects through transmissive glass, so a lens
/// behind another lens would vanish; drawn slightly see-through, it shows through instead (see the
/// handbook's chapter on the 3D view).
const GLASS_OPACITY: f32 = 0.7;

/// Build a 3D scene of every component of a model that states an appearance.
///
/// Every node that states how it looks (see
/// [`OpticNode::appearance`](opossum_core::core_optics::OpticNode::appearance)) is meshed and put
/// into the scene; every other one is passed over. A component that cannot be meshed is reported
/// and skipped rather than failing the whole scene, so one unusual lens does not cost the view of
/// the rest of the setup.
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
/// A scene holding one node per drawable component.
///
/// # Errors
///
/// This function returns an error if the nodes cannot be walked.
pub fn scene_of(placed: &NodeGroup, wavelength: Length) -> OpmResult<Scene> {
    let mut scene = Scene::new(SceneOptions::default());
    for_each_drawable(placed, wavelength, |drawn| {
        let placement = placement_of(drawn.node);
        add_at(&mut scene, drawn, placement)
    })?;
    Ok(scene)
}

/// Where a component is drawn: where its node sits, which is where its geometry is traced.
///
/// # Arguments
///
/// - `node`: the placed component
fn placement_of(node: &(impl OpticNodeExt + ?Sized)) -> Isometry3<f64> {
    node.effective_node_iso()
        .unwrap_or_else(Isometry::identity)
        .get_transform()
}

/// A component as it is drawn: its patches, each in the material of the face it lies on.
struct Drawn<'a> {
    /// The component.
    node: &'a dyn Analyzable,
    /// The component's patches, meshed in its own frame and ready to draw, with their materials.
    patches: Vec<(SurfaceMesh, Material)>,
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
    fn new(
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
/// components it can never fetch. A component that states no appearance is passed over. A
/// component that cannot be drawn is reported and skipped rather than failing the whole walk, so
/// one unusual component does not cost the view of the rest of the setup.
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
fn for_each_drawable<F>(
    placed: &NodeGroup,
    wavelength: Length,
    mut draw: F,
) -> OpmResult<Vec<SkippedSceneNode>>
where
    F: FnMut(Drawn<'_>) -> OpmResult<()>,
{
    let mut skipped = Vec::new();
    for node_ref in placed.collect_all_nodes_recursive()? {
        let Some(appearance) = node_ref.appearance() else {
            continue;
        };
        if node_ref.node_attr().effective_position().is_none() {
            warn!("{node_ref} has no place in the setup and is left out of the scene");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                group: parent_group_id_or_self(placed, node_ref.uuid())?,
                name: node_ref.name().to_string(),
                reason: "has no place in the setup".to_string(),
            });
            continue;
        }
        let drawn = Drawn::new(&*node_ref, appearance, wavelength).and_then(&mut draw);
        if let Err(e) = drawn {
            warn!("{node_ref} cannot be drawn and is left out of the scene: {e}");
            skipped.push(SkippedSceneNode {
                uid: node_ref.uuid(),
                group: parent_group_id_or_self(placed, node_ref.uuid())?,
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

/// Map the [`Appearance`] of one face of a component to the closest [`optoscene::Material`]
/// preset.
///
/// Glass is the one material that needs more than the appearance, which only gives its tint and
/// how mirrored it is: see [`glass_of`].
///
/// # Arguments
///
/// - `node`: the component, named in errors
/// - `appearance`: what the face asks to be drawn in
/// - `body`: the volume the component's geometry encloses, if any
/// - `ior`: the refractive index of the medium that volume is filled with, which glass is drawn
///   with
///
/// # Returns
///
/// The [`Material`] a renderer should draw the face in.
///
/// # Errors
///
/// This function returns an error if glass is asked for but cannot be described (see
/// [`glass_of`]).
fn material_of(
    node: &dyn Analyzable,
    appearance: Appearance,
    body: Option<&dyn Body>,
    ior: f64,
) -> OpmResult<Material> {
    let [red, green, blue, alpha] = appearance.color;
    let rgb = [red, green, blue];
    Ok(match appearance.finish {
        SurfaceFinish::Reflective => Material::Mirror {
            color: rgb,
            roughness: appearance.roughness,
            metallic: appearance.metallic,
            opacity: alpha,
        },
        SurfaceFinish::Transmissive => Material::Translucent {
            color: appearance.color,
        },
        SurfaceFinish::Opaque => Material::Opaque {
            color: appearance.color,
            metallic: appearance.metallic,
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
        SurfaceFinish::Glass => glass_of(node, appearance, body, ior)?,
    })
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
/// The GLB bytes of a scene holding nothing but this component, or `None` if the component states
/// no appearance and is not drawn at all.
///
/// # Errors
///
/// This function returns an error if the component cannot be drawn (see [`Drawn::new`]), or if the
/// scene cannot be written.
pub fn glb_of_node(node: &dyn Analyzable, wavelength: Length) -> OpmResult<Option<Vec<u8>>> {
    let Some(appearance) = node.appearance() else {
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

/// The colour the optical axis is drawn in: a bright purple that stands apart from the wavelength
/// colours real rays are drawn in and from the grey and glass tones of the components.
pub const AXIS_COLOR: [f32; 4] = [183. / 255., 17. / 255., 1., 1.0];

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
            let style = RayStyle {
                max_lines,
                line_color: color.or_else(|| invisible_light_color(wavelength)),
                ..RayStyle::default()
            };
            scene
                .add_ray_trace(&trace, &style)
                .map_err(|e| OpossumError::Other(format!("the rays could not be drawn: {e}")))?;
        }
    }
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}

/// The colour infrared light is drawn in: a deep red, next to the visible end of the spectrum it
/// lies beyond.
const INFRARED_COLOR: [f32; 4] = [0.8, 0.05, 0.05, 1.0];
/// The colour ultraviolet light is drawn in: a violet, next to the visible end of the spectrum it
/// lies beyond.
const ULTRAVIOLET_COLOR: [f32; 4] = [0.45, 0.1, 0.9, 1.0];

/// The colour light of a wavelength the eye cannot see is drawn in.
///
/// Visible light is coloured by `optoscene` after its wavelength. Beyond the visible range it
/// falls back to one magenta for everything, which would make an infrared laser look like the
/// optical axis; so infrared and ultraviolet light get colours of their own here, taken from the
/// end of the spectrum each lies beyond.
///
/// # Arguments
///
/// - `wavelength`: the vacuum wavelength of the light
///
/// # Returns
///
/// The colour for infrared or ultraviolet light, or `None` for visible light.
fn invisible_light_color(wavelength: Length) -> Option<[f32; 4]> {
    let nm = wavelength.get::<uom::si::length::nanometer>();
    if nm > 780.0 {
        Some(INFRARED_COLOR)
    } else if nm < 380.0 {
        Some(ULTRAVIOLET_COLOR)
    } else {
        None
    }
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
            group: parent_group_id_or_self(placed, uid)?,
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
        let (tag, color, numbers): (u8, &[f32], [f32; 4]) = match material {
            Material::Glass {
                color,
                ior,
                thickness,
                metallic,
                opacity,
            } => (0, color, [*ior, *thickness, *metallic, *opacity]),
            Material::Mirror {
                color,
                roughness,
                metallic,
                opacity,
            } => (1, color, [*roughness, *metallic, *opacity, 0.0]),
            Material::Opaque {
                color,
                metallic,
                roughness,
            } => (2, color, [*metallic, *roughness, 0.0, 0.0]),
            Material::Unlit { color } => (3, color, [0.0; 4]),
            Material::Translucent { color } => (4, color, [0.0; 4]),
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

/// Return the refractive index of the medium a component's solid is filled with.
///
/// A medium that states no index at the wavelength is drawn as plain glass, with a warning.
///
/// # Arguments
///
/// - `node`: the component, named in errors and warnings
/// - `body`: the volume the component's geometry encloses
/// - `wavelength`: the wavelength to evaluate the refractive index at
///
/// # Errors
///
/// This function returns an error if the volume holds no single material.
fn refractive_index_of(
    node: &dyn Analyzable,
    body: &dyn Body,
    wavelength: Length,
) -> OpmResult<f64> {
    Ok(body
        .material()
        .ok_or_else(|| {
            OpossumError::Other(format!(
                "'{}' is filled with no single material to draw as glass",
                node.name()
            ))
        })?
        .refractive_index(wavelength)
        .unwrap_or_else(|e| {
            warn!(
                "'{}' states no refractive index at this wavelength ({e}), drawing it as plain \
                 glass",
                node.name()
            );
            FALLBACK_REFRACTIVE_INDEX
        }))
}

/// Describe the glass one face of a component is drawn in, as far as a renderer needs it.
///
/// Its thickness is the extent of the component's solid along its axis. Where nothing mirrors the
/// face, it is drawn at [`GLASS_OPACITY`]; the more it is mirrored, the more opaque it gets.
///
/// # Arguments
///
/// - `node`: the component, named in errors
/// - `appearance`: the tint of the glass and how mirrored the face is
/// - `body`: the volume the component's geometry encloses, if any
/// - `ior`: the refractive index of the medium that volume is filled with
///
/// # Errors
///
/// This function returns an error if the component's geometry encloses no volume, or if that
/// volume has no extent to measure a thickness from.
// A refractive index and a thickness in millimeters are nowhere near the range of an f32.
#[allow(clippy::cast_possible_truncation)]
fn glass_of(
    node: &dyn Analyzable,
    appearance: Appearance,
    body: Option<&dyn Body>,
    ior: f64,
) -> OpmResult<Material> {
    let body = body.ok_or_else(|| {
        OpossumError::Other(format!(
            "'{}' encloses no volume to draw as glass",
            node.name()
        ))
    })?;
    let thickness = body.bounding_box()?.z_range();
    let [red, green, blue, _] = appearance.color;
    Ok(Material::Glass {
        color: [red, green, blue],
        ior: ior as f32,
        thickness: (thickness.end - thickness.start).value as f32,
        metallic: appearance.metallic,
        opacity: (1.0 - GLASS_OPACITY).mul_add(appearance.metallic, GLASS_OPACITY),
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
        apertures::{Bore, CircleShape, OpticShape, Outline},
        coatings::{CoatingConstantR, CoatingType},
        core_optics::{OpticNode, node_attr::NodePositioning},
        degree,
        geometry::{Extruded, SurfaceGeometry, body::OPTIC_SHAPE},
        joule,
        light::{Ray, lightdata::ray_data_builder::RayDataBuilder},
        material::default_reference_wavelength,
        millimeter, nanometer,
        nodes::{
            BeamSplitter, CylindricLens, EnergyMeter, IdealFilter, Lens, NodeReference,
            ReflectiveGrating, SourcePort, ThinMirror, Wedge, round_collimated_ray_builder,
        },
        opm_document::OpmDocument,
        percent,
        properties::Proptype,
    };
    use uuid::Uuid;

    /// A model holding one of everything: three components that enclose a volume, two that are a
    /// single surface (a mirror and a detector), a source port that is neither, and a reference,
    /// which is the same lens passed through a second time rather than another component. The
    /// mirror folds the beam back, so that the light can take that second pass at all.
    fn a_bit_of_everything() -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let wedge = scenery.add_node(Wedge::default())?;
        let cylinder = scenery.add_node(CylindricLens::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let mut lens_again = NodeReference::from_node(&scenery.node_recursive(lens)?.0)?;
        lens_again.set_inverted(true)?;
        let lens_again = scenery.add_node(lens_again)?;
        let detector = scenery.add_node(EnergyMeter::default())?;
        let hop = millimeter!(50.0);
        scenery.connect_nodes(source, "output_1", wedge, "input_1", hop)?;
        scenery.connect_nodes(wedge, "output_1", cylinder, "input_1", hop)?;
        scenery.connect_nodes(cylinder, "output_1", lens, "input_1", hop)?;
        scenery.connect_nodes(lens, "output_1", mirror, "input_1", hop)?;
        // Passed backwards, the lens is entered through what is its output port.
        scenery.connect_nodes(mirror, "output_1", lens_again, "output_1", hop)?;
        scenery.connect_nodes(lens_again, "input_1", detector, "input_1", hop)?;

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
        let placed = document.placed_scenery(None)?;
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

    /// A filter lets light through, so it is drawn as a translucent surface (`optoscene`'s
    /// `"translucent"` preset) rather than a reflective or an opaque one.
    #[test]
    fn a_filter_is_drawn_translucent() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let filter = scenery.add_node(IdealFilter::default())?;
        scenery.connect_nodes(source, "output_1", filter, "input_1", millimeter!(50.0))?;
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
        assert_eq!(material_names, ["translucent"]);
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
    /// The materials a component's own file draws its patches in, patch by patch.
    fn patch_materials(node: &dyn Analyzable) -> OpmResult<Vec<serde_json::Value>> {
        let glb = glb_of_node(node, default_reference_wavelength())?
            .ok_or_else(|| OpossumError::Other("the component is not drawn".into()))?;
        let scene = glb_json(&glb);
        let mesh_index = drawn_components(&scene)[0]["mesh"]
            .as_u64()
            .expect("a drawn component has a mesh");
        Ok(scene["meshes"][mesh_index as usize]["primitives"]
            .as_array()
            .expect("a mesh has primitives")
            .iter()
            .map(|primitive| {
                let index = primitive["material"]
                    .as_u64()
                    .expect("a patch has a material");
                scene["materials"][index as usize].clone()
            })
            .collect())
    }
    /// How metallic and how opaque a material is drawn; one without an alpha mode is opaque.
    fn metallic_and_opacity(material: &serde_json::Value) -> (f64, f64) {
        let pbr = &material["pbrMetallicRoughness"];
        let metallic = pbr["metallicFactor"].as_f64().expect("a metallic factor");
        let opacity = material.get("alphaMode").map_or(1.0, |mode| {
            assert_eq!(mode, "BLEND");
            pbr["baseColorFactor"][3].as_f64().expect("an alpha")
        });
        (metallic, opacity)
    }
    /// Check that a material is drawn as metallic and as opaque as expected.
    fn assert_drawn_as(material: &serde_json::Value, metallic: f64, opacity: f64) {
        let (drawn_metallic, drawn_opacity) = metallic_and_opacity(material);
        assert!(
            (drawn_metallic - metallic).abs() < 1e-6 && (drawn_opacity - opacity).abs() < 1e-6,
            "drawn {drawn_metallic} metallic and {drawn_opacity} opaque instead of {metallic} and \
             {opacity}: {material}"
        );
    }
    /// A mirror is drawn as mirrored as its coating makes it: an opaque mirror by default, the pale
    /// pane it is without a coating that reflects.
    #[test]
    fn a_mirror_is_drawn_as_mirrored_as_its_coating_makes_it() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        for material in patch_materials(&mirror)? {
            assert_drawn_as(&material, 1.0, 1.0);
        }
        mirror.set_coating(SurfaceGeometry::FACE, &CoatingType::IdealAR)?;
        for material in patch_materials(&mirror)? {
            assert_drawn_as(&material, 0.0, 0.3);
        }
        Ok(())
    }
    /// A beam splitter reflects half of the light, so it is drawn half mirrored.
    #[test]
    fn a_beam_splitter_is_drawn_half_mirrored() -> OpmResult<()> {
        let mut splitter = BeamSplitter::default();
        splitter.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        for material in patch_materials(&splitter)? {
            assert_drawn_as(&material, 0.5, 0.65);
        }
        Ok(())
    }
    /// Every face of a lens is drawn as slightly see-through glass, so that what lies behind it
    /// stays visible; a coating that mirrors one of its faces mirrors that face alone.
    #[test]
    fn a_mirror_coating_on_a_lens_mirrors_only_its_own_face() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let materials = patch_materials(&lens)?;
        assert_eq!(materials.len(), 3);
        for material in &materials {
            assert_eq!(material["name"], "glass");
            assert_drawn_as(material, 0.0, 0.7);
        }
        lens.set_coating(
            Extruded::REAR,
            &CoatingConstantR::new(percent!(100.0))?.into(),
        )?;
        // An uncoated face reflects by the index of the lens behind it: 4 % for n = 1.5.
        lens.set_coating(Extruded::FRONT, &CoatingType::Fresnel)?;
        let materials = patch_materials(&lens)?;
        // front, rear, lateral face
        assert_drawn_as(&materials[0], 0.04, 0.712);
        assert_drawn_as(&materials[1], 1.0, 1.0);
        assert_drawn_as(&materials[2], 0.0, 0.7);
        Ok(())
    }
    /// A changed coating changes how a component looks, so the viewer has to fetch it again.
    #[test]
    fn changing_a_coating_changes_the_geometry() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        let wavelength = default_reference_wavelength();
        let appearance = mirror.appearance().expect("a mirror is drawn");
        let before = geometry_of(&Drawn::new(&mirror, appearance, wavelength)?);
        mirror.set_coating(SurfaceGeometry::FACE, &CoatingType::IdealAR)?;
        let after = geometry_of(&Drawn::new(&mirror, appearance, wavelength)?);
        assert_ne!(before, after);
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

        let smaller =
            OpticShape::Bounded(Outline::new(CircleShape::new(millimeter!(5.0))?, vec![])?);
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

    /// A mirror whose clear aperture is a stack of shapes — here a disc with a bore beside the
    /// optical axis — is drawn like any other component rather than left out.
    #[test]
    fn a_mirror_with_a_bore_is_drawn() -> OpmResult<()> {
        let bored = OpticShape::Bounded(Outline::new(
            CircleShape::new(millimeter!(12.5))?,
            vec![Bore::new(
                CircleShape::new(millimeter!(2.))?,
                millimeter!(6.0, 0.0),
                degree!(0.),
            )],
        )?);
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

    /// Drawing a setup must not decide anything about the setup itself — least of all freeze the
    /// placements this run worked out into the model.
    #[test]
    fn drawing_a_model_leaves_it_alone() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let before = document.to_opm_file_string()?;
        scene_of(
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
            .find(|node| node.appearance() == Some(Appearance::glass()))
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
            .find(|node| node.appearance() == Some(Appearance::glass()))
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

    /// An infrared laser is drawn red, not in the magenta that would make it look like the axis;
    /// visible light keeps its spectral colour.
    #[test]
    fn infrared_rays_are_drawn_red() -> OpmResult<()> {
        let color_of = |wavelength| -> OpmResult<serde_json::Value> {
            let mut ray = Ray::origin_along_z(wavelength, joule!(1.0))?;
            ray.propagate(millimeter!(10.0))?;
            let mut bundle = Rays::default();
            bundle.add_ray(ray);
            let scene = glb_json(&rays_glb(&[bundle], None, 10)?);
            Ok(scene["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"].clone())
        };
        let infrared = color_of(nanometer!(1053.0))?;
        let red: Vec<f64> = INFRARED_COLOR.iter().map(|c| f64::from(*c)).collect();
        let drawn: Vec<f64> = infrared
            .as_array()
            .expect("a line has a colour")
            .iter()
            .map(|c| c.as_f64().unwrap_or(f64::NAN))
            .collect();
        for (d, r) in drawn.iter().zip(&red) {
            assert!((d - r).abs() < 1e-6, "infrared drawn as {infrared}");
        }
        assert_ne!(
            color_of(nanometer!(550.0))?,
            infrared,
            "visible light keeps its colour"
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
