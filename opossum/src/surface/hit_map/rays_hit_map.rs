//! The structures for storing the actual hitmap.
//!
//! This module also conatins the routines for genearating a fluence map using different estimator strategies.
use log::warn;
use nalgebra::{DMatrix, DVector, MatrixXx2, Point2, Point3};
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Area, Energy, Length},
    length,
};

use crate::{
    centimeter,
    error::{OpmResult, OpossumError},
    kde::Kde,
    nodes::fluence_detector::{fluence_data::FluenceData, Fluence},
    plottable::AxLims,
    utils::{
        f64_to_usize,
        griddata::{
            calc_closed_poly_area, create_voronoi_cells, interpolate_3d_triangulated_scatter_data,
            linspace, VoronoiedData,
        },
        usize_to_f64,
    },
    J_per_cm2,
};

use super::FluenceEstimator;

/// A hit point as part of a [`RaysHitMap`].
///
/// It stores the position (intersection point) and the energy of a [`Ray`](crate::ray::Ray) that
/// has hit an [`OpticSurface`](crate::surface::optic_surface::OpticSurface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitPoint {
    /// position of the intersection point
    pub position: Point3<Length>,
    /// energy of the ray that intersected the surface
    pub energy: Energy,
}
impl HitPoint {
    /// Create a new [`HitPoint`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given energy is negative or not finite.
    ///   - the position coordinates (x/y/z) are not finite.
    pub fn new(position: Point3<Length>, energy: Energy) -> OpmResult<Self> {
        if !energy.is_finite() | energy.is_sign_negative() {
            return Err(OpossumError::Other(
                "energy must be positive and finite".into(),
            ));
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other("position must be finite".into()));
        }
        Ok(Self { position, energy })
    }
    /// Returns the position of this [`HitPoint`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the energy of this [`HitPoint`].
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.energy
    }
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
///Storage struct for hitpoints on a surface from a single ray bundle
pub struct RaysHitMap {
    hit_map: Vec<HitPoint>,
}
impl RaysHitMap {
    /// Creates a new [`RaysHitMap`]
    #[must_use]
    pub fn new(hit_points: &[HitPoint]) -> Self {
        let mut hps = Vec::with_capacity(hit_points.len());
        for hit_point in hit_points {
            hps.push(hit_point.clone());
        }
        Self { hit_map: hps }
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    pub fn add_hit_point(&mut self, hit_point: HitPoint) {
        self.hit_map.push(hit_point);
    }
    /// Merge this [`RaysHitMap`] with another [`RaysHitMap`].
    pub fn merge(&mut self, other_map: &Self) {
        for hit_point in &other_map.hit_map {
            self.add_hit_point(hit_point.to_owned());
        }
    }
    /// Calculate a fluence map from this [`RaysHitMap`] using a simple binning of th hit points.
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`RaysHitMap`] is empty.
    pub fn calc_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let (left, right, top, bottom) = self.calc_2d_bounding_box(Length::zero())?;
        let bin_width: Length = (right - left) / usize_to_f64(nr_of_points.0);
        let bin_height: Length = (top - bottom) / usize_to_f64(nr_of_points.1);
        let bin_area: Area = bin_width * bin_height;
        let width_step = ((right - left) / (usize_to_f64(nr_of_points.0 - 1))).value;
        let height_step = ((top - bottom) / (usize_to_f64(nr_of_points.1 - 1))).value;
        let mut fluence_matrix = DMatrix::<Fluence>::zeros(nr_of_points.1, nr_of_points.0);
        for hit_point in &self.hit_map {
            let x_index =
                f64_to_usize(((hit_point.position.x.value - left.value) / width_step).floor());
            let y_index =
                f64_to_usize(((hit_point.position.y.value - bottom.value) / height_step).floor());
            let fluence = hit_point.energy / bin_area;
            fluence_matrix[(y_index, x_index)] += fluence;
        }
        Ok(FluenceData::new(fluence_matrix, left..right, bottom..top))
    }
    /// Calculates the fluence of this [`RaysHitMap`] using the "Voronoi" method
    /// # Attributes
    /// - `max_fluence`: the maximum allowed fluence on this surface
    /// # Errors
    /// This function errors if no reasonable axlimits  can be estimated due to only non-finite values in the positions
    #[allow(clippy::type_complexity)]
    pub fn calc_fluence_with_voronoi(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let mut pos_in_cm = MatrixXx2::<f64>::zeros(self.hit_map.len());
        let mut energy = DVector::<f64>::zeros(self.hit_map.len());
        // let mut energy_in_ray_bundle = 0.;

        if self.hit_map.len() < 3 {
            return Err(OpossumError::Other(
                "Too few points (<3) on hitmap to calculate fluence!".into(),
            ));
        }
        for (row, p) in self.hit_map.iter().enumerate() {
            pos_in_cm[(row, 0)] = p.position.x.get::<length::centimeter>();
            pos_in_cm[(row, 1)] = p.position.y.get::<length::centimeter>();
            energy[row] = p.energy.get::<joule>();
        }

        let proj_ax1_lim = AxLims::finite_from_dvector(&pos_in_cm.column(0)).ok_or_else(|| {
            OpossumError::Other(
                "cannot construct voronoi cells with non-finite axes bounds!".into(),
            )
        })?;
        let proj_ax2_lim = AxLims::finite_from_dvector(&pos_in_cm.column(1)).ok_or_else(|| {
            OpossumError::Other(
                "cannot construct voronoi cells with non-finite axes bounds!".into(),
            )
        })?;
        let (voronoi, _beam_area) = create_voronoi_cells(&pos_in_cm).map_err(|e| {
            OpossumError::Other(format!(
                "Voronoi diagram for fluence estimation could not be created!: {e}"
            ))
        })?;
        //get the voronoi cells
        let v_cells = voronoi.cells();
        let mut fluence_scatter = DVector::from_element(voronoi.sites.len(), f64::NAN);
        let mut max_fluence_val = 0.;
        for (i, v_cell) in v_cells.iter().enumerate() {
            let v_neighbours = v_cell
                .points()
                .iter()
                .map(|p| Point2::new(p.x, p.y))
                .collect::<Vec<Point2<f64>>>();
            if v_neighbours.len() >= 3 {
                let poly_area = calc_closed_poly_area(&v_neighbours)?;
                fluence_scatter[i] = energy[i] / poly_area;
                if max_fluence_val < fluence_scatter[i] || i == 0 {
                    max_fluence_val = fluence_scatter[i];
                }
            } else {
                warn!(
                    "polygon could not be created. number of neighbors {}",
                    v_neighbours.len()
                );
            }
        }

        //axes definition
        let co_ax1 = linspace(proj_ax1_lim.min, proj_ax1_lim.max, nr_of_points.0)?;
        let co_ax2 = linspace(proj_ax2_lim.min, proj_ax2_lim.max, nr_of_points.1)?;

        let voronied_data =
            VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence_scatter)?;
        //currently only interpolation. voronoid data for plotting must still be implemented
        let (interp_fluence, _) =
            interpolate_3d_triangulated_scatter_data(&voronied_data, &co_ax1, &co_ax2)?;
        let fluence_matrix = DMatrix::from_iterator(
            co_ax1.len(),
            co_ax2.len(),
            interp_fluence.iter().map(|val| J_per_cm2!(*val)),
        );
        let fluence_data = FluenceData::new(
            fluence_matrix,
            centimeter!(proj_ax1_lim.min)..centimeter!(proj_ax1_lim.max),
            centimeter!(proj_ax2_lim.min)..centimeter!(proj_ax2_lim.max),
        );
        Ok(fluence_data)
    }
    fn calc_2d_bounding_box(&self, margin: Length) -> OpmResult<(Length, Length, Length, Length)> {
        if !margin.is_finite() {
            return Err(OpossumError::Other("margin must be finite".into()));
        }
        self.hit_map.first().map_or_else(
            || {
                Err(OpossumError::Other(
                    "could not calculate bounding box".into(),
                ))
            },
            |hit_point| {
                let mut left = hit_point.position.x;
                let mut right = hit_point.position.x;
                let mut top = hit_point.position.y;
                let mut bottom = hit_point.position.y;
                for point in &self.hit_map {
                    if point.position.x < left {
                        left = point.position.x;
                    }
                    if point.position.y < bottom {
                        bottom = point.position.y;
                    }
                    if point.position.x > right {
                        right = point.position.x;
                    }
                    if point.position.y > top {
                        top = point.position.y;
                    }
                }
                left -= margin;
                right += margin;
                bottom -= margin;
                top += margin;
                Ok((left, right, top, bottom))
            },
        )
    }
    /// Returns the calc fluence with kde of this [`RaysHitMap`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn calc_fluence_with_kde(&self, nr_of_points: (usize, usize)) -> OpmResult<FluenceData> {
        let mut kde = Kde::default();
        let hitmap_2d = self
            .hit_map
            .iter()
            .map(|p| (p.position.xy(), p.energy))
            .collect();
        kde.set_hit_map(hitmap_2d);
        let est_bandwidth = kde.bandwidth_estimate();
        kde.set_band_width(est_bandwidth);
        let (left, right, top, bottom) = self.calc_2d_bounding_box(3. * est_bandwidth)?;
        let fluence_matrix = kde.kde_2d(&(left..right, bottom..top), nr_of_points);
        let fluence_data = FluenceData::new(fluence_matrix, left..right, bottom..top);
        Ok(fluence_data)
    }
    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`].
    ///
    /// Create a fluence map with the given number of points and the concrete estimator algorithm.
    ///
    /// # Errors
    ///
    /// This function will return an error if  the underlying concrete estimator function returns an error.
    pub fn calc_fluence_map(
        &self,
        nr_of_points: (usize, usize),
        estimator: &FluenceEstimator,
    ) -> OpmResult<FluenceData> {
        match estimator {
            FluenceEstimator::Voronoi => self.calc_fluence_with_voronoi(nr_of_points),
            FluenceEstimator::KDE => self.calc_fluence_with_kde(nr_of_points),
            FluenceEstimator::Binning => self.calc_fluence_with_binning(nr_of_points),
        }
    }
    /// Returns a reference to the hit map of this [`RaysHitMap`].
    #[must_use]
    pub fn hit_map(&self) -> &[HitPoint] {
        &self.hit_map
    }
}
#[cfg(test)]
mod test_hitpoint {
    use super::HitPoint;
    use crate::{joule, meter};
    use core::f64;
    #[test]
    fn new() {
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NAN)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::INFINITY)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(-0.1)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NEG_INFINITY)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(0.0)).is_ok());

        assert!(HitPoint::new(meter!(f64::NAN, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), joule!(1.0)).is_err());

        assert!(HitPoint::new(meter!(1.0, f64::NAN, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, f64::INFINITY, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), joule!(1.0)).is_err());

        assert!(HitPoint::new(meter!(1.0, 1.0, f64::NAN), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, f64::INFINITY), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), joule!(1.0)).is_err());
    }
    #[test]
    fn getters() {
        let hm = HitPoint::new(meter!(1.0, 2.0, 3.0), joule!(4.0)).unwrap();
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.energy(), joule!(4.0));
    }
}
#[cfg(test)]
mod test_rays_hit_map {
    use super::RaysHitMap;
    use crate::{joule, meter, surface::hit_map::HitPoint};
    use core::f64;
    #[test]
    fn new() {
        let rhm = RaysHitMap::new(&vec![]);
        assert_eq!(rhm.hit_map.len(), 0);

        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let rhm = RaysHitMap::new(&vec![hp]);
        assert_eq!(rhm.hit_map.len(), 1);
    }
    #[test]
    fn add_to_hitmap() {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_map.len(), 0);
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        rhm.add_hit_point(hp);
        assert_eq!(rhm.hit_map.len(), 1);
    }
    #[test]
    fn merge() {
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let mut rhm = RaysHitMap::new(&vec![hp]);
        let hp2 = HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(1.0)).unwrap();
        let rhm2 = RaysHitMap::new(&vec![hp2]);
        rhm.merge(&rhm2);
        assert_eq!(rhm.hit_map.len(), 2);
    }
    #[test]
    fn calc_2d_bounding_box() {
        let mut rhm = RaysHitMap::default();
        assert!(rhm.calc_2d_bounding_box(meter!(0.0)).is_err());
        rhm.add_hit_point(HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(0.0), meter!(0.0), meter!(0.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(-1.0), meter!(0.0), meter!(1.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-0.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, -1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 2.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(2.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(1.0, 2.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(1.5), meter!(2.5), meter!(-1.5))
        );
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NAN)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::INFINITY)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NEG_INFINITY)).is_err());
    }
}
