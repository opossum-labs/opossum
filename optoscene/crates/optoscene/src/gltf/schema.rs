//! Minimal serde structs for the subset of glTF 2.0 that `optoscene` emits.
//!
//! Empty fields are skipped so the JSON stays small and only contains what is
//! actually used. All structs live in a private module, so they are not part of
//! the crate's public API.

use serde::Serialize;
use serde_json::Value;

/// The glTF document root.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub asset: Asset,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<Material>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffer_views: Vec<BufferView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
}

/// The `asset` block.
#[derive(Serialize)]
pub struct Asset {
    pub version: String,
    pub generator: String,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            version: String::from("2.0"),
            generator: format!("optoscene {}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// A glTF scene: a list of root node indices.
#[derive(Serialize)]
pub struct Scene {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub nodes: Vec<usize>,
}

/// A node in the scene graph.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

/// A mesh: a list of primitives.
#[derive(Serialize)]
pub struct Mesh {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}

/// A single primitive within a mesh.
#[derive(Serialize)]
pub struct Primitive {
    pub attributes: Attributes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

/// Vertex attribute accessor indices, in a fixed order for determinism.
#[derive(Serialize, Default)]
pub struct Attributes {
    #[serde(rename = "POSITION", skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    #[serde(rename = "NORMAL", skip_serializing_if = "Option::is_none")]
    pub normal: Option<usize>,
    #[serde(rename = "COLOR_0", skip_serializing_if = "Option::is_none")]
    pub color_0: Option<usize>,
}

/// A typed view into a buffer view.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    pub buffer_view: usize,
    pub component_type: u32,
    pub count: usize,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f32>>,
}

/// A slice of a buffer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
}

/// A binary buffer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    pub byte_length: usize,
}

/// A PBR material.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_mode: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub double_sided: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<MaterialExtensions>,
}

/// The metallic-roughness PBR parameters.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct PbrMetallicRoughness {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roughness_factor: Option<f32>,
}

/// The `KHR_materials_*` extensions carried by a material.
#[derive(Serialize, Default)]
pub struct MaterialExtensions {
    #[serde(
        rename = "KHR_materials_transmission",
        skip_serializing_if = "Option::is_none"
    )]
    pub transmission: Option<Transmission>,
    #[serde(rename = "KHR_materials_ior", skip_serializing_if = "Option::is_none")]
    pub ior: Option<Ior>,
    #[serde(
        rename = "KHR_materials_volume",
        skip_serializing_if = "Option::is_none"
    )]
    pub volume: Option<Volume>,
    #[serde(
        rename = "KHR_materials_unlit",
        skip_serializing_if = "Option::is_none"
    )]
    pub unlit: Option<Unlit>,
}

/// `KHR_materials_transmission`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transmission {
    pub transmission_factor: f32,
}

/// `KHR_materials_ior`.
#[derive(Serialize)]
pub struct Ior {
    pub ior: f32,
}

/// `KHR_materials_volume`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub thickness_factor: f32,
}

/// `KHR_materials_unlit` (an empty object).
#[derive(Serialize)]
pub struct Unlit {}

/// Extension identifiers, in the order they are listed in `extensionsUsed`.
pub const EXT_TRANSMISSION: &str = "KHR_materials_transmission";
/// The `KHR_materials_ior` identifier.
pub const EXT_IOR: &str = "KHR_materials_ior";
/// The `KHR_materials_volume` identifier.
pub const EXT_VOLUME: &str = "KHR_materials_volume";
/// The `KHR_materials_unlit` identifier.
pub const EXT_UNLIT: &str = "KHR_materials_unlit";

/// Returns `true` if the boolean is `false` (for `skip_serializing_if`).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

/// glTF component type: 32-bit float.
pub const COMPONENT_FLOAT: u32 = 5126;
/// glTF component type: unsigned 16-bit integer.
pub const COMPONENT_UNSIGNED_SHORT: u32 = 5123;
/// glTF component type: unsigned 32-bit integer.
pub const COMPONENT_UNSIGNED_INT: u32 = 5125;
/// glTF buffer view target: vertex attributes.
pub const TARGET_ARRAY_BUFFER: u32 = 34962;
/// glTF buffer view target: indices.
pub const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;
/// glTF primitive mode: line list.
pub const MODE_LINES: u32 = 1;
