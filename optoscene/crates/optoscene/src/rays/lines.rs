//! Decimated ray lines.

use nalgebra::Point3;

use crate::model::RayTrace;

/// Decimated line geometry: the vertex positions and the index pairs of the
/// segments connecting them.
pub type LineGeometry = (Vec<Point3<f64>>, Vec<[u32; 2]>);

/// Builds the decimated line geometry for a ray trace, relative to `center`.
///
/// The rays valid at station 0 are decimated to at most `max_lines` in even
/// index steps (`i * n_valid / max_lines`). For each such ray, a line segment is
/// emitted between every consecutive pair of stations where it is valid at both.
///
/// # Arguments
/// * `trace` — the ray trace.
/// * `max_lines` — the maximum number of rays drawn (must be `> 0`).
/// * `center` — the reference point subtracted from every position.
///
/// # Returns
/// `Some((positions, segments))` with positions relative to `center`, or `None`
/// if there is nothing to draw.
#[allow(clippy::cast_possible_truncation)]
pub fn decimated_lines(
    trace: &RayTrace,
    max_lines: usize,
    center: Point3<f64>,
) -> Option<LineGeometry> {
    let stations = &trace.stations;
    let first = stations.first()?;
    let valid0: Vec<usize> = (0..first.len()).filter(|&i| first[i].is_some()).collect();
    if valid0.is_empty() || max_lines == 0 {
        return None;
    }

    let selected: Vec<usize> = if valid0.len() <= max_lines {
        valid0
    } else {
        (0..max_lines)
            .map(|i| valid0[i * valid0.len() / max_lines])
            .collect()
    };

    let mut positions = Vec::new();
    let mut segments = Vec::new();
    for &ray in &selected {
        let mut previous: Option<u32> = None;
        for station in stations {
            match station.get(ray).copied().flatten() {
                Some(point) => {
                    let index = positions.len() as u32;
                    positions.push(point - center.coords);
                    if let Some(prev) = previous {
                        segments.push([prev, index]);
                    }
                    previous = Some(index);
                }
                None => previous = None,
            }
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some((positions, segments))
    }
}
