//! Test geometries, available to unit tests and (with the `fixtures` feature)
//! to examples and downstream crates.
//!
//! These are deterministic, analytically-normalled meshes used to exercise the
//! export without pulling in real optical geometry.

use std::f64::consts::TAU;

use nalgebra::{Point3, Vector3};

use crate::model::{MaterialId, RayTrace, SurfacePatch, TriMesh};

/// Builds an axis-aligned cube centered at the origin.
///
/// The cube is a single patch with 24 vertices (four per face) so that its
/// edges stay sharp, each face carrying its constant outward normal.
///
/// # Arguments
/// * `size` — the edge length in meters.
/// * `material` — the material id the patch is rendered with.
///
/// # Returns
/// A [`TriMesh`] with one patch.
#[must_use]
pub fn cube(size: f64, material: MaterialId) -> TriMesh {
    let h = size / 2.0;
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(12);

    // (corner-a, corner-b, corner-c, corner-d) in CCW outward order, plus normal.
    let faces = [
        (
            [h, -h, -h],
            [h, h, -h],
            [h, h, h],
            [h, -h, h],
            [1.0, 0.0, 0.0],
        ),
        (
            [-h, -h, h],
            [-h, h, h],
            [-h, h, -h],
            [-h, -h, -h],
            [-1.0, 0.0, 0.0],
        ),
        (
            [-h, h, h],
            [h, h, h],
            [h, h, -h],
            [-h, h, -h],
            [0.0, 1.0, 0.0],
        ),
        (
            [-h, -h, -h],
            [h, -h, -h],
            [h, -h, h],
            [-h, -h, h],
            [0.0, -1.0, 0.0],
        ),
        (
            [-h, -h, h],
            [h, -h, h],
            [h, h, h],
            [-h, h, h],
            [0.0, 0.0, 1.0],
        ),
        (
            [h, -h, -h],
            [-h, -h, -h],
            [-h, h, -h],
            [h, h, -h],
            [0.0, 0.0, -1.0],
        ),
    ];

    for (a, b, c, d, n) in faces {
        let base = u32::try_from(positions.len()).unwrap_or(0);
        for corner in [a, b, c, d] {
            positions.push(Point3::new(corner[0], corner[1], corner[2]));
            normals.push(Vector3::new(n[0], n[1], n[2]));
        }
        indices.push([base, base + 1, base + 2]);
        indices.push([base, base + 2, base + 3]);
    }

    TriMesh {
        patches: vec![SurfacePatch {
            name: Some("cube".to_string()),
            positions,
            normals: Some(normals),
            colors: None,
            indices,
            material,
        }],
    }
}

/// Builds a biconvex lens whose optical axis is the z-axis, centered on the
/// origin.
///
/// The lens is three patches — front spherical cap, back spherical cap and the
/// cylindrical edge — each with analytically computed normals.
///
/// # Arguments
/// * `diameter` — the full clear aperture diameter in meters.
/// * `center_thickness` — the on-axis thickness between the two poles, in meters.
/// * `r1` — the radius of curvature of the front surface (magnitude, meters).
/// * `r2` — the radius of curvature of the back surface (magnitude, meters).
/// * `segments` — the azimuthal and radial subdivision count (clamped to `>= 3`).
/// * `material` — the material id all three patches are rendered with.
///
/// # Returns
/// A [`TriMesh`] with three patches.
#[must_use]
pub fn biconvex_lens(
    diameter: f64,
    center_thickness: f64,
    r1: f64,
    r2: f64,
    segments: u32,
    material: MaterialId,
) -> TriMesh {
    let seg = segments.max(3);
    let aperture = diameter / 2.0;
    let half_thickness = center_thickness / 2.0;
    let sag1 = sag(r1, aperture);
    let sag2 = sag(r2, aperture);

    let front = cap_patch(
        aperture,
        r1,
        -half_thickness,
        -1.0,
        seg,
        "lens_front",
        material,
    );
    let back = cap_patch(
        aperture,
        r2,
        half_thickness,
        1.0,
        seg,
        "lens_back",
        material,
    );
    let edge = cylinder_patch(
        aperture,
        -half_thickness + sag1,
        half_thickness - sag2,
        seg,
        material,
    );

    TriMesh {
        patches: vec![front, back, edge],
    }
}

/// The sag (bulge height over the aperture) of a spherical surface.
fn sag(curvature_radius: f64, aperture_radius: f64) -> f64 {
    let inside = curvature_radius
        .mul_add(curvature_radius, -(aperture_radius * aperture_radius))
        .max(0.0);
    curvature_radius - inside.sqrt()
}

/// Builds one spherical cap patch.
///
/// `bulge_dir` is `-1.0` for a surface bulging toward `-z` (front) and `+1.0`
/// for one bulging toward `+z` (back). Triangle winding is chosen so the visible
/// side faces outward.
fn cap_patch(
    aperture: f64,
    curvature_radius: f64,
    pole_z: f64,
    bulge_dir: f64,
    seg: u32,
    name: &str,
    material: MaterialId,
) -> SurfacePatch {
    let rings = seg;
    let center_z = bulge_dir.mul_add(-curvature_radius, pole_z);
    let flip = bulge_dir < 0.0;

    let mut positions = vec![Point3::new(0.0, 0.0, pole_z)];
    let mut normals = vec![Vector3::new(0.0, 0.0, bulge_dir)];

    for j in 1..=rings {
        let rho = aperture * f64::from(j) / f64::from(rings);
        let axial = curvature_radius
            .mul_add(curvature_radius, -(rho * rho))
            .max(0.0)
            .sqrt();
        let z = bulge_dir.mul_add(axial, center_z);
        for i in 0..seg {
            let theta = TAU * f64::from(i) / f64::from(seg);
            let (sin, cos) = theta.sin_cos();
            positions.push(Point3::new(rho * cos, rho * sin, z));
            normals.push(Vector3::new(
                rho * cos / curvature_radius,
                rho * sin / curvature_radius,
                bulge_dir * axial / curvature_radius,
            ));
        }
    }

    let mut indices = Vec::new();
    // Fan from the pole (index 0) to the first ring.
    for i in 0..seg {
        let a = 1 + i;
        let b = 1 + (i + 1) % seg;
        push_triangle(&mut indices, 0, a, b, flip);
    }
    // Bands between successive rings.
    for j in 1..rings {
        let inner = 1 + (j - 1) * seg;
        let outer = 1 + j * seg;
        for i in 0..seg {
            let i1 = (i + 1) % seg;
            push_triangle(&mut indices, inner + i, outer + i, outer + i1, flip);
            push_triangle(&mut indices, inner + i, outer + i1, inner + i1, flip);
        }
    }

    SurfacePatch {
        name: Some(name.to_string()),
        positions,
        normals: Some(normals),
        colors: None,
        indices,
        material,
    }
}

/// Builds the cylindrical edge patch connecting the two rims.
fn cylinder_patch(
    aperture: f64,
    z_front_rim: f64,
    z_back_rim: f64,
    seg: u32,
    material: MaterialId,
) -> SurfacePatch {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for i in 0..seg {
        let theta = TAU * f64::from(i) / f64::from(seg);
        let (sin, cos) = theta.sin_cos();
        positions.push(Point3::new(aperture * cos, aperture * sin, z_front_rim));
        positions.push(Point3::new(aperture * cos, aperture * sin, z_back_rim));
        normals.push(Vector3::new(cos, sin, 0.0));
        normals.push(Vector3::new(cos, sin, 0.0));
    }

    let mut indices = Vec::new();
    for i in 0..seg {
        let i1 = (i + 1) % seg;
        let front = 2 * i;
        let back = 2 * i + 1;
        let front_next = 2 * i1;
        let back_next = 2 * i1 + 1;
        push_triangle(&mut indices, front, front_next, back_next, false);
        push_triangle(&mut indices, front, back_next, back, false);
    }

    SurfacePatch {
        name: Some("lens_edge".to_string()),
        positions,
        normals: Some(normals),
        colors: None,
        indices,
        material,
    }
}

/// Appends a triangle, reversing its winding when `flip` is set.
fn push_triangle(indices: &mut Vec<[u32; 3]>, a: u32, b: u32, c: u32, flip: bool) {
    if flip {
        indices.push([a, c, b]);
    } else {
        indices.push([a, b, c]);
    }
}

/// Cross-section coordinates of a bundle of concentric rings.
///
/// The outermost ring has radius `r0` and `outer` points; inner rings are scaled
/// down. A single center point is included.
///
/// # Arguments
/// * `r0` — the outer ring radius in meters.
/// * `rings` — the number of rings (clamped to `>= 1`).
/// * `outer` — the number of points on the outer ring (clamped to `>= 3`).
///
/// # Returns
/// The `(x, y)` coordinates of the bundle.
#[must_use]
pub fn concentric_bundle_xy(r0: f64, rings: u32, outer: u32) -> Vec<(f64, f64)> {
    let rings = rings.max(1);
    let outer = outer.max(3);
    let mut points = vec![(0.0, 0.0)];
    for ring in 1..=rings {
        let radius = r0 * f64::from(ring) / f64::from(rings);
        let count = (outer * ring / rings).max(1);
        for i in 0..count {
            let theta = TAU * f64::from(i) / f64::from(count);
            points.push((radius * theta.cos(), radius * theta.sin()));
        }
    }
    points
}

/// The cross-section `xy` laid out as a full (all-present) station at height `z`.
fn station_at_z(xy: &[(f64, f64)], z: f64) -> Vec<Option<Point3<f64>>> {
    xy.iter()
        .map(|&(x, y)| Some(Point3::new(x, y, z)))
        .collect()
}

/// A collimated bundle: two stations at `z = 0` and `z = 0.1`, rays parallel to
/// the z-axis, outer radius `r0`.
///
/// # Arguments
/// * `r0` — the bundle radius in meters.
/// * `wavelength` — the wavelength in meters, used for coloring.
#[must_use]
pub fn collimated_bundle(r0: f64, wavelength: Option<f64>) -> RayTrace {
    let xy = concentric_bundle_xy(r0, 4, 256);
    RayTrace {
        uid: "collimated".to_string(),
        stations: vec![station_at_z(&xy, 0.0), station_at_z(&xy, 0.1)],
        wavelength,
    }
}

/// A focusing bundle: two stations at `z = 0` and `z = 0.1`, all rays passing
/// through `(0, 0, 0.03)` (so the focus lies between the stations).
///
/// # Arguments
/// * `r0` — the starting bundle radius in meters.
#[must_use]
pub fn focusing_bundle(r0: f64) -> RayTrace {
    let xy = concentric_bundle_xy(r0, 4, 256);
    let scale = 0.1 / 0.03;
    let station1 = xy
        .iter()
        .map(|&(x, y)| Some(Point3::new(x - scale * x, y - scale * y, 0.1)))
        .collect();
    RayTrace {
        uid: "focus".to_string(),
        stations: vec![station_at_z(&xy, 0.0), station1],
        wavelength: None,
    }
}

/// A folded bundle modelling a 45° mirror: three stations, giving two segments
/// (propagation in `+z` then in `+x`).
///
/// Station 1 lies on the plane `z = 0.05 + x`, and station 2 propagates in `+x`
/// to `x = 0.05`.
///
/// # Arguments
/// * `r0` — the bundle radius in meters.
#[must_use]
pub fn folded_bundle(r0: f64) -> RayTrace {
    let xy = concentric_bundle_xy(r0, 4, 256);
    let station1 = xy
        .iter()
        .map(|&(x, y)| Some(Point3::new(x, y, 0.05 + x)))
        .collect();
    let station2 = xy
        .iter()
        .map(|&(x, y)| Some(Point3::new(0.05, y, 0.05 + x)))
        .collect();
    RayTrace {
        uid: "folded".to_string(),
        stations: vec![station_at_z(&xy, 0.0), station1, station2],
        wavelength: None,
    }
}

/// A vignetted bundle: three collimated stations at `z = 0, 0.05, 0.1`, with the
/// outer half of the rays lost (`None`) at the last station.
///
/// # Arguments
/// * `r0` — the bundle radius in meters.
#[must_use]
pub fn vignetted_bundle(r0: f64) -> RayTrace {
    let xy = concentric_bundle_xy(r0, 4, 256);
    let cutoff = xy.len() / 2;
    let station2 = xy
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            if i < cutoff {
                Some(Point3::new(x, y, 0.1))
            } else {
                None
            }
        })
        .collect();
    RayTrace {
        uid: "vignetted".to_string(),
        stations: vec![station_at_z(&xy, 0.0), station_at_z(&xy, 0.05), station2],
        wavelength: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{biconvex_lens, cube};
    use crate::{Layer, Material, MeshId, Scene, SceneNode, SceneOptions};
    use nalgebra::Isometry3;

    fn place(scene: &mut Scene, uid: &str, mesh: MeshId) {
        scene
            .add_node(SceneNode {
                uid: uid.to_string(),
                name: uid.to_string(),
                mesh: Some(mesh),
                transform: Isometry3::identity(),
                layer: Layer::Optics,
                data: None,
            })
            .unwrap();
    }

    #[test]
    fn cube_and_lens_are_valid_and_exportable() {
        let mut scene = Scene::new(SceneOptions::default());
        let material = scene.add_material(Material::Opaque {
            color: [0.5, 0.5, 0.5, 1.0],
            metallic: 0.0,
            roughness: 1.0,
        });
        let cube_mesh = scene.add_mesh(cube(0.01, material)).unwrap();
        let lens = biconvex_lens(0.05, 0.01, 0.1, 0.1, 16, material);
        assert_eq!(lens.patches.len(), 3);
        let lens_mesh = scene.add_mesh(lens).unwrap();
        place(&mut scene, "cube", cube_mesh);
        place(&mut scene, "lens", lens_mesh);

        let glb = scene.to_glb().unwrap();
        let gltf = gltf::Gltf::from_slice(&glb).unwrap();
        assert_eq!(gltf.document.meshes().count(), 2);
        // The lens mesh has one primitive per patch.
        assert!(gltf
            .document
            .meshes()
            .any(|mesh| mesh.primitives().count() == 3));
    }
}
