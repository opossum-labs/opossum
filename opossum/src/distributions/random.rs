use super::Distribution;
use nalgebra::{point, Point3};
use num::Zero;
use rand::Rng;
use uom::si::f64::Length;

pub struct Random {
    nr_of_rays: usize,
    side_length_x: Length,
    side_length_y: Length,
}
impl Random {
    pub fn new(side_length_x: Length, side_length_y: Length, nr_of_rays: usize) -> Self {
        Self {
            nr_of_rays,
            side_length_x,
            side_length_y,
        }
    }
}
impl Distribution for Random {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        let mut rng = rand::thread_rng();
        for _ in 0..self.nr_of_rays {
            let point_x = self.side_length_x * rng.gen_range(-1.0..1.0);
            let point_y = self.side_length_y * rng.gen_range(-1.0..1.0);
            points.push(point![point_x, point_y, Length::zero()]);
        }
        points
    }
}
