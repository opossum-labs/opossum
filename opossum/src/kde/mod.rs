//! Kernel density estimator

mod gaussian;
use crate::{
    error::OpmResult,
    millimeter,
    nodes::fluence_detector::Fluence,
    utils::{f64_to_usize, math_utils::distance_2d_point, usize_to_f64},
};
use gaussian::Gaussian2D;
use nalgebra::{point, DMatrix, Point2};
use num::Zero;
use rayon::prelude::*;
use std::ops::Range;
use uom::si::f64::{Area, Energy, Length};

pub struct Kde {
    hit_map: Vec<(Point2<Length>, Energy)>,
    band_width: Length,
}
impl Default for Kde {
    fn default() -> Self {
        Self {
            hit_map: Vec::default(),
            band_width: millimeter!(1.0),
        }
    }
}
impl Kde {
    pub fn set_hit_map(&mut self, hit_map: Vec<(Point2<Length>, Energy)>) {
        self.hit_map = hit_map;
    }
    pub fn set_band_width(&mut self, band_width: Length) -> OpmResult<()> {
        if !band_width.is_normal() {
            return Err(crate::error::OpossumError::Other(
                "bandwidth must be != 0.0 and finite".into(),
            ));
        }
        self.band_width = band_width;
        Ok(())
    }
    fn point_distances_std_dev(&self) -> (Vec<Length>, Length) {
        let mut sum = Length::zero();
        let mut distances = Vec::default();
        for point1 in self.hit_map.iter().enumerate() {
            for point2 in &self.hit_map[point1.0 + 1..] {
                let distance = distance_2d_point(&point1.1 .0, &point2.0);
                distances.push(distance);
                sum += distance;
            }
        }
        let nr_of_points = usize_to_f64(distances.len());
        let average_dist = sum / nr_of_points;
        let mut deviation_sum = Area::zero();
        for d in &distances {
            deviation_sum += (*d - average_dist) * (*d - average_dist);
        }
        (distances, (deviation_sum / nr_of_points).sqrt())
    }
    fn distances_iqr(values: &[Length]) -> Length {
        if values.is_empty() {
            millimeter!(f64::NAN)
        } else if values.len() == 1 {
            values[0]
        } else {
            let mut sorted_values = values.to_owned();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let quart_index = 0.75 * usize_to_f64(sorted_values.len());
            // check if quart_index effectively an integer
            if quart_index.fract().is_zero() {
                0.5 * (sorted_values[f64_to_usize(quart_index) - 1]
                    + sorted_values[f64_to_usize(quart_index)])
            } else {
                sorted_values[f64_to_usize(f64::floor(quart_index))]
            }
        }
    }
    #[must_use]
    pub fn bandwidth_estimate(&self) -> Length {
        let (distances, std_dev) = self.point_distances_std_dev();
        let iqr = Self::distances_iqr(&distances);
        // Silverman's rule of thumb
        0.9 * Length::min(std_dev, iqr / 1.34) * (usize_to_f64(self.hit_map.len())).powf(-0.2)
    }
    #[must_use]
    pub fn kde_value(&self, point: Point2<Length>) -> Fluence {
        self.hit_map
            .iter()
            .map(|hit| Gaussian2D::new(hit.0, self.band_width, hit.1).value(point))
            .sum()
    }
    #[must_use]
    pub fn kde_2d(
        &self,
        ranges: &(Range<Length>, Range<Length>),
        dimensions: (usize, usize),
    ) -> DMatrix<Fluence> {
        let dx = (ranges.0.end - ranges.0.start) / usize_to_f64(dimensions.0);
        let dy = (ranges.1.end - ranges.1.start) / usize_to_f64(dimensions.1);
        let mut matrix = DMatrix::<Fluence>::zeros(dimensions.1, dimensions.0);
        matrix
            .par_column_iter_mut()
            .enumerate()
            .for_each(|(col_idx, mut col)| {
                for point in col.iter_mut().enumerate() {
                    *point.1 = self.kde_value(point![
                        ranges.0.start + usize_to_f64(col_idx) * dx,
                        ranges.1.start + usize_to_f64(point.0) * dy
                    ]);
                }
            });
        matrix
    }
}

#[cfg(test)]
mod test {
    use super::Kde;
    use crate::{joule, meter, millimeter};
    use core::f64;
    #[test]
    fn default() {
        let kde = Kde::default();
        assert_eq!(kde.hit_map.len(), 0);
        assert_eq!(kde.band_width, millimeter!(1.0));
    }
    #[test]
    fn set_hit_map() {
        let mut kde = Kde::default();
        let hit_map = vec![(millimeter!(1.0, 2.0), joule!(3.0))];
        kde.set_hit_map(hit_map);
        assert_eq!(kde.hit_map.len(), 1);
        assert_eq!(kde.hit_map[0].0.x, millimeter!(1.0));
        assert_eq!(kde.hit_map[0].0.y, millimeter!(2.0));
        assert_eq!(kde.hit_map[0].1, joule!(3.0));
    }
    #[test]
    fn set_bandwidth() {
        let mut kde = Kde::default();
        assert!(kde.set_band_width(millimeter!(0.0)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::NAN)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::INFINITY)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::NEG_INFINITY)).is_err());
        kde.set_band_width(millimeter!(2.0)).unwrap();
        assert_eq!(kde.band_width, millimeter!(2.0));
    }
    #[test]
    fn point_distances_std_dev() {
        let mut kde = Kde::default();
        assert_eq!(kde.point_distances_std_dev().0.len(), 0);
        assert!(kde.point_distances_std_dev().1.value.is_nan());
        let hit_map = vec![(millimeter!(0.0, 0.0), joule!(0.0))];
        kde.set_hit_map(hit_map);
        assert_eq!(kde.point_distances_std_dev().0.len(), 0);
        assert!(kde.point_distances_std_dev().1.value.is_nan());
        let hit_map = vec![
            (millimeter!(0.0, 0.0), joule!(0.0)),
            (millimeter!(1.0, 0.0), joule!(0.0)),
        ];
        kde.set_hit_map(hit_map);
        assert_eq!(kde.point_distances_std_dev().0, vec![millimeter!(1.0)]);
        assert_eq!(kde.point_distances_std_dev().1, millimeter!(0.0));
        let hit_map = vec![
            (meter!(0.0, 0.0), joule!(0.0)),
            (meter!(1.0, 0.0), joule!(0.0)),
            (meter!(-1.0, 0.0), joule!(0.0)),
        ];
        kde.set_hit_map(hit_map);
        assert_eq!(
            kde.point_distances_std_dev().0,
            vec![meter!(1.0), meter!(1.0), meter!(2.0)]
        );
        assert_eq!(
            kde.point_distances_std_dev().1,
            meter!(f64::sqrt(2.0 / 9.0))
        );
    }
    #[test]
    fn distances_iqr() {
        assert!(Kde::distances_iqr(&vec![]).is_nan());
        assert_eq!(Kde::distances_iqr(&vec![meter!(1.0)]), meter!(1.0));
        assert_eq!(
            Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0)]),
            meter!(1.0)
        );
        assert_eq!(
            Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0), meter!(2.0)]),
            meter!(2.0)
        );
        assert_eq!(
            Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0), meter!(2.0), meter!(3.0)]),
            meter!(2.5)
        );
        // Example from Wikipedia
        let lengths = vec![
            meter!(25.0),
            meter!(28.0),
            meter!(4.0),
            meter!(28.0),
            meter!(19.0),
            meter!(3.0),
            meter!(9.0),
            meter!(17.0),
            meter!(29.0),
            meter!(29.0),
        ];
        assert_eq!(Kde::distances_iqr(&lengths), meter!(28.0));
    }
}
