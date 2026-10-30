//! A stable hash of what a component looks like.

use opossum_core::geometry::SurfaceMesh;
use optoscene::Material;

/// A running 64 bit FNV-1a hash.
pub struct Fnv1a(u64);

impl Fnv1a {
    /// Start a hash.
    pub const fn new() -> Self {
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
    pub fn write_mesh(&mut self, mesh: &SurfaceMesh) {
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
    pub fn write_material(&mut self, material: &Material) {
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
    pub fn finish(&self) -> String {
        format!("{:016x}", self.0)
    }
}
