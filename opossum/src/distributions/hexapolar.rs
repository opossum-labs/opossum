//! Circular, hexapolar distribution
use super::Distribution;
use nalgebra::{point, Point3};
use num::Zero;
use uom::si::f64::Length;

pub struct Hexapolar {
    nr_of_rings: u8,
    radius: Length,
}
impl Hexapolar {
    #[must_use]
    pub fn new(radius: Length, nr_of_rings: u8) -> Self {
        Self {
            nr_of_rings,
            radius,
        }
    }
}
impl Distribution for Hexapolar {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        let radius_step = self.radius / f64::from(self.nr_of_rings);
        points.push(Point3::origin());
        for ring in 0u8..self.nr_of_rings {
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
}
