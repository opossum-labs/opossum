use super::Distribution;
use nalgebra::Point3;
use num::Zero;
use uom::si::f64::Length;

pub struct Grid {
    nr_of_points_x: usize,
    nr_of_points_y: usize,
    side_length: Length,
}

impl Grid {
    pub fn new(nr_of_points_x: usize, nr_of_points_y: usize, side_length: Length) -> Self {
        Self {
            nr_of_points_x,
            nr_of_points_y,
            side_length,
        }
    }
}

impl Distribution for Grid {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points_x = self.nr_of_points_x.clamp(1, usize::MAX);
        let nr_of_points_y = self.nr_of_points_y.clamp(1, usize::MAX);
        #[allow(clippy::cast_precision_loss)]
        let distance_x = if nr_of_points_x > 1 {
            self.side_length / ((nr_of_points_x - 1) as f64)
        } else {
            Length::zero()
        };
        #[allow(clippy::cast_precision_loss)]
        let distance_y = if nr_of_points_y > 1 {
            self.side_length / ((nr_of_points_y - 1) as f64)
        } else {
            Length::zero()
        };
        let offset_x = if nr_of_points_x > 1 {
            self.side_length / 2.0
        } else {
            Length::zero()
        };
        let offset_y = if nr_of_points_y > 1 {
            self.side_length / 2.0
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
}
