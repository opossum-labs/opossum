//! Public input types for building a [`crate::Scene`].
//!
//! All lengths are in **meters** and all coordinates are `f64`. The conversion
//! to `f32` happens only in the exporter.

use nalgebra::{Isometry3, Point3, Vector3};
use serde_json::Value;

/// Identifier of a material registered in a [`crate::Scene`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(u32);

impl MaterialId {
    /// Creates an id from its raw index (crate-internal).
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The raw index value.
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }

    /// The index into the scene's material list.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Identifier of a mesh registered in a [`crate::Scene`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(u32);

impl MeshId {
    /// Creates an id from its raw index (crate-internal).
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The raw index value.
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }

    /// The index into the scene's mesh list.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Logical layer a node belongs to. Layers can be exported and replaced
/// individually.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    /// Optical components (lenses, mirrors, ...).
    Optics,
    /// Ray lines and bundle envelopes.
    Rays,
    /// Auxiliary helpers.
    Aux,
}

impl Layer {
    /// The layer name used in glTF extras and in the streaming protocol
    /// (`"optics"`, `"rays"`, `"aux"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Optics => "optics",
            Self::Rays => "rays",
            Self::Aux => "aux",
        }
    }
}

/// A triangulated surface patch with its own vertices and material.
///
/// Patches do not share vertices, so edges between patches stay sharp.
pub struct SurfacePatch {
    /// Optional patch name.
    pub name: Option<String>,
    /// Vertex positions in the local frame of the component, in meters.
    pub positions: Vec<Point3<f64>>,
    /// Optional vertex normals, in the same frame as `positions`.
    ///
    /// Not generated: if `None`, no `NORMAL` attribute is written and viewers
    /// fall back to flat shading.
    pub normals: Option<Vec<Vector3<f64>>>,
    /// Optional linear RGBA vertex colors.
    pub colors: Option<Vec<[f32; 4]>>,
    /// Triangle indices into `positions`.
    pub indices: Vec<[u32; 3]>,
    /// The material this patch is rendered with.
    pub material: MaterialId,
}

/// A mesh consisting of one or more surface patches.
pub struct TriMesh {
    /// The surface patches making up this mesh.
    pub patches: Vec<SurfacePatch>,
}

/// Material presets mapped to glTF PBR materials and extensions.
pub enum Material {
    /// Refractive material; `thickness` is the typical thickness in meters.
    Glass {
        /// Linear RGB base color.
        color: [f32; 3],
        /// Index of refraction.
        ior: f32,
        /// Typical thickness in meters.
        thickness: f32,
    },
    /// Reflective material.
    Mirror {
        /// Linear RGB base color.
        color: [f32; 3],
        /// Surface roughness in `[0, 1]`.
        roughness: f32,
    },
    /// Opaque PBR material.
    Opaque {
        /// Linear RGBA base color.
        color: [f32; 4],
        /// Metallic factor in `[0, 1]`.
        metallic: f32,
        /// Surface roughness in `[0, 1]`.
        roughness: f32,
    },
    /// Lighting-independent color, used for rays and helpers.
    Unlit {
        /// Linear RGBA color.
        color: [f32; 4],
    },
    /// Semi-transparent, double-sided surface, used for bundle envelopes.
    Translucent {
        /// Linear RGBA color.
        color: [f32; 4],
    },
}

/// A placed instance of a mesh.
pub struct SceneNode {
    /// Stable identifier, unique within the scene (e.g. an Opossum node UUID).
    pub uid: String,
    /// Human-readable node name.
    pub name: String,
    /// The mesh this node instances, if any.
    pub mesh: Option<MeshId>,
    /// Transformation from the local mesh frame to world coordinates.
    pub transform: Isometry3<f64>,
    /// The layer this node belongs to.
    pub layer: Layer,
    /// Arbitrary metadata, exported to glTF extras.
    pub data: Option<Value>,
}

/// Ray positions at consecutive surfaces ("stations").
///
/// `stations[k][i]` is ray `i` at station `k`; `None` marks a lost ray. Rays
/// travel in straight lines between stations.
pub struct RayTrace {
    /// Stable identifier, unique within the scene.
    pub uid: String,
    /// Ray positions per station.
    pub stations: Vec<Vec<Option<Point3<f64>>>>,
    /// Wavelength in meters, used for coloring.
    pub wavelength: Option<f64>,
}

/// Visual representation of a ray trace.
pub struct RayStyle {
    /// Maximum number of rays drawn as lines. `0` disables lines. Default 200.
    pub max_lines: usize,
    /// Line color; derived from the wavelength if `None`.
    pub line_color: Option<[f32; 4]>,
    /// Envelope surface of the bundle. Default [`Envelope::None`].
    pub envelope: Envelope,
    /// Color used for [`Envelope::DefaultRound`]. Ignored for [`Envelope::Mesh`],
    /// which carries its own materials. Default `[0.2, 0.6, 1.0, 0.25]`.
    pub envelope_color: [f32; 4],
}

impl Default for RayStyle {
    fn default() -> Self {
        Self {
            max_lines: 200,
            line_color: None,
            envelope: Envelope::None,
            envelope_color: [0.2, 0.6, 1.0, 0.25],
        }
    }
}

/// Envelope surface of a ray bundle.
#[derive(Default)]
pub enum Envelope {
    /// No envelope surface.
    #[default]
    None,
    /// Envelope provided by the caller, in the same frame as the ray positions.
    ///
    /// This is the accurate option: only the caller knows apertures, clipping
    /// and the true bundle shape.
    Mesh(TriMesh),
    /// Crude round envelope derived from the ray lines, for callers that only
    /// have ray positions. Not a physically accurate beam shape.
    DefaultRound {
        /// Number of points per cross-section ring.
        sectors: u32,
        /// Number of cross sections per segment between two stations.
        steps: u32,
    },
}

impl Envelope {
    /// Returns [`Envelope::DefaultRound`] with the documented default
    /// subdivision (`sectors = 32`, `steps = 16`).
    #[must_use]
    pub const fn default_round() -> Self {
        Self::DefaultRound {
            sectors: 32,
            steps: 16,
        }
    }
}
