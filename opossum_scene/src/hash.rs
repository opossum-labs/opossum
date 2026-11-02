//! A stable hash of what a component looks like.

use optoscene::Material;

use crate::mesh::SurfaceMesh;

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
        // The zeros pad some materials as they always did, which keeps the hashes viewers hold.
        match material {
            Material::Glass {
                color,
                ior,
                thickness,
                metallic,
                opacity,
            } => self.write_tagged(0, &[], color, &[*ior, *thickness, *metallic, *opacity]),
            Material::Mirror {
                color,
                roughness,
                metallic,
                opacity,
            } => self.write_tagged(1, &[], color, &[*roughness, *metallic, *opacity, 0.0]),
            Material::Opaque {
                color,
                metallic,
                roughness,
            } => self.write_tagged(2, &[], color, &[*metallic, *roughness, 0.0, 0.0]),
            Material::Unlit { color } => self.write_tagged(3, &[], color, &[0.0; 4]),
            Material::Translucent { color } => self.write_tagged(4, &[], color, &[0.0; 4]),
            Material::Iridescent {
                color,
                roughness,
                iridescence,
                iridescence_ior,
                iridescence_thickness_min,
                iridescence_thickness_max,
            } => self.write_tagged(
                5,
                &[],
                color,
                &[
                    *roughness,
                    *iridescence,
                    *iridescence_ior,
                    *iridescence_thickness_min,
                    *iridescence_thickness_max,
                ],
            ),
            Material::Textured {
                image_png,
                tint,
                metallic,
                roughness,
                uv_scale,
            } => self.write_tagged(6, image_png, tint, &[*metallic, *roughness, *uv_scale]),
        }
    }

    /// Fold one material into the hash: its tag, the bytes it carries, its color and its numbers.
    fn write_tagged(&mut self, tag: u8, bytes: &[u8], color: &[f32], numbers: &[f32]) {
        self.write(&[tag]);
        self.write(bytes);
        for value in color.iter().chain(numbers) {
            self.write(&value.to_le_bytes());
        }
    }

    /// The hash so far, as hexadecimal.
    pub fn finish(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// Return a short string that changes whenever `bytes` do.
///
/// A viewer names it in the address of a file it fetches, so that an unchanged file keeps its
/// address and is not fetched again.
///
/// # Arguments
///
/// - `bytes`: what to digest, a drawn file say
///
/// # Returns
///
/// The 64 bit FNV-1a hash of `bytes`, as 16 hexadecimal digits.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    let mut hash = Fnv1a::new();
    hash.write(bytes);
    hash.finish()
}

#[cfg(test)]
mod test {
    use super::*;

    /// Every kind of material keeps the hash it always had: viewers compare it, and fetch a
    /// component again whenever it changes.
    #[test]
    fn every_material_keeps_its_hash() {
        let materials = [
            (
                Material::Glass {
                    color: [0.1, 0.2, 0.3],
                    ior: 1.5,
                    thickness: 0.004,
                    metallic: 0.04,
                    opacity: 0.7,
                },
                "2fc12a3a86509701",
            ),
            (
                Material::Mirror {
                    color: [0.0, 0.5, 0.5],
                    roughness: 0.05,
                    metallic: 1.0,
                    opacity: 0.6,
                },
                "68252b0aa1f10ee6",
            ),
            (
                Material::Opaque {
                    color: [0.25, 0.25, 0.28, 1.0],
                    metallic: 0.0,
                    roughness: 0.7,
                },
                "b15db7f24f341aac",
            ),
            (
                Material::Unlit {
                    color: [0.7, 0.07, 1.0, 1.0],
                },
                "b848cc81603aa91f",
            ),
            (
                Material::Translucent {
                    color: [0.85, 0.9, 0.85, 0.4],
                },
                "4983b7e15fbc85b7",
            ),
            (
                Material::Iridescent {
                    color: [0.85, 0.85, 0.9],
                    roughness: 0.15,
                    iridescence: 1.0,
                    iridescence_ior: 1.3,
                    iridescence_thickness_min: 100.0,
                    iridescence_thickness_max: 400.0,
                },
                "2bab1bcff687f3c8",
            ),
            (
                Material::Textured {
                    image_png: vec![1, 2, 3],
                    tint: [1.0, 0.9, 0.8, 1.0],
                    metallic: 0.1,
                    roughness: 0.9,
                    uv_scale: 2.0,
                },
                "c136882f8828cc8d",
            ),
        ];
        let digests: Vec<String> = materials
            .iter()
            .map(|(material, _)| {
                let mut hash = Fnv1a::new();
                hash.write_material(material);
                hash.finish()
            })
            .collect();
        let expected: Vec<&str> = materials.iter().map(|(_, digest)| *digest).collect();
        assert_eq!(digests, expected);
    }

    /// A digest is the FNV-1a hash of the bytes: nothing digested is the hash's starting value, and
    /// one changed byte is another digest.
    #[test]
    fn a_digest_is_the_hash_of_the_bytes() {
        assert_eq!(digest(b""), "cbf29ce484222325");
        assert_eq!(digest(b"glTF"), digest(b"glTF"));
        assert_ne!(digest(b"glTF"), digest(b"glTG"));
    }
}
