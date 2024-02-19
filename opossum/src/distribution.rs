#![warn(missing_docs)]
//! Module for creation of 2D point sets with a defined distribution

use nalgebra::{point, Point3};
use num::ToPrimitive;
use num::Zero;
use rand::Rng;
use sobol::{params::JoeKuoD6, Sobol};
use std::f64::consts::PI;
use uom::si::{f64::Length, length::millimeter};

/// Distribution strategies
pub enum DistributionStrategy {
    /// Circular, hexapolar distribution with a given number of rings within a given radius
    Hexapolar(u8),
    /// Square, random distribution with a given number of points within a given side length
    Random(usize),
    /// Square, low-discrepancy quasirandom distribution with a given number of points within a given side length
    Sobol(usize),
    /// Square, evenly sized grid with the given number of points
    Grid(usize),
    /// Fibonacci-distributed circular distribution with the given number of points
    FibonacciCircular(usize),
    /// Fibonacci-distributed square distribution with the given number of points
    FibonacciSquare(usize),
}

impl DistributionStrategy {
    /// Generate a vector of 2D points within a given size (which depends on the concrete [`DistributionStrategy`])
    #[must_use]
    pub fn generate(&self, size: Length) -> Vec<Point3<Length>> {
        match self {
            Self::Hexapolar(rings) => hexapolar(*rings, size),
            Self::Random(nr_of_rays) => random(*nr_of_rays, size),
            Self::Sobol(nr_of_rays) => sobol(*nr_of_rays, size),
            Self::Grid(nr_of_rays) => grid(*nr_of_rays, size),
            Self::FibonacciCircular(nr_of_rays) => fibonacci(*nr_of_rays, size),
            Self::FibonacciSquare(nr_of_rays) => fibonacci_square(*nr_of_rays, size),
        }
    }
}

fn fibonacci(nr_of_rays: usize, radius: Length) -> Vec<Point3<Length>> {
    let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_rays);
    let golden_ratio = (1. + f64::sqrt(5.)) / 2.;
    for i in 0_usize..nr_of_rays {
        let sin_cos = f64::sin_cos(2. * PI * (i.to_f64().unwrap() / golden_ratio).fract());
        let sqrt_r = f64::sqrt(i.to_f64().unwrap() / nr_of_rays.to_f64().unwrap());
        points.push(point![
            radius * sin_cos.0 * sqrt_r,
            radius * sin_cos.1 * sqrt_r,
            Length::zero()
        ]);
    }
    points
}
fn fibonacci_square(nr_of_rays: usize, size: Length) -> Vec<Point3<Length>> {
    let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_rays);
    let golden_ratio = (1. + f64::sqrt(5.)) / 2.;
    for i in 0_usize..nr_of_rays {
        let i_f64 = i.to_f64().unwrap();
        points.push(point![
            size * (i_f64 / golden_ratio).fract(),
            size * (i_f64 / nr_of_rays.to_f64().unwrap()),
            Length::zero()
        ]);
    }
    points
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
fn grid(nr_of_rays: usize, side_length: Length) -> Vec<Point3<Length>> {
    #[allow(clippy::cast_precision_loss)]
    let distance = side_length / ((nr_of_rays - 1) as f64);
    let offset = side_length / 2.0;
    let mut points: Vec<Point3<Length>> = Vec::new();
    for i_x in 0..nr_of_rays {
        for i_y in 0..nr_of_rays {
            #[allow(clippy::cast_precision_loss)]
            points.push(Point3::new(
                (i_x as f64) * distance - offset,
                (i_y as f64) * distance - offset,
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
        let strategy = DistributionStrategy::Random(10);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn strategy_sobol() {
        let strategy = DistributionStrategy::Sobol(10);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn strategy_grid() {
        let strategy = DistributionStrategy::Grid(2);
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
}
