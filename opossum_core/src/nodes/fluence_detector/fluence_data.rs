//! Data structure for holding a fluence map.
use std::ops::Range;

use super::{Fluence, fluence_grid::FluenceGrid};
use crate::{
    J_per_cm2, core_optics::hit_map::fluence_estimator::FluenceEstimator, joule,
    properties::Proptype, types::SiScalarSchema,
};
use nalgebra::{DMatrix, DVector};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Energy, Length};

impl From<FluenceData> for Proptype {
    fn from(value: FluenceData) -> Self {
        Self::FluenceData(value)
    }
}
/// Struct to hold the fluence map information of a beam
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FluenceData {
    /// peak fluence of the beam
    #[schemars(with = "SiScalarSchema")]
    peak: Fluence,
    /// the fluence of the beam at each point of the map
    grid: FluenceGrid,
    /// the estimator which has been used to calculate this fluence
    estimator: FluenceEstimator,
    /// for a map that follows ray tubes, the energy it draws in the tubes and in the margins at the
    /// edges of the light (see [`FluenceData::with_drawn_energy`])
    #[serde(default)]
    #[schemars(with = "Option<(SiScalarSchema, SiScalarSchema)>")]
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
        Self {
            peak: peak_of(interp_distribution.iter().copied()),
            grid: FluenceGrid::with_values(x_range, y_range, interp_distribution),
            estimator,
            drawn_energy: None,
            deviation: None,
        }
    }
    /// Returns this [`FluenceData`] with the estimated deviation of the fluence from it at each
    /// of its points, from the curvature of the fluence between the rays: positive where the
    /// fluence lies above the map (see the handbook, "Fluence uncertainty").
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
            self.x_range().clone(),
            self.y_range().clone(),
            self.estimator,
        ))
    }
    /// Returns how far the peak of the fluence may lie above the peak of this [`FluenceData`]:
    /// the highest value of the map plus its estimated deviation, less the peak of the map, and
    /// zero if that is negative (see the handbook, "Fluence uncertainty", for when the peak of the
    /// map cannot lie above that of the fluence). `None` for a map that does not follow ray tubes.
    #[must_use]
    pub fn peak_uncertainty(&self) -> Option<Fluence> {
        let deviation = self.deviation.as_ref()?;
        let corrected_peak = peak_of(
            self.interp_distribution()
                .iter()
                .zip(deviation.iter())
                .map(|(fluence, deviation)| *fluence + *deviation),
        );
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

    /// Returns the grid of points of this [`FluenceData`], with the fluence at each.
    #[must_use]
    pub const fn grid(&self) -> &FluenceGrid {
        &self.grid
    }
    /// Returns the interpolated distribution of this [`FluenceData`]
    #[must_use]
    pub const fn interp_distribution(&self) -> &DMatrix<Fluence> {
        self.grid.values()
    }

    /// Returns the x coordinate range of the fluence map.
    #[must_use]
    pub const fn x_range(&self) -> &Range<Length> {
        self.grid.x_range()
    }

    /// Returns the y coordinate range of the fluence map.
    #[must_use]
    pub const fn y_range(&self) -> &Range<Length> {
        self.grid.y_range()
    }
    /// Returns the fluence distribution and the corresponding x and y axes in a tuple (x, y, distribution)
    #[must_use]
    pub fn get_fluence_distribution(&self) -> (DVector<Length>, DVector<Length>, DMatrix<Fluence>) {
        let (x, y) = self.grid.points();
        (x, y, self.interp_distribution().clone())
    }
    /// Returns length of the x data points (columns)
    #[must_use]
    pub fn len_x(&self) -> usize {
        self.interp_distribution().ncols()
    }
    /// Returns length of the y data points (rows)
    #[must_use]
    pub fn len_y(&self) -> usize {
        self.interp_distribution().nrows()
    }
    /// Returns the shape of the interpolation distribution in pixels
    ///
    /// The order of the returned tuple is `(rows, columns)`.
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        self.interp_distribution().shape()
    }
    /// Returns the peak fluence of this [`FluenceData`].
    #[must_use]
    pub fn peak(&self) -> Fluence {
        self.peak
    }
    /// Returns the total energy of this [`FluenceData`]: the fluence at each point times the area
    /// it stands for (see [`FluenceGrid::point_areas`]), leaving out points without a value.
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        self.grid
            .point_areas()
            .zip_map(self.interp_distribution(), |area, fluence| {
                if fluence.is_nan() {
                    joule!(0.0)
                } else {
                    area * fluence
                }
            })
            .iter()
            .fold(joule!(0.0), |total, energy| total + *energy)
    }
}
/// The highest of the given fluences that is finite, minus infinity if none is.
fn peak_of(fluences: impl Iterator<Item = Fluence>) -> Fluence {
    fluences
        .filter(|fluence| fluence.is_finite())
        .fold(J_per_cm2!(f64::NEG_INFINITY), Fluence::max)
}

#[cfg(test)]
mod test {
    use super::FluenceData;
    use crate::{
        J_per_cm2, J_per_m2, core_optics::hit_map::fluence_estimator::FluenceEstimator, joule,
        meter, properties::Proptype,
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
    fn get_fluence_distribution() {
        let fluence_data = FluenceData::new(
            dmatrix![
                J_per_cm2!(1.0), J_per_cm2!(2.0);
                J_per_cm2!(3.0), J_per_cm2!(4.0)],
            meter!(0.0)..meter!(1.0),
            meter!(0.0)..meter!(1.0),
            FluenceEstimator::Binning,
        );
        let (x, y, distribution) = fluence_data.get_fluence_distribution();
        assert_eq!(x, vector![meter!(0.0), meter!(1.0)]);
        assert_eq!(y, vector![meter!(0.0), meter!(1.0)]);
        assert_eq!(
            distribution,
            dmatrix![
            J_per_cm2!(1.0), J_per_cm2!(2.0);
            J_per_cm2!(3.0), J_per_cm2!(4.0)]
        );
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
