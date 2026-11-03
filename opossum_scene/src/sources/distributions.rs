//! Coarser position distributions, for drawing a source.

use nalgebra::Point2;
use opossum_core::{
    distributions::position::{Grid, HexagonalTiling, PosDistType, PositionDistribution},
    error::OpmResult,
    utils::{to_f64, try_f64_to_usize},
};

/// Returns a distribution of the same kind over the same area with at most `max_points` points.
///
/// The distribution only gets fewer points - fewer rings, a coarser grid, ... - and one that
/// already fits is returned unchanged. Every kind of distribution is coarsened here, so a new
/// kind does not compile until it is.
///
/// # Arguments
///
/// - `dist`: the distribution to coarsen
/// - `max_points`: the most points the distribution may have, at least one
///
/// # Errors
///
/// This function returns an error if the coarser distribution cannot be built.
pub(super) fn coarsened_distribution(
    dist: &PosDistType,
    max_points: usize,
) -> OpmResult<PosDistType> {
    match dist {
        PosDistType::Random(random) => {
            let mut coarse = *random;
            coarse.set_nr_of_points(random.nr_of_points().min(max_points))?;
            Ok(coarse.into())
        }
        PosDistType::Sobol(sobol) => {
            let mut coarse = *sobol;
            coarse.set_nr_of_points(sobol.nr_of_points().min(max_points))?;
            Ok(coarse.into())
        }
        PosDistType::FibonacciRectangle(fibonacci) => {
            let mut coarse = *fibonacci;
            coarse.set_nr_of_points(fibonacci.nr_of_points().min(max_points))?;
            Ok(coarse.into())
        }
        PosDistType::FibonacciEllipse(fibonacci) => {
            let mut coarse = *fibonacci;
            coarse.set_nr_of_points(fibonacci.nr_of_points().min(max_points))?;
            Ok(coarse.into())
        }
        PosDistType::Hexapolar(hexapolar) => {
            let mut coarse = *hexapolar;
            coarse.set_nr_of_rings(hexapolar.nr_of_rings().min(max_rings_for(max_points)));
            Ok(coarse.into())
        }
        PosDistType::HexagonalTiling(tiling) => Ok(coarsened_tiling(tiling, max_points).into()),
        PosDistType::Grid(grid) => coarsened_grid(grid, max_points),
    }
}

/// Returns the most rings a hexapolar distribution of at most `max_points` points can have.
///
/// With `r` rings it has `1 + 3·r·(r + 1)` points: the center and `6·k` on ring `k`.
///
/// # Arguments
///
/// - `max_points`: the most points the distribution may have
///
/// # Returns
///
/// The number of rings, zero if only the center fits.
fn max_rings_for(max_points: usize) -> u8 {
    let mut rings = 0_u8;
    while rings < u8::MAX {
        let next = usize::from(rings) + 1;
        if 1 + 3 * next * (next + 1) > max_points {
            break;
        }
        rings += 1;
    }
    rings
}

/// Returns a hexagonal tiling over the same radius with the most hexagons that keep it within
/// `max_points` points.
///
/// # Arguments
///
/// - `tiling`: the tiling to coarsen
/// - `max_points`: the most points the tiling may have
///
/// # Returns
///
/// The coarser tiling, the center alone if not even one ring of hexagons fits.
fn coarsened_tiling(tiling: &HexagonalTiling, max_points: usize) -> HexagonalTiling {
    // The tiling holds the whole hexagon of as many rings, so no more rings than a hexapolar
    // distribution of that size can fit.
    let mut coarse = *tiling;
    let mut nr_of_hex = tiling
        .nr_of_hex_along_radius()
        .min(max_rings_for(max_points));
    coarse.set_nr_of_hex_along_radius(nr_of_hex);
    while nr_of_hex > 0 && coarse.generate().len() > max_points {
        nr_of_hex -= 1;
        coarse.set_nr_of_hex_along_radius(nr_of_hex);
    }
    coarse
}

/// Returns a grid over the same side lengths with at most `max_points` points.
///
/// # Arguments
///
/// - `grid`: the grid to coarsen
/// - `max_points`: the most points the grid may have, at least one
///
/// # Errors
///
/// This function returns an error if the coarser grid cannot be built.
fn coarsened_grid(grid: &Grid, max_points: usize) -> OpmResult<PosDistType> {
    let (nr_x, nr_y) = (grid.nr_of_points_x(), grid.nr_of_points_y());
    let nr_of_points = nr_x.saturating_mul(nr_y);
    if nr_of_points <= max_points {
        return Ok((*grid).into());
    }
    // Both axes shrink by the same factor; the longer one then takes up what the shorter left.
    let scale = (to_f64(max_points) / to_f64(nr_of_points)).sqrt();
    let (shorter, longer) = (nr_x.min(nr_y), nr_x.max(nr_y));
    let shorter = try_f64_to_usize((to_f64(shorter) * scale).floor())
        .unwrap_or(0)
        .max(1);
    let longer = longer.min(max_points / shorter);
    let mut coarse = *grid;
    if nr_x <= nr_y {
        coarse.set_nr_of_points(Point2::new(shorter, longer))?;
    } else {
        coarse.set_nr_of_points(Point2::new(longer, shorter))?;
    }
    Ok(coarse.into())
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;
    use opossum_core::{distributions::position::Hexapolar, millimeter};
    use strum::IntoEnumIterator;
    use uom::{num_traits::Zero, si::f64::Length};

    /// Every kind of distribution, at its defaults, coarsens to at most the asked number of points
    /// and keeps at least one, and stays as it is when it already fits.
    #[test]
    fn every_distribution_coarsens_to_at_most_the_asked_points() -> OpmResult<()> {
        for dist in PosDistType::iter() {
            let coarse = coarsened_distribution(&dist, 50)?;
            let nr_of_points = coarse.generate().generate().len();
            assert!((1..=50).contains(&nr_of_points), "{dist}: {nr_of_points}");
            assert_eq!(coarsened_distribution(&dist, 1_000_000)?, dist, "{dist}");
        }
        Ok(())
    }

    #[test]
    fn max_rings_for_counts_the_center_and_whole_rings() {
        // 0, 1, 2, ... rings hold 1, 7, 19, ... points.
        for (max_points, rings) in [(0, 0), (6, 0), (7, 1), (18, 1), (19, 2), (100, 5)] {
            assert_eq!(max_rings_for(max_points), rings, "{max_points}");
        }
        assert_eq!(max_rings_for(usize::MAX), u8::MAX);
    }

    #[test]
    fn a_coarsened_hexapolar_distribution_keeps_fewer_rings_over_the_same_radius() -> OpmResult<()>
    {
        let fine = Hexapolar::new(millimeter!(5.0), 15)?;
        let PosDistType::Hexapolar(coarse) = coarsened_distribution(&fine.into(), 100)? else {
            panic!("a coarsened hexapolar distribution is still hexapolar");
        };
        assert_eq!(coarse.nr_of_rings(), 5);
        assert_eq!(coarse.radius(), fine.radius());
        assert_eq!(coarse.generate().len(), 91);
        Ok(())
    }

    #[test]
    fn a_coarsened_tiling_keeps_the_most_hexagons_that_fit_over_the_same_radius() -> OpmResult<()> {
        let fine = HexagonalTiling::new(millimeter!(5.0), 7)?;
        let coarse = coarsened_tiling(&fine, 50);
        let points = coarse.generate();
        assert!(points.len() <= 50);
        let outermost = points
            .iter()
            .map(|point| (point.x * point.x + point.y * point.y).sqrt())
            .fold(Length::zero(), Length::max);
        assert_relative_eq!(outermost.value, fine.radius().value, max_relative = 1e-9);
        let mut finer = coarse;
        finer.set_nr_of_hex_along_radius(coarse.nr_of_hex_along_radius() + 1);
        assert!(finer.generate().len() > 50, "one hexagon more does not fit");
        Ok(())
    }

    #[test]
    fn a_coarsened_grid_shrinks_both_axes_and_keeps_the_side_lengths() -> OpmResult<()> {
        let fine = Grid::new(millimeter!(4.0, 2.0), Point2::new(100, 100))?;
        let PosDistType::Grid(coarse) = coarsened_grid(&fine, 100)? else {
            panic!("a coarsened grid is still a grid");
        };
        assert_eq!(coarse.nr_of_points(), Point2::new(10, 10));
        assert_eq!(coarse.side_length(), fine.side_length());
        Ok(())
    }

    #[test]
    fn a_stretched_grid_coarsens_along_its_long_side() -> OpmResult<()> {
        for (fine, coarse) in [
            (Point2::new(1000, 1), Point2::new(100, 1)),
            (Point2::new(1, 1000), Point2::new(1, 100)),
        ] {
            let PosDistType::Grid(coarsened) =
                coarsened_grid(&Grid::new(millimeter!(1.0, 1.0), fine)?, 100)?
            else {
                panic!("a coarsened grid is still a grid");
            };
            assert_eq!(coarsened.nr_of_points(), coarse);
        }
        Ok(())
    }
}
