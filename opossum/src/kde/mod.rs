//! Kernel density estimator

mod gaussian;
use crate::{millimeter, nodes::fluence_detector::Fluence};
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
        let nr_of_points = distances.len() as f64;
        let average_dist = sum / nr_of_points;
        let mut deviation_sum = Area::zero();
        for d in &distances {
            deviation_sum += (*d - average_dist) * (*d - average_dist);
        }
        (distances, (deviation_sum / nr_of_points).sqrt())
    }
    fn distances_iqr(values: &[Length]) -> Length {
        if values.is_empty() {
            millimeter!(0.0)
        } else {
            let mut sorted_values = values.to_owned();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = sorted_values.len() / 2;
            let lower_mid = sorted_values[..mid].len() / 2;
            let upper_mid = sorted_values[mid..].len() / 2;
            let lower_median = sorted_values[..mid][lower_mid];
            let upper_median = sorted_values[mid..][upper_mid];
            upper_median - lower_median
        }
    }
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn bandwidth_estimate(&self) -> Length {
        let (distances, std_dev) = self.point_distances_std_dev();
        let iqr = Self::distances_iqr(&distances);
        // Silverman's rule of thumb
        0.9 * Length::min(std_dev, iqr / 1.34) * (self.hit_map.len() as f64).powf(-0.2)
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
        #[allow(clippy::cast_precision_loss)]
        let dx = (ranges.0.end - ranges.0.start) / (dimensions.0 as f64);
        #[allow(clippy::cast_precision_loss)]
        let dy = (ranges.1.end - ranges.1.start) / (dimensions.1 as f64);
        let mut matrix = DMatrix::<Fluence>::zeros(dimensions.1, dimensions.0);
        matrix
            .par_column_iter_mut()
            .enumerate()
            .for_each(|(col_idx, mut col)| {
                #[allow(clippy::cast_precision_loss)]
                for point in col.iter_mut().enumerate() {
                    *point.1 = self.kde_value(point![
                        ranges.0.start + (col_idx as f64) * dx,
                        ranges.1.start + (point.0 as f64) * dy
                    ]);
                }
            });
        matrix
    }

    // pub fn kde_2d_2(&self,
    //     ranges: &(Range<Length>, Range<Length>),
    //     dimensions: (usize, usize),) -> DMatrix<Fluence>{

    //     #[allow(clippy::cast_precision_loss)]
    //     let dx = (ranges.0.end - ranges.0.start) / (dimensions.0 as f64);
    //     #[allow(clippy::cast_precision_loss)]
    //     let dy = (ranges.1.end - ranges.1.start) / (dimensions.1 as f64);

    //     let num_interp = 150;
    //     let mut spline_vec = Vec::<Key<f64, f64>>::with_capacity(num_interp);
    //     let bin_size = 10.*self.band_width.value/(num_interp-1).to_f64().unwrap();
    //     let norm_fac = 1./ (2.*std::f64::consts::PI*self.band_width.value * self.band_width.value);
    //     for i in 0..num_interp{
    //         let r = bin_size*i.to_f64().unwrap();
    //         let r_squared = r*r;
    //         let g = (-0.5*r_squared/(self.band_width.value*self.band_width.value)).exp()*norm_fac;
    //         spline_vec.push(Key::new(r_squared, g, Interpolation::Linear))
    //     }

    //     let spline = Spline::from_vec(spline_vec);
    //     let mut matrix = DMatrix::<Fluence>::zeros(dimensions.1, dimensions.0);

    //     matrix.as_mut_slice()
    //     .par_chunks_mut(dimensions.0)
    //     .enumerate()
    //     .for_each(|(y_i, tile)| {
    //         let y = ranges.1.start + y_i.to_f64().unwrap()*dy;
    //         for x_i in 0..dimensions.0{
    //             let x = ranges.0.start + x_i.to_f64().unwrap()*dx;
    //             for (hitp, e) in &self.hit_map {
    //                 let dist_squared = (x.value-hitp.x.value)*(x.value-hitp.x.value) + (y.value-hitp.y.value)*(y.value-hitp.y.value);
    //                 if let Some(val) = spline.sample(dist_squared){
    //                     tile[x_i] += ArealNumberDensity::new::<per_square_meter>(val)**e;
    //                 }
    //             }
    //         }
    //     }
    //     );
    //     matrix
    // }
}
