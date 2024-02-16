#![warn(missing_docs)]
//! Module for creation of 2D point sets with a defined distribution

use nalgebra::{point, Point3};
use num::Zero;
use rand::Rng;
use sobol::{params::JoeKuoD6, Sobol};
use uom::si::{f64::Length, length::millimeter};

/// Distribution strategies
pub enum DistributionStrategy {
    /// Circular, hexapolar distribution within a given radius
    Hexapolar {
        /// number of rings of the hexaploar pattern
        nr_of_rings: u8,
    },
    /// Square, random distribution with a given number of points within a given side length
    Random {
        /// total number of points to be generated
        nr_of_points: usize,
    },
    /// Square, low-discrepancy quasirandom distribution with a given number of points within a given side length
    Sobol {
        /// total number of points to be generated
        nr_of_points: usize,
    },
    /// Square, evenly sized grid
    Grid {
        /// number of points in x direction
        nr_of_points_x: usize,
        /// number of points in y direction
        nr_of_points_y: usize,
    },
}
impl DistributionStrategy {
    /// Generate a vector of 2D points within a given size (which depends on the concrete [`DistributionStrategy`])
    #[must_use]
    pub fn generate(&self, size: Length) -> Vec<Point3<Length>> {
        match self {
            Self::Hexapolar { nr_of_rings } => hexapolar(*nr_of_rings, size),
            Self::Random { nr_of_points } => random(*nr_of_points, size),
            Self::Sobol { nr_of_points } => sobol(*nr_of_points, size),
            Self::Grid {
                nr_of_points_x,
                nr_of_points_y,
            } => grid(*nr_of_points_x, *nr_of_points_y, size),
        }
    }
}
fn hexapolar(rings: u8, radius: Length) -> Vec<Point3<Length>> {
    let mut points: Vec<Point3<Length>> = Vec::new();
    let radius_step = radius / f64::from(rings);
    points.push(point![Length::zero(), Length::zero(), Length::zero()]);
    for ring in 0u8..rings {
        let radius = f64::from(ring + 1) * radius_step;
        let points_per_ring = 6 * (ring + 1);
        let angle_step = 2.0 * std::f64::consts::PI / f64::from(points_per_ring);
        for point_nr in 0u8..points_per_ring {
            let point = (f64::from(point_nr) * angle_step).sin_cos();
            points.push(point![radius * point.0, radius * point.1, Length::zero()]);
        }
    }
    points
}
fn random(nr_of_rays: usize, side_length: Length) -> Vec<Point3<Length>> {
    let mut points: Vec<Point3<Length>> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..nr_of_rays {
        points.push(point![
            Length::new::<millimeter>(
                rng.gen_range(-side_length.get::<millimeter>()..side_length.get::<millimeter>())
            ),
            Length::new::<millimeter>(
                rng.gen_range(-side_length.get::<millimeter>()..side_length.get::<millimeter>())
            ),
            Length::zero()
        ]);
    }
    points
}
fn sobol(nr_of_rays: usize, side_length: Length) -> Vec<Point3<Length>> {
    let side_length = side_length.get::<millimeter>();
    let mut points: Vec<Point3<Length>> = Vec::new();
    let params = JoeKuoD6::minimal();
    let seq = Sobol::<f64>::new(2, &params);
    let offset = side_length / 2.0;
    for point in seq.take(nr_of_rays) {
        points.push(point!(
            Length::new::<millimeter>(point[0] - offset),
            Length::new::<millimeter>(point[1] - offset),
            Length::zero()
        ));
    }
    points
}
fn grid(nr_of_points_x: usize, nr_of_points_y: usize, side_length: Length) -> Vec<Point3<Length>> {
    let nr_of_points_x = nr_of_points_x.clamp(1, usize::MAX);
    let nr_of_points_y = nr_of_points_y.clamp(1, usize::MAX);
    #[allow(clippy::cast_precision_loss)]
    let distance_x = if nr_of_points_x > 1 {
        side_length / ((nr_of_points_x - 1) as f64)
    } else {
        Length::zero()
    };
    #[allow(clippy::cast_precision_loss)]
    let distance_y = if nr_of_points_y > 1 {
        side_length / ((nr_of_points_y - 1) as f64)
    } else {
        Length::zero()
    };
    let offset_x = if nr_of_points_x > 1 {
        side_length / 2.0
    } else {
        Length::zero()
    };
    let offset_y = if nr_of_points_y > 1 {
        side_length / 2.0
    } else {
        Length::zero()
    };
    let mut points: Vec<Point3<Length>> = Vec::new();
    for i_x in 0..nr_of_points_x {
        for i_y in 0..nr_of_points_y {
            #[allow(clippy::cast_precision_loss)]
            points.push(Point3::new(
                (i_x as f64) * distance_x - offset_x,
                (i_y as f64) * distance_y - offset_y,
                Length::zero(),
            ));
        }
    }
    points
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn strategy_random() {
        let strategy = DistributionStrategy::Random { nr_of_points: 10 };
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn strategy_sobol() {
        let strategy = DistributionStrategy::Sobol { nr_of_points: 10 };
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn strategy_grid_symmetric() {
        let strategy = DistributionStrategy::Grid {
            nr_of_points_x: 2,
            nr_of_points_y: 2,
        };
        let points = strategy.generate(Length::new::<millimeter>(1.0));
        assert_eq!(points.len(), 4);
        assert_eq!(
            points[0],
            Point3::new(
                Length::new::<millimeter>(-0.5),
                Length::new::<millimeter>(-0.5),
                Length::zero()
            )
        );
        assert_eq!(
            points[1],
            Point3::new(
                Length::new::<millimeter>(-0.5),
                Length::new::<millimeter>(0.5),
                Length::zero()
            )
        );
        assert_eq!(
            points[2],
            Point3::new(
                Length::new::<millimeter>(0.5),
                Length::new::<millimeter>(-0.5),
                Length::zero()
            )
        );
        assert_eq!(
            points[3],
            Point3::new(
                Length::new::<millimeter>(0.5),
                Length::new::<millimeter>(0.5),
                Length::zero()
            )
        );
    }
    #[test]
    fn strategy_grid_size_one() {
        let strategy = DistributionStrategy::Grid {
            nr_of_points_x: 1,
            nr_of_points_y: 1,
        };
        let points = strategy.generate(Length::new::<millimeter>(1.0));
        assert_eq!(points.len(), 1);
        assert_eq!(
            points[0],
            Point3::new(Length::zero(), Length::zero(), Length::zero())
        );
    }
    #[test]
    fn strategy_grid_asymmetric() {
        let strategy = DistributionStrategy::Grid {
            nr_of_points_x: 1,
            nr_of_points_y: 2,
        };
        let points = strategy.generate(Length::new::<millimeter>(1.0));
        assert_eq!(points.len(), 2);
        assert_eq!(
            points[0],
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(-0.5),
                Length::zero()
            )
        );
        assert_eq!(
            points[1],
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(0.5),
                Length::zero()
            )
        );
    }
}
