//! Data structure for holding a fluence map.
use std::ops::Range;

use super::Fluence;
use crate::{
    J_per_cm2,
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    error::OpmResult,
    joule,
    properties::Proptype,
    utils::{griddata::linspace, to_f64},
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Energy, Length},
    length::meter,
};

impl From<FluenceData> for Proptype {
    fn from(value: FluenceData) -> Self {
        Self::FluenceData(value)
    }
}
/// Struct to hold the fluence map information of a beam
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FluenceData {
    /// peak fluence of the beam
    peak: Fluence,
    /// 2d fluence distribution of the beam.
    interp_distribution: DMatrix<Fluence>,
    /// x coordinates of the fluence distribution
    x_range: Range<Length>,
    /// y coordinates of the fluence distribution
    y_range: Range<Length>,
    /// the estimator which has been used to calculate this fluence
    estimator: FluenceEstimator,
}
impl FluenceData {
    /// Constructs a new [`FluenceData`] struct
    #[must_use]
    pub fn new(
        interp_distribution: DMatrix<Fluence>,
        x_range: Range<Length>,
        y_range: Range<Length>,
        estimator: FluenceEstimator,
    ) -> Self {
        let peak_fluence =
            interp_distribution
                .iter()
                .fold(J_per_cm2!(f64::NEG_INFINITY), |arg0, v| {
                    if v.is_finite() {
                        Fluence::max(arg0, *v)
                    } else {
                        arg0
                    }
                });
        Self {
            peak: peak_fluence,
            interp_distribution,
            x_range,
            y_range,
            estimator,
        }
    }
    /// Returns the [`FluenceEstimator`] that was used to calculate this [`FluenceData`]
    #[must_use]
    pub const fn estimator(&self) -> &FluenceEstimator {
        &self.estimator
    }

    /// Returns the interpolated distribution of this [`FluenceData`]
    #[must_use]
    pub const fn interp_distribution(&self) -> &DMatrix<Fluence> {
        &self.interp_distribution
    }

    /// Returns the x coordinate range of the fluence map.
    #[must_use]
    pub const fn x_range(&self) -> &Range<Length> {
        &self.x_range
    }

    /// Returns the y coordinate range of the fluence map.
    #[must_use]
    pub const fn y_range(&self) -> &Range<Length> {
        &self.y_range
    }
    /// Returns the fluence distribution and the corresponding x and y axes in a tuple (x, y, distribution)
    ///
    /// # Errors
    ///
    /// This function returns an error if the linear axis vector (linspace) could not be generated.
    pub fn get_fluence_distribution(
        &self,
    ) -> OpmResult<(DVector<Length>, DVector<Length>, DMatrix<Fluence>)> {
        Ok((
            linspace(
                self.x_range.start.value,
                self.x_range.end.value,
                self.interp_distribution.ncols(),
            )?
            .map(Length::new::<meter>),
            linspace(
                self.y_range.start.value,
                self.y_range.end.value,
                self.interp_distribution.nrows(),
            )?
            .map(Length::new::<meter>),
            self.interp_distribution.clone(),
        ))
    }
    /// Returns length of the x data points (columns)
    #[must_use]
    pub fn len_x(&self) -> usize {
        self.interp_distribution.ncols()
    }
    /// Returns length of the y data points (rows)
    #[must_use]
    pub fn len_y(&self) -> usize {
        self.interp_distribution.nrows()
    }
    /// Returns the shape of the interpolation distribution in pixels
    ///
    /// The order of the returned tuple is `(rows, columns)`.
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        self.interp_distribution.shape()
    }
    /// Returns the peak fluence of this [`FluenceData`].
    #[must_use]
    pub fn peak(&self) -> Fluence {
        self.peak
    }
    /// Returns the total energy of this [`FluenceData`].
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        let dx = (self.x_range.end - self.x_range.start) / to_f64(self.len_x());
        let dy = (self.y_range.end - self.y_range.start) / to_f64(self.len_y());
        let area = dx * dy;
        let mut energy = joule!(0.0);
        for fluence in &self.interp_distribution {
            if !fluence.is_nan() {
                energy += area * (*fluence);
            }
        }
        energy
    }
}

#[cfg(test)]
mod test {
    use super::FluenceData;
    use crate::{
        J_per_cm2, J_per_m2, core_optics::hit_map::fluence_estimator::FluenceEstimator,
        error::OpmResult, joule, meter, properties::Proptype,
    };
    use assert_matches::assert_matches;
    use nalgebra::{dmatrix, vector};
    #[test]
    fn into_proptype() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0);
                J_per_cm2!(3.0), J_per_cm2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::default(),
        );
        let proptype: Proptype = fluence_data.into();
        assert_matches!(proptype, Proptype::FluenceData(_));
    }
    #[test]
    fn estimator() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0);
                J_per_cm2!(3.0), J_per_cm2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        assert_eq!(fluence_data.estimator(), &FluenceEstimator::Binning);
    }
    #[test]
    fn get_fluence_distribution() -> OpmResult<()> {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0);
                J_per_cm2!(3.0), J_per_cm2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        let (x, y, distribution) = fluence_data.get_fluence_distribution()?;
        assert_eq!(x, vector![meter!(0.0), meter!(1.0)]);
        assert_eq!(y, vector![meter!(0.0), meter!(1.0)]);
        assert_eq!(
            distribution,
            dmatrix![
            J_per_cm2!(1.0), J_per_cm2!(2.0);
            J_per_cm2!(3.0), J_per_cm2!(4.0)]
        );
        Ok(())
    }
    #[test]
    fn len_x_y() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0), J_per_cm2!(3.0);
                J_per_cm2!(4.0), J_per_cm2!(5.0), J_per_cm2!(6.0);],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        assert_eq!(fluence_data.len_x(), 3);
        assert_eq!(fluence_data.len_y(), 2);
    }
    #[test]
    fn shape() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0), J_per_cm2!(3.0);
                J_per_cm2!(4.0), J_per_cm2!(5.0), J_per_cm2!(6.0);],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        assert_eq!(fluence_data.shape(), (2, 3));
    }
    #[test]
    fn peak() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0);
                J_per_cm2!(3.0), J_per_cm2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        assert_eq!(fluence_data.peak(), J_per_cm2!(4.0));
    }
    #[test]
    fn total_energy() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_m2!(f64::NAN), J_per_m2!(8.0);
                J_per_m2!(8.0), J_per_m2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        assert_eq!(fluence_data.total_energy(), joule!(5.0));
    }
}
