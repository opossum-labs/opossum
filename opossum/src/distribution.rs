#![warn(missing_docs)]
//! Module for creation of 2D point sets with a defined distribution

use nalgebra::{point, Point3};
use num::Zero;
use rand::Rng;
use sobol::{params::JoeKuoD6, Sobol};
use uom::si::{f64::Length, length::millimeter};

/// Distribution strategies
pub enum DistributionStrategy {
    /// Circular, hexapolar distribution with a given number of rings within a given radius
    Hexapolar(u8),
    /// Square, random distribution with a given number of points within a given side length
    Random(usize),
    /// Square, low-discrepancy quasirandom distribution with a given number of points within a given side length
    Sobol(usize),
}

impl DistributionStrategy {
    /// Generate a vector of 2D points within a given size (which depends on the concrete strategy)
    #[must_use]
    pub fn generate(&self, size: Length) -> Vec<Point3<Length>> {
        match self {
            Self::Hexapolar(rings) => hexapolar(*rings, size),
            Self::Random(nr_of_rays) => random(*nr_of_rays, size),
            Self::Sobol(nr_of_rays) => sobol(*nr_of_rays, size),
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
}
