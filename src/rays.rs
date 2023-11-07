#![warn(missing_docs)]
//! Module for handling rays
use nalgebra::{point, Point2, Point3};
use plotters::prelude::{ChartBuilder, Circle, EmptyElement};
use plotters::series::PointSeries;
use plotters::style::RED;
use rand::Rng;
use uom::si::f64::{Energy, Length};

use crate::error::OpossumError;
use crate::plottable::Plottable;

///Struct that contains all informatino about a ray
#[derive(Debug)]
pub struct Ray {
    ///Stores all positions of the ray
    pos: Point3<f64>, // this should be a vector of points?
    // ///stores the current propagation direction of the ray
    // dir: Vector3<f64>,
    // ///stores the polarization vector (Jones vector) of the ray
    // pol: Vector2<Complex<f64>>,
    // ///energy of the ray
    // e: Energy,
    // ///Wavelength of the ray in nm
    // wvl: Length,
    // ///id of the ray
    // id: usize,
    // ///Bounce count of the ray. Necessary to check if the maximum number of bounces is reached
    // bounce: usize,
    // //True if ray is allowd to further propagate, false else
    // //valid:  bool,
}
impl Ray {
    /// Create a new collimated ray.
    ///
    /// Generate a ray a horizontally polarized ray collinear with the z axis (optical axis).
    pub fn new_collimated(position: Point2<f64>, _wave_length: Length, _energy: Energy) -> Self {
        Self {
            pos: Point3::new(position.x, position.y, 0.0),
            //dir: Vector3::new(0.0, 0.0, 1.0),
            // pol: Vector2::new(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)), // horizontal polarization
            // e: energy,
            // wvl: wave_length,
            // id: 0,
            // bounce: 0,
        }
    }
}
///Struct containing all relevant information of a created bundle of rays
#[derive(Debug)]
pub struct Rays {
    ///vector containing rays
    rays: Vec<Ray>,
    //Maximum number of bounces
    //max_bounces:    usize, do we need this here?
}

/// Strategy for the creation of a 2D point set
pub enum DistributionStrategy {
    /// Hexagonal distribution with a given number of rings
    Hexapolar(u8),
    /// Random distribution of 2D points with a given number of points
    Random(usize),
    //Sobol(usize),
}
impl DistributionStrategy {
    /// Generate a vector of 2D points within a given radius around (0.0,0.0).
    pub fn generate(&self, radius: f64) -> Vec<Point2<f64>> {
        match self {
            DistributionStrategy::Hexapolar(rings) => hexapolar(*rings, radius),
            DistributionStrategy::Random(nr_of_rays) => random(*nr_of_rays, radius),
            //DistributionStrategy::Sobol(nr_of_rays) => sobol(*nr_of_rays, radius),
        }
    }
}
fn hexapolar(rings: u8, radius: f64) -> Vec<Point2<f64>> {
    let mut points: Vec<Point2<f64>> = Vec::new();
    let radius_step = radius / rings as f64;
    points.push(point![0.0, 0.0]);
    for ring in 0u8..rings {
        let radius = (ring + 1) as f64 * radius_step;
        let points_per_ring = 6 * (ring + 1);
        let angle_step = 2.0 * std::f64::consts::PI / (points_per_ring as f64);
        for point_nr in 0u8..points_per_ring {
            let point = ((point_nr as f64) * angle_step).sin_cos();
            points.push(point![radius * point.0, radius * point.1]);
        }
    }
    points
}
fn random(nr_of_rays: usize, radius: f64) -> Vec<Point2<f64>> {
    let mut points: Vec<Point2<f64>> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..nr_of_rays {
        let angle = rng.gen_range(0.0..2.0 * std::f64::consts::PI);
        let radius = rng.gen_range(0.0..radius);
        let point = angle.sin_cos();
        points.push(point![radius * point.0, radius * point.1]);
    }
    points
}
// fn sobol(nr_of_rays: usize, radius: f64) -> Vec<Point2<f64>> {
//     vec![]
// }
impl Rays {
    /// Generate a set of collimated rays (collinear with optical axis).
    pub fn new_uniform_collimated(
        radius: f64,
        wave_length: Length,
        energy: Energy,
        strategy: DistributionStrategy,
    ) -> Self {
        let points: Vec<Point2<f64>> = strategy.generate(radius);
        let nr_of_rays = points.len();
        let mut rays: Vec<Ray> = Vec::new();
        for point in points {
            let ray = Ray::new_collimated(point, wave_length, energy / nr_of_rays as f64);
            rays.push(ray);
        }
        Self { rays }
    }
}
impl Plottable for Rays {
    fn chart<B: plotters::prelude::DrawingBackend>(
        &self,
        root: &plotters::prelude::DrawingArea<B, plotters::coord::Shift>,
    ) -> crate::error::OpmResult<()> {
        let x_min = self
            .rays
            .iter()
            .map(|r| r.pos.x)
            .fold(f64::INFINITY, f64::min)
            * 1.1;
        let x_max = self
            .rays
            .iter()
            .map(|r| r.pos.x)
            .fold(f64::NEG_INFINITY, f64::max)
            * 1.1;
        let y_min = self
            .rays
            .iter()
            .map(|r| r.pos.y)
            .fold(f64::INFINITY, f64::min)
            * 1.1;
        let y_max = self
            .rays
            .iter()
            .map(|r| r.pos.y)
            .fold(f64::NEG_INFINITY, f64::max)
            * 1.1;
        let mut chart = ChartBuilder::on(root)
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {}", e)))?;

        chart
            .configure_mesh()
            .x_desc("x")
            .y_desc("y")
            .draw()
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {}", e)))?;
        let points: Vec<(f64, f64)> = self.rays.iter().map(|ray| (ray.pos.x, ray.pos.y)).collect();
        let series = PointSeries::of_element(points, 5, &RED, &|c, s, st| {
            EmptyElement::at(c)    // We want to construct a composed element on-the-fly
                + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
        });

        chart
            .draw_series(series)
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {}", e)))?;
        root.present()
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {}", e)))?;
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use uom::si::{energy::joule, length::nanometer};

    use super::*;
    #[test]
    fn strategy_hexapolar() {
        let strategy = DistributionStrategy::Hexapolar(0);
        assert_eq!(strategy.generate(1.0).len(), 1);
        let strategy = DistributionStrategy::Hexapolar(1);
        assert_eq!(strategy.generate(1.0).len(), 7);
        let strategy = DistributionStrategy::Hexapolar(5);
        assert_eq!(strategy.generate(1.0).len(), 91);
    }
    #[test]
    fn strategy_random() {
        let strategy = DistributionStrategy::Random(10);
        assert_eq!(strategy.generate(1.0).len(), 10);
    }
    #[test]
    fn new_uniform_collimated() {
        let rays = Rays::new_uniform_collimated(
            1.0,
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
            DistributionStrategy::Hexapolar(2),
        );
        assert_eq!(rays.rays.len(), 19);
    }
}
