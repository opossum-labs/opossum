//! The [`Scene`]: the mutable container everything is added to before export.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;

use nalgebra::{Isometry3, Point3, UnitQuaternion};

use crate::error::Error;
use crate::gltf::{builder, container};
use crate::model::{
    Envelope, Layer, Material, MaterialId, MeshId, RayStyle, RayTrace, SceneNode, SurfacePatch,
    TriMesh,
};
use crate::rays::{color, hull, lines};

/// Global export options.
pub struct SceneOptions {
    /// Scene name, written to the glTF `scene`.
    pub name: String,
    /// Reference point subtracted from all world positions before conversion to
    /// `f32`. `None`: center of the world-space AABB of all nodes.
    pub origin: Option<Point3<f64>>,
    /// Rotation applied to the root node. Default identity.
    pub root_rotation: UnitQuaternion<f64>,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            name: String::from("optoscene"),
            origin: None,
            root_rotation: UnitQuaternion::identity(),
        }
    }
}

/// A line mesh: a set of line segments over shared vertices (crate-internal).
///
/// Used for ray lines, which glTF renders as a `LINES`-mode primitive.
pub struct LineMesh {
    /// Vertex positions in the node's local frame, in meters.
    pub positions: Vec<Point3<f64>>,
    /// Line segments as pairs of indices into `positions`.
    pub segments: Vec<[u32; 2]>,
    /// The (unlit) material the lines are drawn with.
    pub material: MaterialId,
}

/// A stored renderable: either a triangle mesh or a line mesh (crate-internal).
pub enum StoredMesh {
    /// A triangulated surface mesh.
    Tri(TriMesh),
    /// A set of line segments.
    Lines(LineMesh),
}

/// A collection of materials, meshes and placed nodes, exportable to glTF/GLB.
///
/// Materials and meshes are content-deduplicated: adding an identical value
/// returns the id already registered. Iteration order for the export always
/// follows insertion order, so the output is deterministic.
pub struct Scene {
    options: SceneOptions,
    materials: Vec<Material>,
    material_lookup: HashMap<u64, MaterialId>,
    meshes: Vec<StoredMesh>,
    mesh_lookup: HashMap<u64, MeshId>,
    nodes: Vec<SceneNode>,
    node_lookup: HashMap<String, usize>,
}

impl Scene {
    /// Creates an empty scene with the given options.
    #[must_use]
    pub fn new(options: SceneOptions) -> Self {
        Self {
            options,
            materials: Vec::new(),
            material_lookup: HashMap::new(),
            meshes: Vec::new(),
            mesh_lookup: HashMap::new(),
            nodes: Vec::new(),
            node_lookup: HashMap::new(),
        }
    }

    /// Registers a material; identical materials return the same id.
    ///
    /// # Arguments
    /// * `material` — the material preset to register.
    ///
    /// # Returns
    /// The id of the (possibly pre-existing) material.
    pub fn add_material(&mut self, material: Material) -> MaterialId {
        let hash = material_hash(&material);
        if let Some(&id) = self.material_lookup.get(&hash) {
            return id;
        }
        let id = MaterialId::new(index_to_u32(self.materials.len()));
        self.material_lookup.insert(hash, id);
        self.materials.push(material);
        id
    }

    /// Validates and registers a mesh; identical meshes return the same id.
    ///
    /// # Arguments
    /// * `mesh` — the triangulated mesh to register.
    ///
    /// # Returns
    /// The id of the (possibly pre-existing) mesh.
    ///
    /// # Errors
    /// Returns [`Error::InvalidMesh`] if the mesh has no patches, a patch has no
    /// vertices or triangles, an index is out of range, a `normals`/`colors`
    /// length does not match the vertex count, or a value is not finite.
    /// Returns [`Error::UnknownMaterial`] if a patch references an unregistered
    /// material.
    pub fn add_mesh(&mut self, mesh: TriMesh) -> Result<MeshId, Error> {
        self.validate_mesh(&mesh)?;
        let hash = mesh_hash(&mesh);
        if let Some(&id) = self.mesh_lookup.get(&hash) {
            return Ok(id);
        }
        let id = MeshId::new(index_to_u32(self.meshes.len()));
        self.mesh_lookup.insert(hash, id);
        self.meshes.push(StoredMesh::Tri(mesh));
        Ok(id)
    }

    /// Registers a line mesh (crate-internal; not content-deduplicated).
    ///
    /// # Arguments
    /// * `line_mesh` — the line geometry to register.
    ///
    /// # Returns
    /// The id of the newly registered mesh.
    pub(crate) fn push_line_mesh(&mut self, line_mesh: LineMesh) -> MeshId {
        let id = MeshId::new(index_to_u32(self.meshes.len()));
        self.meshes.push(StoredMesh::Lines(line_mesh));
        id
    }

    /// Adds a node to the scene.
    ///
    /// # Arguments
    /// * `node` — the placed mesh instance to add.
    ///
    /// # Errors
    /// Returns [`Error::DuplicateUid`] if a node with the same uid already
    /// exists, or [`Error::UnknownMesh`] if `node.mesh` refers to an
    /// unregistered mesh.
    pub fn add_node(&mut self, node: SceneNode) -> Result<(), Error> {
        if self.node_lookup.contains_key(&node.uid) {
            return Err(Error::DuplicateUid(node.uid));
        }
        if let Some(mesh_id) = node.mesh {
            if mesh_id.index() >= self.meshes.len() {
                return Err(Error::UnknownMesh(mesh_id));
            }
        }
        let index = self.nodes.len();
        self.node_lookup.insert(node.uid.clone(), index);
        self.nodes.push(node);
        Ok(())
    }

    /// Adds the line and (optional) envelope nodes for a ray trace to the
    /// [`Layer::Rays`] layer.
    ///
    /// Both meshes are stored relative to the center of the AABB of the trace's
    /// valid ray points, which becomes the node translation; that gives rays the
    /// same `f32` precision as optics. Up to two nodes are created:
    /// `"<uid>/lines"` (a `LINES` primitive) and `"<uid>/envelope"` (a triangle
    /// mesh, according to [`RayStyle::envelope`]).
    ///
    /// # Arguments
    /// * `trace` — the ray trace to visualize.
    /// * `style` — how to draw it (line budget, colors, envelope).
    ///
    /// # Errors
    /// Returns [`Error::InvalidRayTrace`] if the trace has fewer than two
    /// stations, unequal station lengths or a non-finite coordinate;
    /// [`Error::InvalidMesh`]/[`Error::UnknownMaterial`] if a supplied
    /// [`Envelope::Mesh`] is invalid; or [`Error::DuplicateUid`] if the derived
    /// node uids already exist.
    pub fn add_ray_trace(&mut self, trace: &RayTrace, style: &RayStyle) -> Result<(), Error> {
        validate_ray_trace(trace)?;
        let center = ray_center(trace);

        if style.max_lines > 0 {
            if let Some((positions, segments)) =
                lines::decimated_lines(trace, style.max_lines, center)
            {
                let color = line_color(style, trace);
                let material = self.add_material(Material::Unlit { color });
                let mesh = self.push_line_mesh(LineMesh {
                    positions,
                    segments,
                    material,
                });
                self.add_node(SceneNode {
                    uid: format!("{}/lines", trace.uid),
                    name: format!("{}/lines", trace.uid),
                    mesh: Some(mesh),
                    transform: Isometry3::translation(center.x, center.y, center.z),
                    layer: Layer::Rays,
                    data: None,
                })?;
            }
        }

        let envelope_mesh = match &style.envelope {
            Envelope::None => None,
            Envelope::Mesh(mesh) => Some(shift_trimesh(mesh, center)),
            Envelope::DefaultRound { sectors, steps } => {
                let material = self.add_material(Material::Translucent {
                    color: style.envelope_color,
                });
                hull::default_round(trace, *sectors, *steps, center, material)
            }
        };
        if let Some(mesh) = envelope_mesh {
            let mesh_id = self.add_mesh(mesh)?;
            self.add_node(SceneNode {
                uid: format!("{}/envelope", trace.uid),
                name: format!("{}/envelope", trace.uid),
                mesh: Some(mesh_id),
                transform: Isometry3::translation(center.x, center.y, center.z),
                layer: Layer::Rays,
                data: None,
            })?;
        }
        Ok(())
    }

    /// Exports the whole scene as a self-contained GLB.
    ///
    /// # Returns
    /// The GLB bytes.
    ///
    /// # Errors
    /// Returns [`Error::InvalidMesh`] if a normal has near-zero length, or
    /// [`Error::Json`] if serialization fails.
    pub fn to_glb(&self) -> Result<Vec<u8>, Error> {
        let (root, bin) = builder::build(self)?;
        let json = serde_json::to_string(&root)?;
        Ok(container::glb_bytes(json.as_bytes(), &bin))
    }

    /// Writes the whole scene as a GLB to `writer`.
    ///
    /// # Arguments
    /// * `writer` — the sink the GLB bytes are written to.
    ///
    /// # Errors
    /// Returns [`Error::InvalidMesh`]/[`Error::Json`] on build failures, or
    /// [`Error::Io`] if writing fails.
    pub fn write_glb<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let bytes = self.to_glb()?;
        writer.write_all(&bytes)?;
        Ok(())
    }

    /// Returns the JSON document and the binary buffer for a `.gltf` + `.bin`
    /// pair.
    ///
    /// The JSON is identical to the GLB's, except that `buffers[0].uri` is set to
    /// `bin_uri`.
    ///
    /// # Arguments
    /// * `bin_uri` — the URI written into `buffers[0].uri` (e.g. `"scene.bin"`).
    ///
    /// # Returns
    /// The JSON string and the binary buffer (empty if the scene has no
    /// geometry).
    ///
    /// # Errors
    /// Returns a build error, or [`Error::Json`] if serialization fails.
    pub fn to_gltf_separate(&self, bin_uri: &str) -> Result<(String, Vec<u8>), Error> {
        let (mut root, bin) = builder::build(self)?;
        if let Some(buffer) = root.buffers.first_mut() {
            buffer.uri = Some(bin_uri.to_string());
        }
        let json = serde_json::to_string(&root)?;
        Ok((json, bin))
    }

    /// Writes `<stem>.gltf` and (if the scene has geometry) `<stem>.bin` into
    /// `dir`.
    ///
    /// # Arguments
    /// * `dir` — the target directory (must already exist).
    /// * `stem` — the file stem shared by the two outputs.
    ///
    /// # Errors
    /// Returns a build/serialization error, or [`Error::Io`] if writing fails.
    pub fn write_gltf(&self, dir: &Path, stem: &str) -> Result<(), Error> {
        let bin_name = format!("{stem}.bin");
        let (json, bin) = self.to_gltf_separate(&bin_name)?;
        std::fs::write(dir.join(format!("{stem}.gltf")), json)?;
        if !bin.is_empty() {
            std::fs::write(dir.join(bin_name), bin)?;
        }
        Ok(())
    }

    /// Exports only the nodes of one layer, keeping the root/layer structure and
    /// the scene-global origin.
    ///
    /// # Arguments
    /// * `layer` — the layer to export.
    ///
    /// # Returns
    /// The GLB bytes.
    ///
    /// # Errors
    /// Returns a build error, or [`Error::Json`] if serialization fails.
    pub fn layer_to_glb(&self, layer: Layer) -> Result<Vec<u8>, Error> {
        let selected: Vec<&SceneNode> = self
            .nodes
            .iter()
            .filter(|node| node.layer == layer)
            .collect();
        let (root, bin) = builder::build_selected(self, &selected)?;
        let json = serde_json::to_string(&root)?;
        Ok(container::glb_bytes(json.as_bytes(), &bin))
    }

    /// Exports only the given nodes, keeping the root/layer structure and the
    /// scene-global origin.
    ///
    /// Unknown uids are ignored; the exported nodes follow the scene's insertion
    /// order regardless of the order of `uids`.
    ///
    /// # Arguments
    /// * `uids` — the uids of the nodes to export.
    ///
    /// # Returns
    /// The GLB bytes.
    ///
    /// # Errors
    /// Returns a build error, or [`Error::Json`] if serialization fails.
    pub fn nodes_to_glb(&self, uids: &[&str]) -> Result<Vec<u8>, Error> {
        let wanted: HashSet<&str> = uids.iter().copied().collect();
        let selected: Vec<&SceneNode> = self
            .nodes
            .iter()
            .filter(|node| wanted.contains(node.uid.as_str()))
            .collect();
        let (root, bin) = builder::build_selected(self, &selected)?;
        let json = serde_json::to_string(&root)?;
        Ok(container::glb_bytes(json.as_bytes(), &bin))
    }

    /// The effective origin: [`SceneOptions::origin`] if set, otherwise the
    /// center of the world-space axis-aligned bounding box of all node vertices.
    /// Returns the coordinate origin if the scene has no vertices.
    ///
    /// # Returns
    /// The reference point subtracted from world positions before export.
    #[must_use]
    pub fn effective_origin(&self) -> Point3<f64> {
        if let Some(origin) = self.options.origin {
            return origin;
        }
        aabb_center(
            self.nodes
                .iter()
                .filter_map(|node| Some((node, self.meshes.get(node.mesh?.index())?)))
                .flat_map(|(node, stored)| {
                    stored_positions(stored).map(move |p| node.transform.transform_point(p))
                }),
        )
    }

    /// The registered materials in insertion order (crate-internal).
    pub(crate) fn materials(&self) -> &[Material] {
        &self.materials
    }

    /// The registered meshes in insertion order (crate-internal).
    pub(crate) fn meshes(&self) -> &[StoredMesh] {
        &self.meshes
    }

    /// The nodes in insertion order (crate-internal).
    pub(crate) fn nodes(&self) -> &[SceneNode] {
        &self.nodes
    }

    /// The export options (crate-internal).
    pub(crate) const fn options(&self) -> &SceneOptions {
        &self.options
    }

    /// Content hash of a node for diffing: its name, layer, metadata and the
    /// content of the mesh it references (geometry plus resolved material
    /// content). The transform is deliberately excluded — transform changes are
    /// reported separately by the diff, not as content changes.
    #[cfg(feature = "protocol")]
    pub(crate) fn node_content_hash(&self, node: &SceneNode) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write(node.name.as_bytes());
        hasher.write_u8(0xff);
        hasher.write(node.layer.name().as_bytes());
        hasher.write_u8(0xff);
        match &node.data {
            Some(data) => {
                hasher.write_u8(1);
                if let Ok(bytes) = serde_json::to_vec(data) {
                    hasher.write(&bytes);
                }
            }
            None => hasher.write_u8(0),
        }
        match node.mesh.and_then(|id| self.meshes.get(id.index())) {
            Some(StoredMesh::Tri(mesh)) => {
                hasher.write_u8(1);
                hash_tri_mesh(&mut hasher, mesh, |hasher, material| {
                    if let Some(m) = self.materials.get(material.index()) {
                        hasher.write_u64(material_hash(m));
                    }
                });
            }
            Some(StoredMesh::Lines(lines)) => {
                hasher.write_u8(2);
                hasher.write_usize(lines.positions.len());
                for p in &lines.positions {
                    hasher.write_u64(p.x.to_bits());
                    hasher.write_u64(p.y.to_bits());
                    hasher.write_u64(p.z.to_bits());
                }
                hasher.write_usize(lines.segments.len());
                for segment in &lines.segments {
                    hasher.write_u32(segment[0]);
                    hasher.write_u32(segment[1]);
                }
                if let Some(m) = self.materials.get(lines.material.index()) {
                    hasher.write_u64(material_hash(m));
                }
            }
            None => hasher.write_u8(0),
        }
        hasher.finish()
    }

    /// Validates a mesh against the rules documented on [`Self::add_mesh`].
    fn validate_mesh(&self, mesh: &TriMesh) -> Result<(), Error> {
        if mesh.patches.is_empty() {
            return Err(Error::InvalidMesh("mesh has no patches".to_string()));
        }
        for (pi, patch) in mesh.patches.iter().enumerate() {
            if patch.positions.is_empty() {
                return Err(Error::InvalidMesh(format!("patch {pi} has no vertices")));
            }
            if patch.indices.is_empty() {
                return Err(Error::InvalidMesh(format!("patch {pi} has no triangles")));
            }
            for (vi, p) in patch.positions.iter().enumerate() {
                if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                    return Err(Error::InvalidMesh(format!(
                        "patch {pi} vertex {vi} position is not finite"
                    )));
                }
            }
            let vertex_count = patch.positions.len();
            if let Some(normals) = &patch.normals {
                if normals.len() != vertex_count {
                    return Err(Error::InvalidMesh(format!(
                        "patch {pi}: {} normals for {vertex_count} vertices",
                        normals.len()
                    )));
                }
                for (vi, n) in normals.iter().enumerate() {
                    if !(n.x.is_finite() && n.y.is_finite() && n.z.is_finite()) {
                        return Err(Error::InvalidMesh(format!(
                            "patch {pi} normal {vi} is not finite"
                        )));
                    }
                }
            }
            if let Some(colors) = &patch.colors {
                if colors.len() != vertex_count {
                    return Err(Error::InvalidMesh(format!(
                        "patch {pi}: {} colors for {vertex_count} vertices",
                        colors.len()
                    )));
                }
                for (vi, c) in colors.iter().enumerate() {
                    if !c.iter().all(|x| x.is_finite()) {
                        return Err(Error::InvalidMesh(format!(
                            "patch {pi} color {vi} is not finite"
                        )));
                    }
                }
            }
            let vertex_count_u32 = u32::try_from(vertex_count)
                .map_err(|_| Error::InvalidMesh(format!("patch {pi} has too many vertices")))?;
            for (ti, tri) in patch.indices.iter().enumerate() {
                for &index in tri {
                    if index >= vertex_count_u32 {
                        return Err(Error::InvalidMesh(format!(
                            "patch {pi} triangle {ti} index {index} out of range (>= {vertex_count})"
                        )));
                    }
                }
            }
            if patch.material.index() >= self.materials.len() {
                return Err(Error::UnknownMaterial(patch.material));
            }
        }
        Ok(())
    }
}

/// Converts an insertion index to a `u32` scene id.
#[allow(clippy::cast_possible_truncation)]
const fn index_to_u32(index: usize) -> u32 {
    // Scene ids equal the insertion index; a scene never holds `u32::MAX` items.
    index as u32
}

/// The vertex positions of a stored mesh, whatever its kind.
fn stored_positions(stored: &StoredMesh) -> Box<dyn Iterator<Item = &Point3<f64>> + '_> {
    match stored {
        StoredMesh::Tri(mesh) => {
            Box::new(mesh.patches.iter().flat_map(|patch| patch.positions.iter()))
        }
        StoredMesh::Lines(lines) => Box::new(lines.positions.iter()),
    }
}

/// Validates a ray trace: at least two stations, all stations of equal length,
/// and every present coordinate finite.
fn validate_ray_trace(trace: &RayTrace) -> Result<(), Error> {
    if trace.stations.len() < 2 {
        return Err(Error::InvalidRayTrace(
            "fewer than two stations".to_string(),
        ));
    }
    let width = trace.stations[0].len();
    for (k, station) in trace.stations.iter().enumerate() {
        if station.len() != width {
            return Err(Error::InvalidRayTrace(format!(
                "station {k} has {} rays, expected {width}",
                station.len()
            )));
        }
        for (i, point) in station.iter().enumerate() {
            if let Some(p) = point {
                if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                    return Err(Error::InvalidRayTrace(format!(
                        "station {k} ray {i} is not finite"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// The center of the axis-aligned bounding box of all valid ray points.
fn ray_center(trace: &RayTrace) -> Point3<f64> {
    aabb_center(trace.stations.iter().flatten().flatten().copied())
}

/// The center of the axis-aligned bounding box of `points`, or the coordinate
/// origin if `points` yields nothing.
fn aabb_center(points: impl Iterator<Item = Point3<f64>>) -> Point3<f64> {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut any = false;
    for p in points {
        min[0] = min[0].min(p.x);
        min[1] = min[1].min(p.y);
        min[2] = min[2].min(p.z);
        max[0] = max[0].max(p.x);
        max[1] = max[1].max(p.y);
        max[2] = max[2].max(p.z);
        any = true;
    }
    if !any {
        return Point3::origin();
    }
    Point3::new(
        f64::midpoint(min[0], max[0]),
        f64::midpoint(min[1], max[1]),
        f64::midpoint(min[2], max[2]),
    )
}

/// The line color: the explicit style color, else the wavelength-derived color,
/// else a magenta fallback.
fn line_color(style: &RayStyle, trace: &RayTrace) -> [f32; 4] {
    if let Some(color) = style.line_color {
        return color;
    }
    if let Some(wavelength) = trace.wavelength {
        if let Some(rgb) = color::wavelength_to_rgb(wavelength) {
            return [rgb[0], rgb[1], rgb[2], 1.0];
        }
    }
    [0.8, 0.1, 0.8, 1.0]
}

/// Returns a copy of `mesh` with every position shifted by `-center`.
fn shift_trimesh(mesh: &TriMesh, center: Point3<f64>) -> TriMesh {
    TriMesh {
        patches: mesh
            .patches
            .iter()
            .map(|patch| SurfacePatch {
                name: patch.name.clone(),
                positions: patch.positions.iter().map(|p| p - center.coords).collect(),
                normals: patch.normals.clone(),
                colors: patch.colors.clone(),
                indices: patch.indices.clone(),
                material: patch.material,
            })
            .collect(),
    }
}

/// Content hash of a material (`f32` fields hashed via `to_bits`).
fn material_hash(material: &Material) -> u64 {
    let mut hasher = DefaultHasher::new();
    match material {
        Material::Glass {
            color,
            ior,
            thickness,
        } => {
            hasher.write_u8(0);
            hash_f32_slice(&mut hasher, color);
            hasher.write_u32(ior.to_bits());
            hasher.write_u32(thickness.to_bits());
        }
        Material::Mirror { color, roughness } => {
            hasher.write_u8(1);
            hash_f32_slice(&mut hasher, color);
            hasher.write_u32(roughness.to_bits());
        }
        Material::Opaque {
            color,
            metallic,
            roughness,
        } => {
            hasher.write_u8(2);
            hash_f32_slice(&mut hasher, color);
            hasher.write_u32(metallic.to_bits());
            hasher.write_u32(roughness.to_bits());
        }
        Material::Unlit { color } => {
            hasher.write_u8(3);
            hash_f32_slice(&mut hasher, color);
        }
        Material::Translucent { color } => {
            hasher.write_u8(4);
            hash_f32_slice(&mut hasher, color);
        }
    }
    hasher.finish()
}

/// Hashes a triangle mesh's geometry, letting `hash_material` append the
/// per-patch material contribution.
///
/// The material contribution differs by use: deduplication hashes the material
/// *id* (identical inputs must dedup), while node diffing hashes the material
/// *content* (so a material edit is detected even when its id is unchanged).
fn hash_tri_mesh<F>(hasher: &mut DefaultHasher, mesh: &TriMesh, mut hash_material: F)
where
    F: FnMut(&mut DefaultHasher, MaterialId),
{
    hasher.write_usize(mesh.patches.len());
    for patch in &mesh.patches {
        match &patch.name {
            Some(name) => {
                hasher.write_u8(1);
                hasher.write(name.as_bytes());
                hasher.write_u8(0xff);
            }
            None => hasher.write_u8(0),
        }
        hasher.write_usize(patch.positions.len());
        for p in &patch.positions {
            hasher.write_u64(p.x.to_bits());
            hasher.write_u64(p.y.to_bits());
            hasher.write_u64(p.z.to_bits());
        }
        match &patch.normals {
            Some(normals) => {
                hasher.write_u8(1);
                for n in normals {
                    hasher.write_u64(n.x.to_bits());
                    hasher.write_u64(n.y.to_bits());
                    hasher.write_u64(n.z.to_bits());
                }
            }
            None => hasher.write_u8(0),
        }
        match &patch.colors {
            Some(colors) => {
                hasher.write_u8(1);
                for c in colors {
                    hash_f32_slice(hasher, c);
                }
            }
            None => hasher.write_u8(0),
        }
        hasher.write_usize(patch.indices.len());
        for tri in &patch.indices {
            for &index in tri {
                hasher.write_u32(index);
            }
        }
        hash_material(hasher, patch.material);
    }
}

/// Content hash of a mesh for deduplication (materials contribute by id).
fn mesh_hash(mesh: &TriMesh) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_tri_mesh(&mut hasher, mesh, |hasher, material| {
        hasher.write_u32(material.raw());
    });
    hasher.finish()
}

/// Feeds a slice of `f32` values into a hasher via their bit patterns.
fn hash_f32_slice(hasher: &mut DefaultHasher, values: &[f32]) {
    for v in values {
        hasher.write_u32(v.to_bits());
    }
}
