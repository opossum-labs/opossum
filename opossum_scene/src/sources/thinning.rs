//! Thinning a bundle of rays that has no distribution to coarsen, for drawing it.

use std::{
    collections::BTreeMap,
    f64::consts::{PI, TAU},
};

use nalgebra::{Point2, Vector2, Vector3};
use opossum_core::light::{Ray, Rays};

/// Returns at most `max` rays of a bundle, spread evenly over its cross section.
///
/// A bundle of millions of rays cannot be shown, and a few hundred that cover it evenly show it
/// as well. The subset does not depend on how the rays were laid out - a grid, rings, random points
/// or a loaded image - because it is taken from where the rays are, not from their order in the
/// bundle:
///
/// - Each ray is placed in the plane across the bundle's mean direction, by where it starts. A
///   bundle that starts in one point (a point source) is placed by its directions instead.
/// - Around the center, the outermost ray of each angular sector is kept, so the subset spans
///   the whole bundle.
/// - A raster of square cells is laid over the plane, sized so that about `max` of them are
///   occupied, and the first ray of every occupied cell is kept.
///
/// The subset is deterministic and keeps the rays in their original order. Their energies are
/// left as they are, so the subset no longer carries the bundle's total energy.
///
/// # Arguments
///
/// - `rays`: the bundle to thin
/// - `max`: the most rays to keep
///
/// # Returns
///
/// The bundle unchanged if it holds no more than `max` rays, otherwise a new bundle holding the
/// chosen rays.
pub(super) fn thinned(rays: &Rays, max: usize) -> Rays {
    if rays.iter().len() <= max {
        return rays.clone();
    }
    let all: Vec<&Ray> = rays.iter().collect();
    thinned_indices(rays, max)
        .into_iter()
        .filter_map(|i| all.get(i).map(|ray| (*ray).clone()))
        .collect::<Vec<Ray>>()
        .into()
}

/// Chooses the rays [`thinned`] keeps.
///
/// # Arguments
///
/// - `rays`: the bundle to thin
/// - `max`: the most rays to keep
///
/// # Returns
///
/// The indices of the kept rays, ascending.
// Cell and sector numbers are floored from finite, bounded coordinates.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn thinned_indices(rays: &Rays, max: usize) -> Vec<usize> {
    /// The most angular sectors whose outermost ray is kept.
    const RIM_SECTORS: usize = 32;
    if max == 0 {
        return Vec::new();
    }
    let keys = transverse_keys(rays);
    let n = keys.len() as f64;
    let center = keys
        .iter()
        .fold(Vector2::zeros(), |sum, key| sum + key.coords)
        / n;
    let mut keep = vec![false; keys.len()];

    let sectors = (max / 4).clamp(1, RIM_SECTORS);
    let mut outermost: Vec<Option<(usize, f64)>> = vec![None; sectors];
    for (i, key) in keys.iter().enumerate() {
        let offset = key.coords - center;
        let radius = offset.norm();
        let sector = ((offset.y.atan2(offset.x) + PI) / TAU * sectors as f64) as usize % sectors;
        if let Some(best) = outermost.get_mut(sector)
            && best.is_none_or(|(_, r)| radius > r)
        {
            *best = Some((i, radius));
        }
    }
    for (i, _) in outermost.iter().flatten() {
        if let Some(k) = keep.get_mut(*i) {
            *k = true;
        }
    }
    let on_rim = keep.iter().filter(|k| **k).count();

    let (low, high) = keys.iter().fold(
        (
            Vector2::repeat(f64::INFINITY),
            Vector2::repeat(f64::NEG_INFINITY),
        ),
        |(low, high), key| (low.inf(&key.coords), high.sup(&key.coords)),
    );
    let extent = (high - low).max();
    if extent > 0.0 {
        let mut cell = extent / (max as f64).sqrt();
        loop {
            let mut first_in_cell = BTreeMap::<(i64, i64), usize>::new();
            for (i, key) in keys.iter().enumerate() {
                let at = (key.coords - low) / cell;
                first_in_cell
                    .entry((at.x.floor() as i64, at.y.floor() as i64))
                    .or_insert(i);
            }
            let mut fill: Vec<usize> = first_in_cell
                .into_values()
                .filter(|i| keep.get(*i).is_some_and(|k| !k))
                .collect();
            // Larger cells mean fewer of them; past twice the extent there are at most four.
            if on_rim + fill.len() <= max || cell > 2.0 * extent {
                fill.sort_unstable();
                fill.truncate(max.saturating_sub(on_rim));
                for i in fill {
                    if let Some(k) = keep.get_mut(i) {
                        *k = true;
                    }
                }
                break;
            }
            cell *= 1.1;
        }
    }
    keep.iter()
        .enumerate()
        .filter_map(|(i, k)| k.then_some(i))
        .take(max)
        .collect()
}

/// Places every ray of a bundle in the plane across the bundle's mean direction.
///
/// A ray is placed by where it starts, in meters. If all rays start in one point - a point
/// source - that says nothing, and each ray is placed by its direction instead.
///
/// # Arguments
///
/// - `rays`: the bundle whose rays to place
///
/// # Returns
///
/// One point per ray, in bundle order.
fn transverse_keys(rays: &Rays) -> Vec<Point2<f64>> {
    let mean = rays
        .iter()
        .fold(Vector3::zeros(), |sum, ray| {
            sum + ray
                .direction()
                .try_normalize(0.0)
                .unwrap_or_else(Vector3::z)
        })
        .try_normalize(1e-12)
        .unwrap_or_else(Vector3::z);
    // Of the axes, the one least parallel to the mean direction spans the plane with it.
    let axis = if mean.x.abs() <= mean.y.abs() && mean.x.abs() <= mean.z.abs() {
        Vector3::x()
    } else if mean.y.abs() <= mean.z.abs() {
        Vector3::y()
    } else {
        Vector3::z()
    };
    let u = mean.cross(&axis).normalize();
    let v = mean.cross(&u);
    let across = |p: Vector3<f64>| Point2::new(p.dot(&u), p.dot(&v));

    let by_position: Vec<Point2<f64>> = rays
        .iter()
        .map(|ray| {
            let p = ray.position();
            across(Vector3::new(p.x.value, p.y.value, p.z.value))
        })
        .collect();
    let spread = by_position
        .first()
        .is_some_and(|first| by_position.iter().any(|p| (p - first).norm() > 1e-12));
    if spread {
        by_position
    } else {
        rays.iter()
            .map(|ray| {
                across(
                    ray.direction()
                        .try_normalize(0.0)
                        .unwrap_or_else(Vector3::z),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;
    use opossum_core::{joule, millimeter, nanometer};
    use uom::si::length::millimeter;

    /// A bundle of collimated rays starting at the given points, in millimeters.
    fn collimated_at(points: impl IntoIterator<Item = (f64, f64)>) -> Rays {
        let mut rays = Rays::default();
        for (x, y) in points {
            rays.add_ray(
                Ray::new_collimated(millimeter!(x, y, 0.0), nanometer!(1000.0), joule!(1.0))
                    .unwrap(),
            );
        }
        rays
    }

    /// The start points of a bundle, in millimeters.
    fn starts_of(rays: &Rays) -> Vec<(f64, f64)> {
        rays.iter()
            .map(|ray| {
                let p = ray.position();
                (p.x.get::<millimeter>(), p.y.get::<millimeter>())
            })
            .collect()
    }

    #[test]
    fn thinning_a_small_bundle_leaves_it_as_it_is() {
        let rays = collimated_at([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        assert_eq!(thinned(&rays, 3), rays);
        assert_eq!(thinned(&rays, 100), rays);
    }

    #[test]
    fn thinning_is_deterministic() {
        let rays = collimated_at((0..50).flat_map(|i| (0..50).map(move |j| (i.into(), j.into()))));
        assert_eq!(
            starts_of(&thinned(&rays, 100)),
            starts_of(&thinned(&rays, 100))
        );
    }

    /// A grid thinned by striding over its rows would leave whole columns out. Spread over the
    /// cross section instead, every tenth of the grid still holds a ray.
    #[test]
    fn a_thinned_grid_covers_the_whole_grid() {
        let rays =
            collimated_at((0..100).flat_map(|i| (0..100).map(move |j| (i.into(), j.into()))));
        let starts = starts_of(&thinned(&rays, 400));
        assert!(starts.len() <= 400, "{} rays kept", starts.len());
        assert!(starts.len() >= 200, "only {} rays kept", starts.len());
        for block_x in 0..10 {
            for block_y in 0..10 {
                let covered = starts.iter().any(|(x, y)| {
                    (f64::from(block_x) * 10.0..f64::from(block_x + 1) * 10.0).contains(x)
                        && (f64::from(block_y) * 10.0..f64::from(block_y + 1) * 10.0).contains(y)
                });
                assert!(covered, "block ({block_x}, {block_y}) was left empty");
            }
        }
    }

    /// Rings from the inside out, as a hexapolar source lays them: the rim has to survive, or the
    /// drawn bundle would look narrower than it is.
    #[test]
    fn a_thinned_bundle_keeps_its_rim() {
        let rings = 20;
        let rays = collimated_at((1..=rings).flat_map(|ring| {
            let count = 6 * ring;
            (0..count).map(move |k| {
                let angle = 2.0 * PI * f64::from(k) / f64::from(count);
                let radius = f64::from(ring);
                (radius * angle.cos(), radius * angle.sin())
            })
        }));
        let thinned = thinned(&rays, 200);
        let on_rim = starts_of(&thinned)
            .iter()
            .filter(|(x, y)| (x.hypot(*y) - f64::from(rings)).abs() < 1e-9)
            .count();
        assert!(thinned.nr_of_rays(false) <= 200);
        assert!(on_rim >= 32, "only {on_rim} rays kept on the rim");
    }

    /// Rays that all start in one point differ only in where they go, so that is what they are
    /// spread by.
    #[test]
    fn a_point_source_is_thinned_by_direction() {
        let mut rays = Rays::default();
        for i in -10..=10 {
            for j in -10..=10 {
                let direction = Vector3::new(0.01 * f64::from(i), 0.01 * f64::from(j), 1.0);
                rays.add_ray(
                    Ray::new(
                        millimeter!(0.0, 0.0, 0.0),
                        direction,
                        nanometer!(1000.0),
                        joule!(1.0),
                    )
                    .unwrap(),
                );
            }
        }
        let thinned = thinned(&rays, 60);
        let kept = thinned.nr_of_rays(false);
        assert!(kept <= 60);
        // Placed by their (identical) start points, all rays would fall into one cell and one
        // sector, and only one would be kept.
        assert!(kept >= 30, "only {kept} rays kept");
        let widest = |rays: &Rays| {
            rays.iter()
                .map(|ray| ray.direction().x.hypot(ray.direction().y))
                .fold(0.0, f64::max)
        };
        assert_relative_eq!(widest(&thinned), widest(&rays));
    }
}
