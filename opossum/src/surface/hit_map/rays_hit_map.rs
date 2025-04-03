//! The structures for storing the actual hitmap.
//!
//! This module also conatins the routines for genearating a fluence map using different estimator strategies.
use core::f64;
use std::ops::Range;

use itertools::Itertools;
use log::warn;
use nalgebra::{DMatrix, DVector, MatrixXx2, Point2, Point3};
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Area, Energy, Length},
    length,
    radiant_exposure::joule_per_square_centimeter,
};

use crate::{
    centimeter,
    error::{OpmResult, OpossumError},
    kde::Kde,
    meter,
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

use super::fluence_estimator::FluenceEstimator;

/// A hit point as part of a [`RaysHitMap`].
///
/// It stores the position (intersection point) and the energy of a [`Ray`](crate::ray::Ray) that
/// has hit an [`OpticSurface`](crate::surface::optic_surface::OpticSurface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyHitPoint {
    /// position of the intersection point
    pub position: Point3<Length>,
    /// energy of the ray that intersected the surface
    pub value: Energy,
}

impl EnergyHitPoint {
    /// Create a new [`EnergyHitPoint`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given value is negative or not finite.
    ///   - the position coordinates (x/y/z) are not finite.
    pub fn new(position: Point3<Length>, value: Energy) -> OpmResult<Self> {
        if !value.is_finite() | value.is_sign_negative() {
            return Err(OpossumError::Other(
                "value must be positive and finite".into(),
            ));
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other("position must be finite".into()));
        }
        Ok(Self { position, value })
    }
    /// Returns the position of this [`EnergyHitPoint`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the energy of this [`EnergyHitPoint`].
    #[must_use]
    pub fn value(&self) -> Energy {
        self.value
    }
}

/// A hit point as part of a [`RaysHitMap`].
///
/// It stores the position (intersection point) and the energy of a [`Ray`](crate::ray::Ray) that
/// has hit an [`OpticSurface`](crate::surface::optic_surface::OpticSurface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluenceHitPoint {
    /// position of the intersection point
    pub position: Point3<Length>,
    /// fluence of the ray that intersected the surface
    pub value: Fluence,
}

impl FluenceHitPoint {
    /// Create a new [`FluenceHitPoint`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given value is negative or not finite.
    ///   - the position coordinates (x/y/z) are not finite.
    pub fn new(position: Point3<Length>, value: Fluence) -> OpmResult<Self> {
        if !value.is_finite() | value.is_sign_negative() {
            return Err(OpossumError::Other(
                "value must be positive and finite".into(),
            ));
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other("position must be finite".into()));
        }
        Ok(Self { position, value })
    }
    /// Returns the position of this [`FluenceHitPoint`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the energy of this [`FluenceHitPoint`].
    #[must_use]
    pub fn value(&self) -> Fluence {
        self.value
    }
}

/// Enum to store different types of hit point quantities in a hitmap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitPoints {
    ///Energy hitpoints contain a vec of [`EnergyHitPoint`]
    Energy(Vec<EnergyHitPoint>),
    ///Fluence hitpoints contain a vec of [`FluenceHitPoint`]
    Fluence(Vec<FluenceHitPoint>),
}

impl HitPoints {
    /// Returns the length of the store hit-point vector within this [`HitPoints`]
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Energy(vec) => vec.len(),
            Self::Fluence(vec) => vec.len(),
        }
    }
    /// Checks if the stored hit-point vectors are emtpy. Returns true if empty, false otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Energy(vec) => vec.is_empty(),
            Self::Fluence(vec) => vec.is_empty(),
        }
    }

    /// Returns a vector of 3d positions of the stored hit-point vectors
    #[must_use]
    pub fn positions(&self) -> Vec<Point3<Length>> {
        match self {
            Self::Energy(vec) => vec.iter().map(EnergyHitPoint::position).collect_vec(),
            Self::Fluence(vec) => vec.iter().map(FluenceHitPoint::position).collect_vec(),
        }
    }
}
/// Enum to pass different types of hit point quantities in a hitmap
pub enum HitPoint {
    ///Energy hitpoint contains an [`EnergyHitPoint`]
    Energy(EnergyHitPoint),
    ///Fluence hitpoint contains a [`FluenceHitPoint`]
    Fluence(FluenceHitPoint),
}
impl Default for HitPoints {
    fn default() -> Self {
        Self::Energy(Vec::<EnergyHitPoint>::new())
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
///Storage struct for hitpoints on a surface from a single ray bundle
pub struct RaysHitMap {
    hit_points: HitPoints,
    x_lims: (Length, Length),
    y_lims: (Length, Length),
}
impl RaysHitMap {
    /// Creates a new [`RaysHitMap`]
    #[must_use]
    pub fn new(hit_points: HitPoints) -> Self {
        let (xmin, xmax, ymin, ymax) = match &hit_points {
            HitPoints::Energy(vec) => vec.iter().fold(
                (
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                ),
                |arg0, v| {
                    (
                        arg0.0.min(v.position.x),
                        arg0.1.max(v.position.x),
                        arg0.2.min(v.position.y),
                        arg0.3.max(v.position.y),
                    )
                },
            ),
            HitPoints::Fluence(vec) => vec.iter().fold(
                (
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                ),
                |arg0, v| {
                    (
                        arg0.0.min(v.position.x),
                        arg0.1.max(v.position.x),
                        arg0.2.min(v.position.y),
                        arg0.3.max(v.position.y),
                    )
                },
            ),
        };
        Self {
            hit_points,
            x_lims: (xmin, xmax),
            y_lims: (ymin, ymax),
        }
    }
    /// Add intersection point (with energy) to this [`RaysHitMap`].
    ///
    /// # Errors
    /// This function errors if the hitpoint tha should be added does not match the already stored hit point type
    pub fn add_hit_point(&mut self, hit_point: HitPoint) -> OpmResult<()> {
        match hit_point {
            HitPoint::Energy(energy_hit_point) => {
                if let HitPoints::Energy(v) = &mut self.hit_points {
                    if v.is_empty() {
                        self.x_lims.0 = energy_hit_point.position.x;
                        self.x_lims.1 = energy_hit_point.position.x;
                        self.y_lims.0 = energy_hit_point.position.y;
                        self.y_lims.1 = energy_hit_point.position.y;
                    } else {
                        self.x_lims.0 = energy_hit_point.position.x.min(self.x_lims.0);
                        self.x_lims.1 = energy_hit_point.position.x.max(self.x_lims.1);
                        self.y_lims.0 = energy_hit_point.position.y.min(self.y_lims.0);
                        self.y_lims.1 = energy_hit_point.position.y.max(self.y_lims.1);
                    }
                    v.push(energy_hit_point);
                } else {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Energyhitpoint!".into(),
                    ));
                }
            }
            HitPoint::Fluence(fluence_hit_point) => {
                if let HitPoints::Fluence(v) = &mut self.hit_points {
                    if v.is_empty() {
                        self.x_lims.0 = fluence_hit_point.position.x;
                        self.x_lims.1 = fluence_hit_point.position.x;
                        self.y_lims.0 = fluence_hit_point.position.y;
                        self.y_lims.1 = fluence_hit_point.position.y;
                    } else {
                        self.x_lims.0 = fluence_hit_point.position.x.min(self.x_lims.0);
                        self.x_lims.1 = fluence_hit_point.position.x.max(self.x_lims.1);
                        self.y_lims.0 = fluence_hit_point.position.y.min(self.y_lims.0);
                        self.y_lims.1 = fluence_hit_point.position.y.max(self.y_lims.1);
                    }
                    v.push(fluence_hit_point);
                } else {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ));
                }
            }
        };
        Ok(())
    }

    /// Returns the x limit (min, max) of the [`HitPoints`] that are stored in this [`RaysHitMap`]
    #[must_use]
    pub fn x_lims(&self) -> &(Length, Length) {
        &self.x_lims
    }
    /// Returns the y limit (min, max) of the [`HitPoints`] that are stored in this [`RaysHitMap`]
    #[must_use]
    pub fn y_lims(&self) -> &(Length, Length) {
        &self.y_lims
    }

    /// Merge this [`RaysHitMap`] with another [`RaysHitMap`].
    ///
    /// # Errors
    /// This function errors if the hit-point type of the respective [`RaysHitMap`] does not match
    pub fn merge(&mut self, other_map: &Self) -> OpmResult<()> {
        match &mut self.hit_points {
            HitPoints::Energy(mut_vec) => match &other_map.hit_points {
                HitPoints::Energy(vec) => {
                    for hit_point in vec {
                        mut_vec.push(hit_point.clone());
                    }
                }
                HitPoints::Fluence(_) => {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ))
                }
            },
            HitPoints::Fluence(mut_vec) => match &other_map.hit_points {
                HitPoints::Fluence(vec) => {
                    for hit_point in vec {
                        mut_vec.push(hit_point.clone());
                    }
                }
                HitPoints::Energy(_) => {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ))
                }
            },
        };
        Ok(())
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Binning" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - the [`RaysHitMap`] is empty.
    /// - The hit point type is neither energy nor fluence
    pub fn calc_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            let (left, right, top, bottom) =
                if let (Some(range_1), Some(range_2)) = (ax_1_range, ax_2_range) {
                    (range_1.start, range_1.end, range_2.start, range_2.end)
                } else {
                    self.calc_2d_bounding_box(Length::zero())?
                };
            let bin_width: Length = (right - left) / usize_to_f64(nr_of_points.0);
            let bin_height: Length = (top - bottom) / usize_to_f64(nr_of_points.1);

            let bin_area: Area = bin_width * bin_height;

            let width_step = (right - left) / (usize_to_f64(nr_of_points.0 - 1));
            let height_step = (top - bottom) / (usize_to_f64(nr_of_points.1 - 1));

            let mut fluence_matrix = DMatrix::<Fluence>::zeros(nr_of_points.1, nr_of_points.0);
            for hit_point in hit_points {
                let x_index = f64_to_usize(((hit_point.position.x - left).value / width_step.value).floor());
                let y_index = f64_to_usize(((hit_point.position.y - bottom).value/ height_step.value).floor());
                let fluence = hit_point.value / bin_area;
                fluence_matrix[(y_index, x_index)] += fluence;
            }
            Ok(FluenceData::new(
                fluence_matrix,
                left..right,
                bottom..top,
                FluenceEstimator::Binning,
            ))
        } else if let HitPoints::Fluence(_) = &self.hit_points {
            warn!("Unexpected type of HitPoints for binning estimator! Changing to helper-ray estimator!");
            self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("wrong hit point type for to calculate fluence with binning! Must be an Energyhitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Voronoi" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - there are too few data points for voronoi cell creation (less than three)
    /// - the voronoi diagram generation fails
    /// - no axes ranges are provided and the data is not sufficient to provide valid ranges
    /// - the interpolation step fails
    #[allow(clippy::too_many_lines)]
    pub fn calc_fluence_with_voronoi(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            let mut pos_in_cm = MatrixXx2::<f64>::zeros(hit_points.len());
            let mut energy = DVector::<f64>::zeros(hit_points.len());
            if hit_points.len() < 3 {
                return Err(OpossumError::Other(
                    "Too few points (<3) on hitmap to calculate fluence!".into(),
                ));
            }
            for (row, p) in hit_points.iter().enumerate() {
                pos_in_cm[(row, 0)] = p.position.x.get::<length::centimeter>();
                pos_in_cm[(row, 1)] = p.position.y.get::<length::centimeter>();
                energy[row] = p.value.get::<joule>();
            }
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
            let (co_ax1, ax_1_range) = if let Some(range) = ax_1_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.0,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax1_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(0)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct voronoi cells with non-finite axes bounds!".into(),
                        )
                    })?;
                (
                    linspace(proj_ax1_lim.min, proj_ax1_lim.max, nr_of_points.0)?,
                    centimeter!(proj_ax1_lim.min)..centimeter!(proj_ax1_lim.max),
                )
            };
            let (co_ax2, ax_2_range) = if let Some(range) = ax_2_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.0,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax2_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(1)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct voronoi cells with non-finite axes bounds!".into(),
                        )
                    })?;
                (
                    linspace(proj_ax2_lim.min, proj_ax2_lim.max, nr_of_points.0)?,
                    centimeter!(proj_ax2_lim.min)..centimeter!(proj_ax2_lim.max),
                )
            };
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
                ax_1_range,
                ax_2_range,
                FluenceEstimator::Voronoi,
            );
            Ok(fluence_data)
        } else if let HitPoints::Fluence(_) = &self.hit_points {
            warn!("Unexpected type of HitPoints for Voronoi estimator! Changing to helper-ray estimator!");
            self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("wrong hit point type for to calculate fluence with voronoi cells! Must be an Energyhitpoint!".into()))
        }
    }
    fn calc_2d_bounding_box(&self, margin: Length) -> OpmResult<(Length, Length, Length, Length)> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            if !margin.is_finite() {
                return Err(OpossumError::Other("margin must be finite".into()));
            }
            hit_points.first().map_or_else(
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
                    for point in hit_points {
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
        } else {
            Err(OpossumError::Analysis("wrong hit point type for to calculate 2d bounding box for kde! Must be an Energyhitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Kernel Density Estimator (KDE)" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - no bandwidth for the kernel can be estimated
    /// - The hit point type is neither energy nor fluence
    pub fn calc_fluence_with_kde(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            let mut kde = Kde::default();
            let hitmap_2d = hit_points
                .iter()
                .map(|p| (p.position.xy(), p.value))
                .collect();
            kde.set_hit_map(hitmap_2d);
            let est_bandwidth = kde.bandwidth_estimate();
            kde.set_band_width(est_bandwidth)?;
            let (left, right, top, bottom) =
                if let (Some(range_1), Some(range_2)) = (ax_1_range, ax_2_range) {
                    (range_1.start, range_1.end, range_2.start, range_2.end)
                } else {
                    self.calc_2d_bounding_box(3. * est_bandwidth)?
                };
            let fluence_matrix = kde.kde_2d(&(left..right, bottom..top), nr_of_points);
            let fluence_data = FluenceData::new(
                fluence_matrix,
                left..right,
                bottom..top,
                FluenceEstimator::KDE,
            );
            Ok(fluence_data)
        } else if let HitPoints::Fluence(_) = &self.hit_points {
            warn!("Unexpected type of HitPoints for kernel density estimator! Changing to helper-ray estimator!");
            self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type for to calculate fluence with kde! Must be an EnergyHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Helper Rays" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - there are too few data points for voronoi cell creation (less than three)
    /// - the voronoi diagram generation fails
    /// - no axes ranges are provided and the data is not sufficient to provide valid ranges
    /// - the interpolation step fails
    pub fn calc_fluence_with_helper_rays(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Fluence(hit_points) = &self.hit_points {
            let mut pos_in_cm = MatrixXx2::<f64>::zeros(hit_points.len());
            let mut fluence = DVector::<f64>::zeros(hit_points.len());

            if hit_points.len() < 3 {
                return Err(OpossumError::Other(
                    "Too few points (<3) on hitmap to calculate fluence!".into(),
                ));
            }
            for (row, p) in hit_points.iter().enumerate() {
                pos_in_cm[(row, 0)] = p.position.x.get::<length::centimeter>();
                pos_in_cm[(row, 1)] = p.position.y.get::<length::centimeter>();
                fluence[row] = p.value.get::<joule_per_square_centimeter>();
            }

            let (co_ax1, ax_1_range) = if let Some(range) = ax_1_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.0,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax1_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(0)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct valid axis limits with non-finite axes bounds!"
                                .into(),
                        )
                    })?;
                (
                    linspace(proj_ax1_lim.min, proj_ax1_lim.max, nr_of_points.0)?,
                    centimeter!(proj_ax1_lim.min)..centimeter!(proj_ax1_lim.max),
                )
            };

            let (co_ax2, ax_2_range) = if let Some(range) = ax_2_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.0,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax2_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(1)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct valid axis limits with non-finite axes bounds!"
                                .into(),
                        )
                    })?;
                (
                    linspace(proj_ax2_lim.min, proj_ax2_lim.max, nr_of_points.0)?,
                    centimeter!(proj_ax2_lim.min)..centimeter!(proj_ax2_lim.max),
                )
            };

            let (voronoi, _beam_area) = create_voronoi_cells(&pos_in_cm).map_err(|e| {
                OpossumError::Other(format!(
                    "Voronoi diagram for fluence interpolation could not be created!: {e}"
                ))
            })?;

            let fluence =
                fluence.insert_rows(hit_points.len(), voronoi.sites.len() - hit_points.len(), 0.);

            let voronoi_fluence_scatter =
                VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence)?;

            //currently only interpolation. voronoid data for plotting must still be implemented
            let (interp_fluence, _) = interpolate_3d_triangulated_scatter_data(
                &voronoi_fluence_scatter,
                &co_ax1,
                &co_ax2,
            )?;

            Ok(FluenceData::new(
                DMatrix::from_iterator(
                    co_ax1.len(),
                    co_ax2.len(),
                    interp_fluence.iter().map(|val| J_per_cm2!(*val)),
                ),
                ax_1_range,
                ax_2_range,
                FluenceEstimator::HelperRays,
            ))
        } else if let HitPoints::Energy(_) = &self.hit_points {
            warn!("Unexpected type of HitPoints for helper-ray estimator! Changing to voronoi estimator!");
            self.calc_fluence_with_voronoi(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type for to calculate fluence with helper rays! Must be a FluenceHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`].
    ///
    /// Create a fluence map with the given number of points and the concrete estimator algorithm.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`estimator`: Reference to the [`FluenceEstimator`] that should be used toestimate the fluence value.
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated
    ///
    /// # Errors
    /// This function will return an error if the underlying concrete estimator function returns an error.
    pub fn calc_fluence_map(
        &self,
        nr_of_points: (usize, usize),
        estimator: &FluenceEstimator,
        ax_1_range_opt: Option<&Range<Length>>,
        ax_2_range_opt: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        match estimator {
            FluenceEstimator::Voronoi => {
                self.calc_fluence_with_voronoi(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::KDE => {
                self.calc_fluence_with_kde(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::Binning => {
                self.calc_fluence_with_binning(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::HelperRays => {
                self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
        }
    }

    /// Gets the maximum fluence value off this [`RaysHitMap`]
    ///
    /// # Attributes
    /// -`estimator`: Reference to the [`FluenceEstimator`] that should be used toestimate the fluence value.
    ///
    /// # Errors
    /// This function will return an error if
    /// - `calc_fluence_map` returns an error
    /// - if the `HitPoint` type macthes netieher energy or fluence
    pub fn get_max_fluence(&self, estimator: &FluenceEstimator) -> OpmResult<Fluence> {
        match estimator {
            FluenceEstimator::Voronoi | FluenceEstimator::KDE | FluenceEstimator::Binning => {
                Ok(self
                    .calc_fluence_map((101, 101), estimator, None, None)?
                    .peak())
            }
            FluenceEstimator::HelperRays => {
                if let HitPoints::Fluence(hit_points) = &self.hit_points {
                    Ok(hit_points
                        .iter()
                        .fold(J_per_cm2!(0.), |init, val| val.value.max(init)))
                } else if let HitPoints::Energy(_) = &self.hit_points {
                    Ok(self
                        .calc_fluence_map((101, 101), &FluenceEstimator::Voronoi, None, None)?
                        .peak())
                } else {
                    Err(OpossumError::Analysis("Undefined HitPointType! Cannot use get_max_fluence method to retrieve maximum fluence!".into()))
                }
            }
        }
    }

    /// Returns a reference to the hit map of this [`RaysHitMap`].
    #[must_use]
    pub const fn hit_map(&self) -> &HitPoints {
        &self.hit_points
    }
}
#[cfg(test)]
mod test_hitpoint {
    use crate::{
        joule, meter,
        surface::hit_map::rays_hit_map::{EnergyHitPoint, FluenceHitPoint},
        J_per_cm2,
    };
    use core::f64;
    #[test]
    fn new_fluence_hit_point() {
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::NAN)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::INFINITY)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(-0.1)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::NEG_INFINITY)).is_err()
        );
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(0.0)).is_ok());

        assert!(FluenceHitPoint::new(meter!(f64::NAN, 1.0, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), J_per_cm2!(1.0)).is_err()
        );

        assert!(FluenceHitPoint::new(meter!(1.0, f64::NAN, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, f64::INFINITY, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), J_per_cm2!(1.0)).is_err()
        );

        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, f64::NAN), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, f64::INFINITY), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), J_per_cm2!(1.0)).is_err()
        );
    }
    #[test]
    fn new_energy_hit_point() {
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NAN)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::INFINITY)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(-0.1)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NEG_INFINITY)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(0.0)).is_ok());

        assert!(EnergyHitPoint::new(meter!(f64::NAN, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), joule!(1.0)).is_err());

        assert!(EnergyHitPoint::new(meter!(1.0, f64::NAN, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, f64::INFINITY, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), joule!(1.0)).is_err());

        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::NAN), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::INFINITY), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), joule!(1.0)).is_err());
    }
    #[test]
    fn getters_fluence_hit_point() {
        let hm = FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0)).unwrap();
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.value(), J_per_cm2!(4.0));
    }
    #[test]
    fn getters_energ_hit_point() {
        let hm = EnergyHitPoint::new(meter!(1.0, 2.0, 3.0), joule!(4.0)).unwrap();
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.value(), joule!(4.0));
    }
}
#[cfg(test)]
mod test_hitpoints {
    use super::HitPoints;
    use crate::{
        joule, meter,
        surface::hit_map::rays_hit_map::{EnergyHitPoint, FluenceHitPoint},
        J_per_cm2,
    };
    #[test]
    fn len() {
        let hp = HitPoints::Energy(vec![]);
        assert!(hp.is_empty());
        let hp = HitPoints::Fluence(vec![]);
        assert!(hp.is_empty());

        let hp = HitPoints::Energy(vec![EnergyHitPoint::new(
            meter!(1.0, 2.0, 3.0),
            joule!(4.0),
        )
        .unwrap()]);
        assert!(!hp.is_empty());
        assert!(hp.len() == 1);

        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0)).unwrap(),
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0)).unwrap(),
        ]);
        assert!(!hp.is_empty());
        assert!(hp.len() == 2);
    }
    #[test]
    fn positions() {
        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0)).unwrap(),
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0)).unwrap(),
        ]);
        let pos = hp.positions();
        assert_eq!(pos[0].x.value, 1.);
        assert_eq!(pos[0].y.value, 2.);
        assert_eq!(pos[0].z.value, 3.);
        assert_eq!(pos[1].x.value, 2.);
        assert_eq!(pos[1].y.value, 3.);
        assert_eq!(pos[1].z.value, 4.);
    }
}
#[cfg(test)]
mod test_rays_hit_map {
    use super::RaysHitMap;
    use crate::{
        joule, meter,
        surface::hit_map::rays_hit_map::{EnergyHitPoint, FluenceHitPoint, HitPoint, HitPoints},
        J_per_cm2,
    };
    use core::f64;
    #[test]
    fn lims() {
        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0)).unwrap(),
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0)).unwrap(),
        ]);
        let rhm = RaysHitMap::new(hp);
        assert_eq!(rhm.x_lims.0.value, 1.0);
        assert_eq!(rhm.x_lims.1.value, 2.0);
        assert_eq!(rhm.y_lims.0.value, 2.0);
        assert_eq!(rhm.y_lims.1.value, 3.0);
    }
    #[test]
    fn new_energy_hit_point() {
        let rhm = RaysHitMap::new(HitPoints::Energy(vec![]));
        assert_eq!(rhm.hit_points.len(), 0);

        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        assert_eq!(rhm.hit_points.len(), 1);
    }
    #[test]
    fn new_fluence_hit_point() {
        let rhm = RaysHitMap::new(HitPoints::Fluence(vec![]));
        assert_eq!(rhm.hit_points.len(), 0);

        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap();
        let rhm = RaysHitMap::new(HitPoints::Fluence(vec![hp]));
        assert_eq!(rhm.hit_points.len(), 1);
    }
    #[test]
    fn add_to_hitmap_energy_hit_point() {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        rhm.add_hit_point(HitPoint::Energy(hp)).unwrap();
        assert_eq!(rhm.hit_points.len(), 1);
    }
    #[test]
    fn add_energy_to_hitmap_fluence_hit_point() {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap();
        assert!(rhm.add_hit_point(HitPoint::Fluence(hp)).is_err());
    }
    #[test]
    fn merge_energy_hit_point() {
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let mut rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        let hp2 = EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(1.0)).unwrap();
        let rhm2 = RaysHitMap::new(HitPoints::Energy(vec![hp2]));
        rhm.merge(&rhm2).unwrap();
        assert_eq!(rhm.hit_points.len(), 2);
    }
    #[test]
    fn merge_fluence_hit_point() {
        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap();
        let mut rhm = RaysHitMap::new(HitPoints::Fluence(vec![hp]));
        let hp2 = FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(1.0)).unwrap();
        let rhm2 = RaysHitMap::new(HitPoints::Fluence(vec![hp2]));
        rhm.merge(&rhm2).unwrap();
        assert_eq!(rhm.hit_points.len(), 2);
    }
    #[test]
    fn merge_fluence_to_energy() {
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let mut rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        let hp2 = FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(1.0)).unwrap();
        let mut rhm2 = RaysHitMap::new(HitPoints::Fluence(vec![hp2]));
        assert!(rhm.merge(&rhm2).is_err());
        assert!(rhm2.merge(&rhm).is_err());
    }
    #[test]
    fn calc_2d_bounding_box() {
        let mut rhm = RaysHitMap::default();
        assert!(rhm.calc_2d_bounding_box(meter!(0.0)).is_err());

        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(0.0), meter!(0.0), meter!(0.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(-1.0), meter!(0.0), meter!(1.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-0.5))
        );
        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(-1.0, -1.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(-1.0, 2.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(2.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::Energy(
            EnergyHitPoint::new(meter!(1.0, 2.0, 0.0), joule!(1.0)).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(1.5), meter!(2.5), meter!(-1.5))
        );
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NAN)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::INFINITY)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NEG_INFINITY)).is_err());
    }
}
