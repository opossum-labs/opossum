use crate::nodes::fluence_detector::Fluence;
use nalgebra::Point2;
use uom::si::f64::{Area, Energy, Length};

const FRAC_1_2PI: f64 = 0.159_154_943_091_895_35;
pub struct Gaussian2D {
    mean: Point2<Length>,
    sigma_square: Area,
    amplitude: Fluence,
}
impl Gaussian2D {
    pub fn new(mean: Point2<Length>, sigma: Length, weight: Energy) -> Self {
        let sigma_square = sigma * sigma;
        Self {
            mean,
            sigma_square,
            amplitude: weight * FRAC_1_2PI / sigma_square,
        }
    }
    fn distance_squared(point1: &Point2<Length>, point2: &Point2<Length>) -> Area {
        (point1.x - point2.x) * (point1.x - point2.x)
            + (point1.y - point2.y) * (point1.y - point2.y)
    }
    pub fn value(&self, point: Point2<Length>) -> Fluence {
        let dist_square = Self::distance_squared(&point, &self.mean);
        self.amplitude * (-dist_square / (2. * self.sigma_square)).exp()
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
