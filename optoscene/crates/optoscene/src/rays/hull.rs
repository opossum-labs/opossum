//! The crude default round envelope (`Envelope::DefaultRound`).
//!
//! This is a deliberately rough fallback for callers that only have ray
//! positions: a rotationally-symmetric loft around the bundle's centroid path.
//! It is **not** a physically accurate beam shape; callers that need the true
//! shape pass an `Envelope::Mesh` instead.

use std::f64::consts::TAU;

use nalgebra::{Point3, Vector3};

use crate::model::{MaterialId, RayTrace, SurfacePatch, TriMesh};

/// Builds the default round envelope of a ray trace, relative to `center`.
///
/// For each segment between two stations, the valid rays (valid at both) define
/// a mean axis; `steps + 1` cross sections are sampled along the segment, each a
/// ring of `sectors` points at the maximum perpendicular radius of the rays at
/// that cross section. Consecutive rings are lofted into a tube. Segments with
/// fewer than three valid rays or a degenerate axis are skipped.
///
/// # Arguments
/// * `trace` — the ray trace.
/// * `sectors` — points per ring (clamped to `>= 3`).
/// * `steps` — cross sections per segment (clamped to `>= 1`).
/// * `center` — the reference point subtracted from every position.
/// * `material` — the (translucent) material of the envelope.
///
/// # Returns
/// `Some(TriMesh)` with a single patch, or `None` if no segment produced
/// geometry.
#[allow(clippy::cast_possible_truncation)]
pub fn default_round(
    trace: &RayTrace,
    sectors: u32,
    steps: u32,
    center: Point3<f64>,
    material: MaterialId,
) -> Option<TriMesh> {
    let sectors = sectors.max(3);
    let steps = steps.max(1);
    let mut positions: Vec<Point3<f64>> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();

    let stations = &trace.stations;
    for pair in stations.windows(2) {
        let (start, end) = (&pair[0], &pair[1]);
        let rays: Vec<(Point3<f64>, Point3<f64>)> = (0..start.len())
            .filter_map(|i| match (start[i], end.get(i).copied().flatten()) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            })
            .collect();
        if rays.len() < 3 {
            continue;
        }

        let mut axis = Vector3::zeros();
        for &(a, b) in &rays {
            let direction = b - a;
            let length = direction.norm();
            if length > 1e-12 {
                axis += direction / length;
            }
        }
        let axis_norm = axis.norm();
        if axis_norm < 1e-9 {
            continue;
        }
        let d = axis / axis_norm;
        let (u, v) = perpendicular_basis(d);

        let mut ring_starts: Vec<u32> = Vec::with_capacity((steps + 1) as usize);
        for j in 0..=steps {
            let s = f64::from(j) / f64::from(steps);
            let points: Vec<Point3<f64>> = rays.iter().map(|&(a, b)| a + (b - a) * s).collect();

            let mut centroid = Vector3::zeros();
            for point in &points {
                centroid += point.coords;
            }
            #[allow(clippy::cast_precision_loss)]
            let centroid = Point3::from(centroid / points.len() as f64);

            let mut radius = 0.0_f64;
            for point in &points {
                let rel = point - centroid;
                let perpendicular = rel - d * rel.dot(&d);
                radius = radius.max(perpendicular.norm());
            }

            ring_starts.push(positions.len() as u32);
            for sector in 0..sectors {
                let theta = TAU * f64::from(sector) / f64::from(sectors);
                let ring_point = centroid + (u * theta.cos() + v * theta.sin()) * radius;
                positions.push(ring_point - center.coords);
            }
        }

        for j in 0..steps {
            let inner = ring_starts[j as usize];
            let outer = ring_starts[(j + 1) as usize];
            for sector in 0..sectors {
                let next = (sector + 1) % sectors;
                indices.push([inner + sector, outer + sector, outer + next]);
                indices.push([inner + sector, outer + next, inner + next]);
            }
        }
    }

    if indices.is_empty() {
        return None;
    }
    Some(TriMesh {
        patches: vec![SurfacePatch {
            name: Some("envelope".to_string()),
            positions,
            normals: None,
            colors: None,
            indices,
            material,
        }],
    })
}

/// Returns two unit vectors `(u, v)` spanning the plane perpendicular to `d`.
///
/// `u = normalize(d × a)` with `a` the coordinate axis least parallel to `d`,
/// and `v = d × u`.
fn perpendicular_basis(d: Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let a = if ax <= ay && ax <= az {
        Vector3::x()
    } else if ay <= az {
        Vector3::y()
    } else {
        Vector3::z()
    };
    let u = d.cross(&a).normalize();
    let v = d.cross(&u);
    (u, v)
}
