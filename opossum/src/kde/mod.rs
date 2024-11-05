//! Kernel density estimator

mod gaussian;

use crate::nodes::fluence_detector::Fluence;
use crate::{millimeter, J_per_cm2};
use gaussian::Gaussian2D;
use nalgebra::{point, DMatrix, Point2};
use num::Zero;
use std::ops::Range;
use uom::si::f64::{Area, Energy, Length, Ratio};
use uom::si::ratio::ratio;

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
    pub fn set_band_width(&mut self, band_width: Length) {
        self.band_width = band_width;
    }
    fn distance(point1: &Point2<Length>, point2: &Point2<Length>) -> Length {
        ((point1.x - point2.x) * (point1.x - point2.x)
            + (point1.y - point2.y) * (point1.y - point2.y))
            .sqrt()
    }
    fn point_distances_std_dev(&self) -> (Vec<Length>, Length) {
        let mut sum = Length::zero();
        let mut distances = Vec::default();
        for point1 in self.hit_map.iter().enumerate() {
            for point2 in &self.hit_map[point1.0 + 1..] {
                let distance = Self::distance(&point1.1 .0, &point2.0);
                distances.push(distance);
                sum += distance;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let nr_of_points = Ratio::new::<uom::si::ratio::ratio>(distances.len() as f64);
        let average_dist = sum / nr_of_points;
        let mut deviation_sum = Area::zero();
        for d in &distances {
            deviation_sum += (*d - average_dist) * (*d - average_dist);
        }
        (distances, (deviation_sum / nr_of_points).sqrt())
    }
    fn distances_iqr(values: &[Length]) -> Length {
        let mut sorted_values = values.to_owned();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted_values.len() / 2;
        let lower_mid = sorted_values[..mid].len() / 2;
        let upper_mid = sorted_values[mid..].len() / 2;
        let lower_median = sorted_values[..mid][lower_mid];
        let upper_median = sorted_values[mid..][upper_mid];
        upper_median - lower_median
    }
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn bandwidth_estimate(&self) -> Length {
        let (distances, std_dev) = self.point_distances_std_dev();
        let iqr = Self::distances_iqr(&distances);
        // Silverman's rule of thumb
        Ratio::new::<ratio>(0.9)
            * Length::min(std_dev, iqr / Ratio::new::<ratio>(1.34))
            * Ratio::new::<ratio>((self.hit_map.len() as f64).powf(-0.2))
    }
    #[must_use]
    pub fn kde_value(&self, point: Point2<Length>) -> Fluence {
        let mut value = J_per_cm2!(0.0);
        for hit in &self.hit_map {
            let gaussian = Gaussian2D::new(hit.0, self.band_width, hit.1);
            value += gaussian.value(point);
        }
        value
    }
    #[must_use]
    pub fn kde_2d(
        &self,
        ranges: &(Range<Length>, Range<Length>),
        dimensions: (usize, usize),
    ) -> DMatrix<Fluence> {
        #[allow(clippy::cast_precision_loss)]
        let dx = (ranges.0.end - ranges.0.start) / (dimensions.0 as f64);
        #[allow(clippy::cast_precision_loss)]
        let dy = (ranges.1.end - ranges.1.start) / (dimensions.1 as f64);
        let mut matrix = DMatrix::<Fluence>::zeros(dimensions.1, dimensions.0);

        let mut x = ranges.0.start;
        for x_i in 0..dimensions.0 {
            let mut y = ranges.1.start;
            for y_i in 0..dimensions.1 {
                matrix[(y_i, x_i)] = self.kde_value(point![x, y]);
                y += dy;
            }
            x += dx;
        }
        matrix
    }
}
