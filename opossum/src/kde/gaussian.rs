use crate::nodes::fluence_detector::Fluence;
use nalgebra::Point2;
use std::f64::consts::FRAC_1_PI;
use uom::si::{
    f64::{Energy, Length, Ratio},
    ratio::ratio,
};

pub struct Gaussian2D {
    mean: Point2<Length>,
    sigma: Length,
    weight: Energy,
}
impl Gaussian2D {
    pub const fn new(mean: Point2<Length>, sigma: Length, weight: Energy) -> Self {
        Self {
            mean,
            sigma,
            weight,
        }
    }
    fn distance(point1: &Point2<Length>, point2: &Point2<Length>) -> Length {
        ((point1.x - point2.x) * (point1.x - point2.x)
            + (point1.y - point2.y) * (point1.y - point2.y))
            .sqrt()
    }
    pub fn value(&self, point: Point2<Length>) -> Fluence {
        let distance = Self::distance(&point, &self.mean);
        let factor = Ratio::new::<ratio>(0.5 * FRAC_1_PI); // 1/2pi
        self.weight * factor / (self.sigma * self.sigma)
            * (-distance * distance / (2. * self.sigma * self.sigma)).exp()
    }
}

#[cfg(test)]
mod test {
    use super::Gaussian2D;
    use crate::{joule, millimeter, nodes::fluence_detector::Fluence, utils::griddata::linspace};
    use approx::assert_abs_diff_eq;
    use nalgebra::DMatrix;
    use uom::si::{f64::Ratio, ratio::ratio};

    #[test]
    fn check_norm() {
        let g = Gaussian2D::new(millimeter!(0.0, 0.0), millimeter!(5.0), joule!(1.0));
        let nr_of_points = (120, 120);
        let grid_element_area = millimeter!(200.0) * millimeter!(200.0)
            / Ratio::new::<ratio>((nr_of_points.0 * nr_of_points.1) as f64);

        let x_pos = linspace(-100.0, 100.0, nr_of_points.0).unwrap();
        let y_pos = linspace(-100.0, 100.0, nr_of_points.1).unwrap();
        let mut field = DMatrix::<Fluence>::zeros(nr_of_points.0, nr_of_points.1);
        for i_x in 0..nr_of_points.0 {
            for i_y in 0..nr_of_points.1 {
                field[(i_x, i_y)] = g.value(millimeter!(x_pos[i_x], y_pos[i_y]));
            }
        }
        let total_energy = field.sum() * grid_element_area;
        assert_abs_diff_eq!(total_energy.value, 1.0, epsilon = 0.02);
    }
}
