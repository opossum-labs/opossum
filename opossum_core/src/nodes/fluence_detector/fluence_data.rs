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
    f64::{Area, Energy, Length},
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
    /// for a map that follows ray tubes, the energy it draws in the tubes and in the margins at the
    /// edges of the light (see [`FluenceData::with_drawn_energy`])
    #[serde(default)]
    drawn_energy: Option<(Energy, Energy)>,
    /// for a map that follows ray tubes, the estimated deviation of the fluence from the map at
    /// each of its points (see [`FluenceData::with_deviation`]); derived for the report, not
    /// written to file
    #[serde(skip)]
    deviation: Option<DMatrix<Fluence>>,
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
            drawn_energy: None,
            deviation: None,
        }
    }
    /// Returns this [`FluenceData`] with the estimated deviation of the fluence from it at each
    /// of its points, from the curvature of the fluence between the rays: positive where the
    /// fluence lies above the map (see the handbook, "Uncertainty").
    ///
    /// # Arguments
    ///
    /// * `deviation` - the deviation, with the shape of the map.
    #[must_use]
    pub fn with_deviation(mut self, deviation: DMatrix<Fluence>) -> Self {
        self.deviation = Some(deviation);
        self
    }
    /// Returns the estimated deviation of the fluence from this [`FluenceData`] at each of its
    /// points (see [`FluenceData::with_deviation`]), `None` for a map that does not follow ray
    /// tubes.
    #[must_use]
    pub const fn deviation(&self) -> Option<&DMatrix<Fluence>> {
        self.deviation.as_ref()
    }
    /// Returns a map of how far the fluence may lie from this [`FluenceData`] at each of its
    /// points: the size of the estimated deviation (see [`FluenceData::with_deviation`]), `None`
    /// for a map that does not follow ray tubes.
    #[must_use]
    pub fn uncertainty_map(&self) -> Option<Self> {
        let deviation = self.deviation.as_ref()?;
        Some(Self::new(
            deviation.map(Fluence::abs),
            self.x_range.clone(),
            self.y_range.clone(),
            self.estimator,
        ))
    }
    /// Returns how far the peak of the fluence may lie above the peak of this [`FluenceData`]:
    /// the highest value of the map plus its estimated deviation, less the peak of the map, and
    /// zero if that is negative (see the handbook, "Uncertainty", for when the peak of the map
    /// cannot lie above that of the fluence). `None` for a map that does not follow ray tubes.
    #[must_use]
    pub fn peak_uncertainty(&self) -> Option<Fluence> {
        let deviation = self.deviation.as_ref()?;
        let corrected_peak = self
            .interp_distribution
            .zip_map(deviation, |fluence, deviation| fluence + deviation)
            .iter()
            .filter(|fluence| fluence.is_finite())
            .fold(J_per_cm2!(f64::NEG_INFINITY), |peak, fluence| {
                peak.max(*fluence)
            });
        Some((corrected_peak - self.peak).max(J_per_cm2!(0.0)))
    }
    /// Returns this [`FluenceData`] with the energy it draws, summed over its triangles exactly:
    /// in the ray tubes, and in the margins at the edges of the light, which a map following ray
    /// tubes places only to within half a tube (see the handbook, "Fluence"). It is the energy of
    /// all the triangles, also where a given range of the map cuts them off.
    ///
    /// # Arguments
    ///
    /// * `in_tubes` - the energy in the ray tubes.
    /// * `in_margins` - the energy in the margins.
    #[must_use]
    pub const fn with_drawn_energy(mut self, in_tubes: Energy, in_margins: Energy) -> Self {
        self.drawn_energy = Some((in_tubes, in_margins));
        self
    }
    /// Returns the energy this [`FluenceData`] draws, summed over its triangles exactly (see
    /// [`FluenceData::with_drawn_energy`]), `None` for a map that does not follow ray tubes.
    #[must_use]
    pub fn drawn_energy(&self) -> Option<Energy> {
        self.drawn_energy
            .map(|(in_tubes, in_margins)| in_tubes + in_margins)
    }
    /// Returns the share of the energy of this [`FluenceData`] in the margins at the edges of the
    /// light, `None` for a map that does not follow ray tubes or draws no energy.
    #[must_use]
    pub fn edge_share(&self) -> Option<f64> {
        let (in_tubes, in_margins) = self.drawn_energy?;
        let total = in_tubes + in_margins;
        (total.value > 0.0).then(|| (in_margins / total).value)
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
    /// Returns the total energy of this [`FluenceData`]: the fluence at each point times the area
    /// it stands for (see [`FluenceData::point_areas`]), leaving out points without a value.
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        Self::point_areas(&self.x_range, &self.y_range, (self.len_x(), self.len_y()))
            .zip_map(&self.interp_distribution, |area, fluence| {
                if fluence.is_nan() {
                    joule!(0.0)
                } else {
                    area * fluence
                }
            })
            .iter()
            .fold(joule!(0.0), |total, energy| total + *energy)
    }
    /// Returns the area each point of a map stands for, a row per point along y.
    ///
    /// The points of a map lie evenly from one end of each range to the other, both included. Each
    /// stands for a cell as large as their spacing, cut at the border of the map: a point on an
    /// edge for half of one, a point in a corner for a quarter (the trapezoid rule).
    ///
    /// # Arguments
    ///
    /// * `x`, `y` - the ranges the map covers.
    /// * `nr_of_points` - the number of points along x and y; a single point stands for its whole
    ///   range.
    #[must_use]
    pub fn point_areas(
        x: &Range<Length>,
        y: &Range<Length>,
        (columns, rows): (usize, usize),
    ) -> DMatrix<Area> {
        let spacing = |range: &Range<Length>, points: usize| {
            (range.end - range.start) / to_f64(points.saturating_sub(1).max(1))
        };
        let share = |index: usize, points: usize| {
            if points > 1 && (index == 0 || index + 1 == points) {
                0.5
            } else {
                1.0
            }
        };
        let cell = spacing(x, columns) * spacing(y, rows);
        DMatrix::from_fn(rows, columns, |row, column| {
            cell * share(row, rows) * share(column, columns)
        })
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
    /// A point inside a map stands for a whole cell, one on an edge for half of it, one in a
    /// corner for a quarter, and a single point along an axis for its whole range.
    #[test]
    fn the_points_of_a_map_stand_for_the_cells_around_them() {
        let areas = FluenceData::point_areas(
            &(meter!(0.0)..meter!(4.0)),
            &(meter!(0.0)..meter!(2.0)),
            (5, 3),
        )
        .map(|area| area.value);
        assert_eq!(
            areas,
            dmatrix![
                0.25, 0.5, 0.5, 0.5, 0.25;
                0.5, 1.0, 1.0, 1.0, 0.5;
                0.25, 0.5, 0.5, 0.5, 0.25]
        );
        let single = FluenceData::point_areas(
            &(meter!(0.0)..meter!(4.0)),
            &(meter!(0.0)..meter!(2.0)),
            (1, 1),
        );
        assert_eq!(single[(0, 0)].value, 8.0);
    }
    /// The points of a map lie 1 m apart over 2 m: the one in the middle stands for 1 m², so its
    /// 1 J/m² is 1 J.
    #[test]
    fn a_map_holds_the_fluence_of_each_point_over_the_area_it_stands_for() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_m2!(0.0), J_per_m2!(0.0), J_per_m2!(0.0);
                J_per_m2!(0.0), J_per_m2!(1.0), J_per_m2!(0.0);
                J_per_m2!(0.0), J_per_m2!(0.0), J_per_m2!(0.0)],
            meter!(0.0)..meter!(2.0),
            meter!(0.0)..meter!(2.0),
            FluenceEstimator::Voronoi,
        );
        approx::assert_relative_eq!(fluence_data.total_energy().value, 1.0);
    }
    /// A map tells the energy it draws and the share of it in the margins, and a map that draws
    /// none tells no share.
    #[test]
    fn a_map_tells_its_drawn_energy_and_the_share_in_its_margins() {
        let map = || {
            FluenceData::new(
                dmatrix![J_per_cm2!(0.0)],
                meter!(0.0)..meter!(1.0),
                meter!(0.0)..meter!(1.0),
                FluenceEstimator::Voronoi,
            )
        };
        assert!(map().drawn_energy().is_none() && map().edge_share().is_none());
        let drawn = map().with_drawn_energy(joule!(3.0), joule!(1.0));
        assert_eq!(drawn.drawn_energy(), Some(joule!(4.0)));
        approx::assert_relative_eq!(drawn.edge_share().unwrap_or(f64::NAN), 0.25);
        let empty = map().with_drawn_energy(joule!(0.0), joule!(0.0));
        assert!(empty.edge_share().is_none());
    }
    /// The uncertainty of a map is the size of its estimated deviation at each point, and its
    /// peak may lie as far above the peak of the map as the map plus the deviation reaches beyond
    /// it, but not below.
    #[test]
    fn a_map_tells_its_uncertainty_from_its_deviation() {
        let map = || {
            FluenceData::new(
                dmatrix![
                    J_per_cm2!(1.0), J_per_cm2!(2.0);
                    J_per_cm2!(3.0), J_per_cm2!(4.0)],
                meter!(0.0)..meter!(1.0),
                meter!(0.0)..meter!(1.0),
                FluenceEstimator::Voronoi,
            )
        };
        assert!(map().uncertainty_map().is_none() && map().peak_uncertainty().is_none());
        let deviating = map().with_deviation(dmatrix![
            J_per_cm2!(0.0), J_per_cm2!(-0.5);
            J_per_cm2!(1.5), J_per_cm2!(0.25)]);
        let uncertainty = deviating.uncertainty_map().expect("an uncertainty map");
        assert_eq!(uncertainty.interp_distribution()[(0, 1)], J_per_cm2!(0.5));
        assert_eq!(uncertainty.peak(), J_per_cm2!(1.5));
        // 3 + 1.5 J/cm² reach 0.5 J/cm² beyond the peak of 4 J/cm²
        approx::assert_relative_eq!(
            deviating
                .peak_uncertainty()
                .expect("a peak uncertainty")
                .value,
            J_per_cm2!(0.5).value
        );
        let lower = map().with_deviation(dmatrix![
            J_per_cm2!(0.0), J_per_cm2!(0.0);
            J_per_cm2!(0.0), J_per_cm2!(-1.0)]);
        assert_eq!(lower.peak_uncertainty(), Some(J_per_cm2!(0.0)));
    }
}
