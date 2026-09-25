//! Turns a [`Scene`] into the glTF [`schema`] structs plus the binary buffer.
//!
//! The scene graph is `optoscene_root` → one group per non-empty [`Layer`] →
//! the placed content nodes. All world positions are shifted by the scene origin
//! (see [`Scene::effective_origin`]) before the `f64` → `f32` cast, so exported
//! coordinates stay small. Only meshes and materials referenced by the exported
//! nodes are emitted, and their ids are remapped accordingly.

use nalgebra::{Point3, UnitQuaternion, Vector3};

use super::schema;
use crate::error::Error;
use crate::model::{Layer, Material, SceneNode, SurfacePatch};
use crate::scene::{LineMesh, Scene, StoredMesh};
use crate::util::to_f32;

/// Builds the glTF document and binary buffer for the whole scene.
///
/// # Arguments
/// * `scene` — the scene to export.
///
/// # Returns
/// The glTF [`schema::Root`] and the single binary buffer.
///
/// # Errors
/// Returns [`Error::InvalidMesh`] if a normal has near-zero length.
pub fn build(scene: &Scene) -> Result<(schema::Root, Vec<u8>), Error> {
    let all: Vec<&SceneNode> = scene.nodes().iter().collect();
    build_selected(scene, &all)
}

/// Builds a glTF document containing only `selected` nodes (plus the meshes and
/// materials they reference), keeping the root/layer structure and the
/// scene-global origin.
///
/// # Arguments
/// * `scene` — the scene the nodes belong to (used for origin and lookups).
/// * `selected` — the nodes to export, in the desired order.
///
/// # Returns
/// The glTF [`schema::Root`] and the single binary buffer.
///
/// # Errors
/// Returns [`Error::InvalidMesh`] if a normal has near-zero length.
pub fn build_selected(
    scene: &Scene,
    selected: &[&SceneNode],
) -> Result<(schema::Root, Vec<u8>), Error> {
    let origin = scene.effective_origin();
    let mut root = schema::Root::default();
    let mut bin: Vec<u8> = Vec::new();
    let mut used_ext = UsedExtensions::default();

    // Meshes referenced by the selected nodes.
    let mut mesh_used = vec![false; scene.meshes().len()];
    for node in selected {
        if let Some(mesh_id) = node.mesh {
            if let Some(flag) = mesh_used.get_mut(mesh_id.index()) {
                *flag = true;
            }
        }
    }
    // Materials referenced by the used meshes.
    let mut material_used = vec![false; scene.materials().len()];
    for (index, used) in mesh_used.iter().enumerate() {
        if !*used {
            continue;
        }
        match &scene.meshes()[index] {
            StoredMesh::Tri(mesh) => {
                for patch in &mesh.patches {
                    if let Some(flag) = material_used.get_mut(patch.material.index()) {
                        *flag = true;
                    }
                }
            }
            StoredMesh::Lines(lines) => {
                if let Some(flag) = material_used.get_mut(lines.material.index()) {
                    *flag = true;
                }
            }
        }
    }
    // Emit referenced materials in insertion order; remember the id remapping and, per material, the
    // scale a textured one projects its patches' texture coordinates with (see `emit_materials`).
    let (material_map, uv_scales) =
        emit_materials(scene, &material_used, &mut used_ext, &mut root, &mut bin);
    // Emit referenced meshes in insertion order; remember the id remapping.
    let mut mesh_map = vec![None; scene.meshes().len()];
    for (index, used) in mesh_used.iter().enumerate() {
        if *used {
            let gltf_mesh = build_mesh(
                &scene.meshes()[index],
                &material_map,
                &uv_scales,
                &mut root,
                &mut bin,
            )?;
            let gltf_index = root.meshes.len();
            root.meshes.push(gltf_mesh);
            mesh_map[index] = Some(gltf_index);
        }
    }

    // Content nodes grouped into per-layer groups (canonical layer order).
    let mut root_children = Vec::new();
    for layer in [Layer::Optics, Layer::Rays, Layer::Aux] {
        let mut children = Vec::new();
        for node in selected.iter().filter(|node| node.layer == layer) {
            let gltf_index = root.nodes.len();
            root.nodes
                .push(build_content_node(node, &origin, &mesh_map));
            children.push(gltf_index);
        }
        if children.is_empty() {
            continue;
        }
        let group_index = root.nodes.len();
        root.nodes.push(schema::Node {
            name: Some(format!("layer:{}", layer.name())),
            children,
            extras: Some(serde_json::json!({ "layer": layer.name() })),
            ..schema::Node::default()
        });
        root_children.push(group_index);
    }

    // Root node: carries the root rotation and the origin metadata.
    let root_index = root.nodes.len();
    root.nodes.push(schema::Node {
        name: Some("optoscene_root".to_string()),
        rotation: Some(quat_to_array(&scene.options().root_rotation)),
        children: root_children,
        extras: Some(serde_json::json!({ "origin": [origin.x, origin.y, origin.z] })),
        ..schema::Node::default()
    });

    root.scenes.push(schema::Scene {
        name: Some(scene.options().name.clone()),
        nodes: vec![root_index],
    });
    root.scene = Some(0);
    root.extensions_used = used_ext.list();

    if !bin.is_empty() {
        root.buffers.push(schema::Buffer {
            uri: None,
            byte_length: bin.len(),
        });
    }

    Ok((root, bin))
}

/// Emits the referenced materials in insertion order.
///
/// Returns the scene-index → glTF-index remapping and, per scene material, the texture-coordinate
/// projection scale of a [`Material::Textured`] (see [`map_material`]), which the meshes need to
/// generate a `TEXCOORD_0` attribute for their textured patches.
fn emit_materials(
    scene: &Scene,
    material_used: &[bool],
    used_ext: &mut UsedExtensions,
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> (Vec<Option<usize>>, Vec<Option<f32>>) {
    let mut material_map = vec![None; scene.materials().len()];
    let mut uv_scales: Vec<Option<f32>> = vec![None; scene.materials().len()];
    for (index, used) in material_used.iter().enumerate() {
        if *used {
            let gltf_index = root.materials.len();
            let (material, uv_scale) = map_material(&scene.materials()[index], used_ext, root, bin);
            root.materials.push(material);
            uv_scales[index] = uv_scale;
            material_map[index] = Some(gltf_index);
        }
    }
    (material_map, uv_scales)
}

/// Builds a glTF mesh from a stored mesh (triangles or lines).
fn build_mesh(
    stored: &StoredMesh,
    material_map: &[Option<usize>],
    uv_scales: &[Option<f32>],
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> Result<schema::Mesh, Error> {
    let primitives = match stored {
        StoredMesh::Tri(mesh) => {
            let mut primitives = Vec::with_capacity(mesh.patches.len());
            for patch in &mesh.patches {
                primitives.push(build_primitive(patch, material_map, uv_scales, root, bin)?);
            }
            primitives
        }
        StoredMesh::Lines(lines) => vec![build_line_primitive(lines, material_map, root, bin)],
    };
    Ok(schema::Mesh {
        name: None,
        primitives,
    })
}

/// Builds a single triangle primitive from a patch, appending its buffer data.
fn build_primitive(
    patch: &SurfacePatch,
    material_map: &[Option<usize>],
    uv_scales: &[Option<f32>],
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> Result<schema::Primitive, Error> {
    let position = write_positions(&patch.positions, root, bin);
    let normal = match &patch.normals {
        Some(normals) => Some(write_normals(normals, root, bin)?),
        None => None,
    };
    let color_0 = patch
        .colors
        .as_ref()
        .map(|colors| write_colors(colors, root, bin));
    // A textured material carries no per-vertex UVs; its coordinates are projected from each
    // vertex's local x/z (see `Material::Textured`).
    let texcoord_0 = uv_scales
        .get(patch.material.index())
        .copied()
        .flatten()
        .map(|uv_scale| write_projected_uvs(&patch.positions, uv_scale, root, bin));
    let flat: Vec<u32> = patch.indices.iter().flatten().copied().collect();
    let indices = write_indices(&flat, patch.positions.len(), root, bin);

    Ok(schema::Primitive {
        attributes: schema::Attributes {
            position: Some(position),
            normal,
            color_0,
            texcoord_0,
        },
        indices: Some(indices),
        material: material_map.get(patch.material.index()).copied().flatten(),
        mode: None,
    })
}

/// Builds a `LINES`-mode primitive from a line mesh, appending its buffer data.
fn build_line_primitive(
    lines: &LineMesh,
    material_map: &[Option<usize>],
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> schema::Primitive {
    let position = write_positions(&lines.positions, root, bin);
    let flat: Vec<u32> = lines.segments.iter().flatten().copied().collect();
    let indices = write_indices(&flat, lines.positions.len(), root, bin);

    schema::Primitive {
        attributes: schema::Attributes {
            position: Some(position),
            normal: None,
            color_0: None,
            texcoord_0: None,
        },
        indices: Some(indices),
        material: material_map.get(lines.material.index()).copied().flatten(),
        mode: Some(schema::MODE_LINES),
    }
}

/// Writes vertex positions and returns the accessor index (with `min`/`max`).
fn write_positions(positions: &[Point3<f64>], root: &mut schema::Root, bin: &mut Vec<u8>) -> usize {
    let offset = align_to_four(bin);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        let v = [to_f32(p.x), to_f32(p.y), to_f32(p.z)];
        for k in 0..3 {
            bin.extend_from_slice(&v[k].to_le_bytes());
            min[k] = min[k].min(v[k]);
            max[k] = max[k].max(v[k]);
        }
    }
    let view = push_buffer_view(
        root,
        offset,
        positions.len() * 12,
        Some(schema::TARGET_ARRAY_BUFFER),
    );
    push_accessor(
        root,
        view,
        schema::COMPONENT_FLOAT,
        positions.len(),
        "VEC3",
        Some((min.to_vec(), max.to_vec())),
    )
}

/// Normalizes and writes vertex normals, returning the accessor index.
fn write_normals(
    normals: &[Vector3<f64>],
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> Result<usize, Error> {
    let offset = align_to_four(bin);
    for n in normals {
        let length = n.x.mul_add(n.x, n.y.mul_add(n.y, n.z * n.z)).sqrt();
        if length < 1e-9 {
            return Err(Error::InvalidMesh(
                "normal has near-zero length".to_string(),
            ));
        }
        for component in [n.x, n.y, n.z] {
            bin.extend_from_slice(&to_f32(component / length).to_le_bytes());
        }
    }
    let view = push_buffer_view(
        root,
        offset,
        normals.len() * 12,
        Some(schema::TARGET_ARRAY_BUFFER),
    );
    Ok(push_accessor(
        root,
        view,
        schema::COMPONENT_FLOAT,
        normals.len(),
        "VEC3",
        None,
    ))
}

/// Writes RGBA vertex colors and returns the accessor index.
fn write_colors(colors: &[[f32; 4]], root: &mut schema::Root, bin: &mut Vec<u8>) -> usize {
    let offset = align_to_four(bin);
    for color in colors {
        for &channel in color {
            bin.extend_from_slice(&channel.to_le_bytes());
        }
    }
    let view = push_buffer_view(
        root,
        offset,
        colors.len() * 16,
        Some(schema::TARGET_ARRAY_BUFFER),
    );
    push_accessor(
        root,
        view,
        schema::COMPONENT_FLOAT,
        colors.len(),
        "VEC4",
        None,
    )
}

/// Writes planar texture coordinates projected from each vertex's local `x`/`z`
/// (`u = x * uv_scale`, `v = z * uv_scale`) and returns the accessor index.
fn write_projected_uvs(
    positions: &[Point3<f64>],
    uv_scale: f32,
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> usize {
    let offset = align_to_four(bin);
    for p in positions {
        bin.extend_from_slice(&(to_f32(p.x) * uv_scale).to_le_bytes());
        bin.extend_from_slice(&(to_f32(p.z) * uv_scale).to_le_bytes());
    }
    let view = push_buffer_view(
        root,
        offset,
        positions.len() * 8,
        Some(schema::TARGET_ARRAY_BUFFER),
    );
    push_accessor(
        root,
        view,
        schema::COMPONENT_FLOAT,
        positions.len(),
        "VEC2",
        None,
    )
}

/// Embeds a PNG image and a repeating sampler, returning the index of the
/// [`schema::Texture`] that binds them.
fn embed_texture(image_png: &[u8], root: &mut schema::Root, bin: &mut Vec<u8>) -> usize {
    let offset = align_to_four(bin);
    bin.extend_from_slice(image_png);
    // An image buffer view carries no `target`: it is neither vertex nor index data.
    let view = push_buffer_view(root, offset, image_png.len(), None);
    let image = root.images.len();
    root.images.push(schema::Image {
        buffer_view: view,
        mime_type: "image/png".to_string(),
    });
    let sampler = root.samplers.len();
    root.samplers.push(schema::Sampler {
        wrap_s: schema::WRAP_REPEAT,
        wrap_t: schema::WRAP_REPEAT,
        mag_filter: Some(schema::FILTER_LINEAR),
        min_filter: Some(schema::FILTER_LINEAR_MIPMAP_LINEAR),
    });
    let texture = root.textures.len();
    root.textures.push(schema::Texture {
        source: image,
        sampler,
    });
    texture
}

/// Writes a flat index buffer (u16 if the mesh fits, else u32) and returns the
/// accessor index.
#[allow(clippy::cast_possible_truncation)]
fn write_indices(
    indices: &[u32],
    vertex_count: usize,
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> usize {
    let offset = align_to_four(bin);
    let count = indices.len();
    let (component_type, byte_length) = if vertex_count <= 65_535 {
        for &i in indices {
            // Safe: with <= 65_535 vertices every index is <= 65_534.
            bin.extend_from_slice(&(i as u16).to_le_bytes());
        }
        (schema::COMPONENT_UNSIGNED_SHORT, count * 2)
    } else {
        for &i in indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        (schema::COMPONENT_UNSIGNED_INT, count * 4)
    };
    let view = push_buffer_view(
        root,
        offset,
        byte_length,
        Some(schema::TARGET_ELEMENT_ARRAY_BUFFER),
    );
    push_accessor(root, view, component_type, count, "SCALAR", None)
}

/// Builds a placed content node (origin-shifted translation, extras).
fn build_content_node(
    node: &SceneNode,
    origin: &Point3<f64>,
    mesh_map: &[Option<usize>],
) -> schema::Node {
    let t = node.transform.translation.vector;
    schema::Node {
        name: Some(node.name.clone()),
        mesh: node
            .mesh
            .and_then(|mesh| mesh_map.get(mesh.index()).copied().flatten()),
        translation: Some([
            to_f32(t.x - origin.x),
            to_f32(t.y - origin.y),
            to_f32(t.z - origin.z),
        ]),
        rotation: Some(quat_to_array(&node.transform.rotation)),
        children: Vec::new(),
        extras: Some(node_extras(node)),
    }
}

/// Builds the `extras` object for a content node (`uid`, `layer`, optional `data`).
fn node_extras(node: &SceneNode) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "uid".to_string(),
        serde_json::Value::String(node.uid.clone()),
    );
    map.insert(
        "layer".to_string(),
        serde_json::Value::String(node.layer.name().to_string()),
    );
    if let Some(data) = &node.data {
        map.insert("data".to_string(), data.clone());
    }
    serde_json::Value::Object(map)
}

/// Maps a preset material to a glTF material, recording used extensions.
///
/// Also returns the texture-coordinate projection scale for a [`Material::Textured`] (see there)
/// so its meshes can be given a generated `TEXCOORD_0`; every other preset returns `None`.
// A per-variant dispatch: every arm builds a `schema::Material` inline, so the length is the
// number of material presets rather than a sign the function does too much.
#[allow(clippy::too_many_lines)]
fn map_material(
    material: &Material,
    used: &mut UsedExtensions,
    root: &mut schema::Root,
    bin: &mut Vec<u8>,
) -> (schema::Material, Option<f32>) {
    match material {
        Material::Glass {
            color,
            ior,
            thickness,
        } => {
            used.transmission = true;
            used.ior = true;
            used.volume = true;
            let material = schema::Material {
                name: Some("glass".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(rgb_opaque(*color)),
                    metallic_factor: Some(0.0),
                    roughness_factor: Some(0.05),
                    ..schema::PbrMetallicRoughness::default()
                }),
                extensions: Some(schema::MaterialExtensions {
                    transmission: Some(schema::Transmission {
                        transmission_factor: 1.0,
                    }),
                    ior: Some(schema::Ior { ior: *ior }),
                    volume: Some(schema::Volume {
                        thickness_factor: *thickness,
                    }),
                    unlit: None,
                    iridescence: None,
                }),
                ..schema::Material::default()
            };
            (material, None)
        }
        Material::Mirror { color, roughness } => {
            let material = schema::Material {
                name: Some("mirror".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(rgb_opaque(*color)),
                    metallic_factor: Some(1.0),
                    roughness_factor: Some(*roughness),
                    ..schema::PbrMetallicRoughness::default()
                }),
                ..schema::Material::default()
            };
            (material, None)
        }
        Material::Opaque {
            color,
            metallic,
            roughness,
        } => {
            let material = schema::Material {
                name: Some("opaque".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(*color),
                    metallic_factor: Some(*metallic),
                    roughness_factor: Some(*roughness),
                    ..schema::PbrMetallicRoughness::default()
                }),
                ..schema::Material::default()
            };
            (material, None)
        }
        Material::Unlit { color } => {
            used.unlit = true;
            let material = schema::Material {
                name: Some("unlit".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(*color),
                    ..schema::PbrMetallicRoughness::default()
                }),
                alpha_mode: alpha_blend_if_transparent(color[3]),
                extensions: Some(schema::MaterialExtensions {
                    unlit: Some(schema::Unlit {}),
                    ..schema::MaterialExtensions::default()
                }),
                ..schema::Material::default()
            };
            (material, None)
        }
        Material::Translucent { color } => {
            let material = schema::Material {
                name: Some("translucent".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(*color),
                    metallic_factor: Some(0.0),
                    roughness_factor: Some(0.5),
                    ..schema::PbrMetallicRoughness::default()
                }),
                alpha_mode: Some("BLEND".to_string()),
                double_sided: true,
                extensions: None,
            };
            (material, None)
        }
        Material::Iridescent {
            color,
            roughness,
            iridescence,
            iridescence_ior,
            iridescence_thickness_min,
            iridescence_thickness_max,
        } => {
            used.iridescence = true;
            let material = schema::Material {
                name: Some("iridescent".to_string()),
                // A reflective (metallic) base so the thin-film colour shift reads as a sheen on
                // a mirror-like surface rather than as a tint on a matte one.
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(rgb_opaque(*color)),
                    metallic_factor: Some(1.0),
                    roughness_factor: Some(*roughness),
                    ..schema::PbrMetallicRoughness::default()
                }),
                extensions: Some(schema::MaterialExtensions {
                    iridescence: Some(schema::Iridescence {
                        iridescence_factor: *iridescence,
                        iridescence_ior: *iridescence_ior,
                        iridescence_thickness_minimum: *iridescence_thickness_min,
                        iridescence_thickness_maximum: *iridescence_thickness_max,
                    }),
                    ..schema::MaterialExtensions::default()
                }),
                ..schema::Material::default()
            };
            (material, None)
        }
        Material::Textured {
            image_png,
            tint,
            metallic,
            roughness,
            uv_scale,
        } => {
            let texture = embed_texture(image_png, root, bin);
            let material = schema::Material {
                name: Some("textured".to_string()),
                pbr_metallic_roughness: Some(schema::PbrMetallicRoughness {
                    base_color_factor: Some(*tint),
                    base_color_texture: Some(schema::TextureInfo {
                        index: texture,
                        tex_coord: Some(0),
                    }),
                    metallic_factor: Some(*metallic),
                    roughness_factor: Some(*roughness),
                }),
                ..schema::Material::default()
            };
            (material, Some(*uv_scale))
        }
    }
}

/// Returns `Some("BLEND")` when the alpha channel is below one.
fn alpha_blend_if_transparent(alpha: f32) -> Option<String> {
    if alpha < 1.0 {
        Some("BLEND".to_string())
    } else {
        None
    }
}

/// Extends an RGB color with an opaque alpha channel.
const fn rgb_opaque(color: [f32; 3]) -> [f32; 4] {
    [color[0], color[1], color[2], 1.0]
}

/// Converts a unit quaternion to a glTF `[x, y, z, w]` array in `f32`.
fn quat_to_array(quaternion: &UnitQuaternion<f64>) -> [f32; 4] {
    let q = quaternion.coords;
    [to_f32(q.x), to_f32(q.y), to_f32(q.z), to_f32(q.w)]
}

/// Pads `bin` to a 4-byte boundary and returns the resulting offset.
fn align_to_four(bin: &mut Vec<u8>) -> usize {
    let padding = (4 - bin.len() % 4) % 4;
    bin.resize(bin.len() + padding, 0);
    bin.len()
}

/// Pushes a buffer view over `buffers[0]` and returns its index.
fn push_buffer_view(
    root: &mut schema::Root,
    offset: usize,
    length: usize,
    target: Option<u32>,
) -> usize {
    root.buffer_views.push(schema::BufferView {
        buffer: 0,
        byte_offset: offset,
        byte_length: length,
        target,
    });
    root.buffer_views.len() - 1
}

/// Pushes an accessor and returns its index.
fn push_accessor(
    root: &mut schema::Root,
    buffer_view: usize,
    component_type: u32,
    count: usize,
    kind: &str,
    min_max: Option<(Vec<f32>, Vec<f32>)>,
) -> usize {
    let (min, max) = match min_max {
        Some((min, max)) => (Some(min), Some(max)),
        None => (None, None),
    };
    root.accessors.push(schema::Accessor {
        buffer_view,
        component_type,
        count,
        kind: kind.to_string(),
        min,
        max,
    });
    root.accessors.len() - 1
}

/// Tracks which `KHR_materials_*` extensions have been emitted, so
/// `extensionsUsed` lists exactly those.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct UsedExtensions {
    transmission: bool,
    ior: bool,
    volume: bool,
    unlit: bool,
    iridescence: bool,
}

impl UsedExtensions {
    /// The used extension identifiers, in a fixed order.
    fn list(&self) -> Vec<String> {
        let mut list = Vec::new();
        if self.transmission {
            list.push(schema::EXT_TRANSMISSION.to_string());
        }
        if self.ior {
            list.push(schema::EXT_IOR.to_string());
        }
        if self.volume {
            list.push(schema::EXT_VOLUME.to_string());
        }
        if self.unlit {
            list.push(schema::EXT_UNLIT.to_string());
        }
        if self.iridescence {
            list.push(schema::EXT_IRIDESCENCE.to_string());
        }
        list
    }
}
