//! Kernel density estimator

mod gaussian;

use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
    nodes::fluence_detector::Fluence,
    utils::to_f64,
};
use gaussian::Gaussian2D;
use nalgebra::{DMatrix, Point2, point};
use num::Zero;
use rayon::prelude::*;
use std::ops::Range;
use uom::si::f64::{Area, Energy, Length};

pub struct Kde {
    band_width: Length,
}

impl Default for Kde {
    fn default() -> Self {
        Self {
            band_width: millimeter!(1.0),
        }
    }
}

impl Kde {
    /// Sets the band width of this [`Kde`].
    pub fn set_band_width(&mut self, band_width: Length) -> OpmResult<()> {
        if !band_width.is_normal() {
            return Err(OpossumError::Other(
                "bandwidth must be != 0.0 and finite".into(),
            ));
        }
        self.band_width = band_width;
        Ok(())
    }

    /// Calculates the standard deviation of the positions directly (O(N)).
    /// Replacing the memory-heavy pairwise distance calculation (O(N^2)).
    fn std_dev_of_positions<T, F>(data: &[T], accessor: &F) -> Length
    where
        F: Fn(&T) -> Point2<Length> + Sync,
    {
        if data.len() < 2 {
            return Length::zero();
        }
        let n = to_f64(data.len());

        // Calculate Mean Center
        let mut sum_x = Length::zero();
        let mut sum_y = Length::zero();
        for item in data {
            let p = accessor(item);
            sum_x += p.x;
            sum_y += p.y;
        }
        let mean_x = sum_x / n;
        let mean_y = sum_y / n;

        // Calculate Variance (distance from mean center)
        let mut sum_sq_diff = Area::zero();
        for item in data {
            let p = accessor(item);
            let diff_x = p.x - mean_x;
            let diff_y = p.y - mean_y;
            sum_sq_diff += diff_x * diff_x + diff_y * diff_y;
        }

        // Return combined standard deviation (rough estimate for 2D bandwidth)
        // Dividing by 2 to average the X and Y variance contributions
        (sum_sq_diff / (2.0 * n)).sqrt()
    }

    /// Estimates bandwidth using a faster, O(N) approximation of Silverman's Rule.
    /// Eliminates the O(N^2) memory crash.
    #[must_use]
    pub fn bandwidth_estimate<T, F>(data: &[T], accessor: F) -> Length
    where
        F: Fn(&T) -> Point2<Length> + Sync,
    {
        match data.len() {
            0 | 1 => millimeter!(f64::NAN),
            _ => {
                // Use Standard Deviation of positions (O(N)) instead of pairwise (O(N^2))
                let std_dev = Self::std_dev_of_positions(data, &accessor);

                // Simplified Silverman's rule for 2D without IQR calculation (which requires sorting)
                // If you strictly need IQR, you can calculate it on X and Y coords separately.
                // For performance/memory safety on massive datasets, StdDev is usually robust enough.

                if std_dev.value == 0.0 {
                    millimeter!(1.0) // Fallback
                } else {
                    // Silverman's factor for 2D is often cited around n^(-1/6) or n^(-1/5)
                    // Staying close to your original formula structure:
                    0.9 * std_dev * (to_f64(data.len())).powf(-0.2)
                }
            }
        }
    }

    #[must_use]
    pub fn kde_value<T, F>(&self, data: &[T], accessor: &F, point: Point2<Length>) -> Fluence
    where
        F: Fn(&T) -> (Point2<Length>, Energy) + Sync,
    {
        // Zero-Allocation: Iterating directly over the reference slice
        data.iter()
            .map(|item| {
                let (pos, energy) = accessor(item);
                Gaussian2D::new(pos, self.band_width, energy).value(point)
            })
            .sum()
    }

    #[must_use]
    pub fn kde_2d<T, F>(
        &self,
        data: &[T],  // GENERIC: Reference to original data
        accessor: F, // GENERIC: How to get (x,y,E) from T
        ranges: &(Range<Length>, Range<Length>),
        dimensions: (usize, usize),
    ) -> DMatrix<Fluence>
    where
        T: Sync,
        F: Fn(&T) -> (Point2<Length>, Energy) + Sync + Send + Copy,
    {
        let dx = (ranges.0.end - ranges.0.start) / to_f64(dimensions.0);
        let dy = (ranges.1.end - ranges.1.start) / to_f64(dimensions.1);
        let mut matrix = DMatrix::<Fluence>::zeros(dimensions.1, dimensions.0);

        matrix
            .par_column_iter_mut()
            .enumerate()
            .for_each(|(col_idx, mut col)| {
                for point in col.iter_mut().enumerate() {
                    let eval_point = point![
                        ranges.0.start + to_f64(col_idx) * dx,
                        ranges.1.start + to_f64(point.0) * dy
                    ];
                    // Pass reference and accessor down
                    *point.1 = self.kde_value(data, &accessor, eval_point);
                }
            });
        matrix
    }
}

#[cfg(test)]
mod test {
    // use approx::assert_abs_diff_eq;
    use super::Kde;
    // use crate::{joule, meter, millimeter};
    use crate::{error::OpmResult, millimeter};
    use core::f64;
    #[test]
    fn default() {
        let kde = Kde::default();
        assert_eq!(kde.band_width, millimeter!(1.0));
    }
    #[test]
    fn set_bandwidth() -> OpmResult<()> {
        let mut kde = Kde::default();
        assert!(kde.set_band_width(millimeter!(0.0)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::NAN)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::INFINITY)).is_err());
        assert!(kde.set_band_width(millimeter!(f64::NEG_INFINITY)).is_err());
        kde.set_band_width(millimeter!(2.0))?;
        assert_eq!(kde.band_width, millimeter!(2.0));
        Ok(())
    }
    // #[test]
    // fn point_distances_std_dev() {
    //     let mut kde = Kde::default();
    //     assert_eq!(kde.point_distances_std_dev().0.len(), 0);
    //     assert!(kde.point_distances_std_dev().1.value.is_nan());
    //     let hit_map = vec![(millimeter!(0.0, 0.0), joule!(0.0))];
    //     kde.set_hit_map(hit_map);
    //     assert_eq!(kde.point_distances_std_dev().0.len(), 0);
    //     assert!(kde.point_distances_std_dev().1.value.is_nan());
    //     let hit_map = vec![
    //         (millimeter!(0.0, 0.0), joule!(0.0)),
    //         (millimeter!(1.0, 0.0), joule!(0.0)),
    //     ];
    //     kde.set_hit_map(hit_map);
    //     assert_eq!(kde.point_distances_std_dev().0, vec![millimeter!(1.0)]);
    //     assert_eq!(kde.point_distances_std_dev().1, millimeter!(0.0));
    //     let hit_map = vec![
    //         (meter!(0.0, 0.0), joule!(0.0)),
    //         (meter!(1.0, 0.0), joule!(0.0)),
    //         (meter!(-1.0, 0.0), joule!(0.0)),
    //     ];
    //     kde.set_hit_map(hit_map);
    //     assert_eq!(
    //         kde.point_distances_std_dev().0,
    //         vec![meter!(1.0), meter!(1.0), meter!(2.0)]
    //     );
    //     assert_eq!(
    //         kde.point_distances_std_dev().1,
    //         meter!(f64::sqrt(2.0 / 9.0))
    //     );
    // }
    // #[test]
    // fn distances_iqr() {
    //     assert!(Kde::distances_iqr(&vec![]).is_nan());
    //     assert_eq!(Kde::distances_iqr(&vec![meter!(1.0)]), meter!(1.0));
    //     assert_eq!(
    //         Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0)]),
    //         meter!(1.0)
    //     );
    //     assert_eq!(
    //         Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0), meter!(2.0)]),
    //         meter!(2.0)
    //     );
    //     assert_eq!(
    //         Kde::distances_iqr(&vec![meter!(0.0), meter!(1.0), meter!(2.0), meter!(3.0)]),
    //         meter!(2.5)
    //     );
    //     // Example from Wikipedia
    //     let lengths = vec![
    //         meter!(25.0),
    //         meter!(28.0),
    //         meter!(4.0),
    //         meter!(28.0),
    //         meter!(19.0),
    //         meter!(3.0),
    //         meter!(9.0),
    //         meter!(17.0),
    //         meter!(29.0),
    //         meter!(29.0),
    //     ];
    //     assert_eq!(Kde::distances_iqr(&lengths), meter!(28.0));
    // }
    // #[test]
    // fn bandwidth_estimate() {
    //     let mut kde = Kde::default();
    //     assert!(kde.bandwidth_estimate().is_nan());
    //     let hit_map = vec![(millimeter!(0.0, 0.0), joule!(0.0))];
    //     kde.set_hit_map(hit_map);
    //     assert!(kde.bandwidth_estimate().is_nan());
    //     let hit_map = vec![
    //         (millimeter!(0.0, 0.0), joule!(0.0)),
    //         (millimeter!(1.0, 0.0), joule!(0.0)),
    //     ];
    //     kde.set_hit_map(hit_map);
    //     assert_eq!(kde.bandwidth_estimate(), millimeter!(0.5));
    //     let hit_map = vec![
    //         (millimeter!(0.0, 0.0), joule!(0.0)),
    //         (millimeter!(1.0, 0.0), joule!(0.0)),
    //         (millimeter!(-1.0, 0.0), joule!(0.0)),
    //     ];
    //     kde.set_hit_map(hit_map);
    //     assert_abs_diff_eq!(kde.bandwidth_estimate().value, 0.00034057440111656337);
    // }
}
