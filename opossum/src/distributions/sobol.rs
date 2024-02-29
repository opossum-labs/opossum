use super::Distribution;
use nalgebra::{point, Point3};
use num::Zero;
use sobol::{params::JoeKuoD6, Sobol};
use uom::si::f64::Length;

pub struct SobolDist {
    nr_of_rays: usize,
    side_length_x: Length,
    side_length_y: Length,
}

impl Distribution for SobolDist {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        let params = JoeKuoD6::minimal();
        let seq = Sobol::<f64>::new(2, &params);
        let offset_x = self.side_length_x / 2.0;
        let offset_y = self.side_length_y / 2.0;
        for point in seq.take(self.nr_of_rays) {
            let point_x = self.side_length_x * (point[0] - 0.5);
            let point_y = self.side_length_x * (point[1] - 0.5);
            points.push(point!(point_x, point_y, Length::zero()));
        }
        points
    }
}
