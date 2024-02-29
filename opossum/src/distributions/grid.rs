//! Rectangular, evenly sized grid distribution
use crate::error::OpmResult;
use super::Distribution;
use nalgebra::Point3;
use num::Zero;
use uom::si::f64::Length;

pub struct Grid {
    nr_of_points: (usize, usize),
    side_length: (Length, Length),
}

impl Grid {
    pub fn new(side_length: (Length, Length), nr_of_points: (usize, usize)) -> OpmResult<Self> {
        Ok(Self {
            nr_of_points,
            side_length,
        })
    }
}

impl Distribution for Grid {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points_x = self.nr_of_points.0.clamp(1, usize::MAX);
        let nr_of_points_y = self.nr_of_points.0.clamp(1, usize::MAX);
        #[allow(clippy::cast_precision_loss)]
        let distance_x = if nr_of_points_x > 1 {
            self.side_length.0 / ((nr_of_points_x - 1) as f64)
        } else {
            Length::zero()
        };
        #[allow(clippy::cast_precision_loss)]
        let distance_y = if nr_of_points_y > 1 {
            self.side_length.1 / ((nr_of_points_y - 1) as f64)
        } else {
            Length::zero()
        };
        let offset_x = if nr_of_points_x > 1 {
            self.side_length.0 / 2.0
        } else {
            Length::zero()
        };
        let offset_y = if nr_of_points_y > 1 {
            self.side_length.1 / 2.0
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

#[cfg(test)]
mod test {
    use super::*;
    use uom::si::length::millimeter;
    #[test]
    fn generate_symmetric() {
        let strategy = Grid::new(
            (
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            (2, 2),
        )
        .unwrap();
        let points = strategy.generate();
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
    fn generate_size_one() {
        let strategy = Grid::new(
            (
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            (1, 1),
        )
        .unwrap();
        let points = strategy.generate();
        assert_eq!(points.len(), 1);
        assert_eq!(
            points[0],
            Point3::new(Length::zero(), Length::zero(), Length::zero())
        );
    }
    #[test]
    fn generate_asymmetric() {
        let strategy = Grid::new(
            (
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            (1, 2),
        )
        .unwrap();
        let points = strategy.generate();
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
