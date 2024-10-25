//! Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
//! [`OpticalSurface`](crate::surface::OpticalSurface).
use std::collections::HashMap;

use log::warn;
use nalgebra::{DVector, MatrixXx2, Point2, Point3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::centimeter,
    radiant_exposure::joule_per_square_centimeter,
};
use uuid::Uuid;

use crate::{
    error::{OpmResult, OpossumError},
    nodes::fluence_detector::Fluence,
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::{
        griddata::{calc_closed_poly_area, create_voronoi_cells, VoronoiedData},
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
    J_per_cm2,
};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
///Storage struct for `RaysHitMap` on a surface from a single bounce
pub struct BouncedHitMap {
    hit_map: HashMap<Uuid, RaysHitMap>,
}

impl BouncedHitMap {
    /// Add intersection point (with energy) to this [`BouncedHitMap`].
    pub fn add_to_hitmap(&mut self, hit_point: (Point3<Length>, Energy), uuid: &Uuid) {
        if let Some(rays_hit_map) = self.hit_map.get_mut(uuid) {
            rays_hit_map.add_to_hitmap(hit_point);
        } else {
            self.hit_map.insert(*uuid, RaysHitMap::new(vec![hit_point]));
        }
    }

    /// creates a new [`BouncedHitMap`]
    #[must_use]
    pub const fn new(hit_points: HashMap<Uuid, RaysHitMap>) -> Self {
        Self {
            hit_map: hit_points,
        }
    }
    ///returns a reference to a [`RaysHitMap`] in this [`BouncedHitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, uuid: &Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get(uuid)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
///Storage struct for hitpoints on a surface from a single raybundle
pub struct RaysHitMap {
    hit_map: Vec<(Point3<Length>, Energy)>,
}

impl RaysHitMap {
    /// Add intersection point (with energy) to this [`HitMap`].
    pub fn add_to_hitmap(&mut self, hit_point: (Point3<Length>, Energy)) {
        self.hit_map.push(hit_point);
    }

    /// creates a new [`RaysHitMap`]
    #[must_use]
    pub fn new(hit_points: Vec<(Point3<Length>, Energy)>) -> Self {
        Self {
            hit_map: hit_points,
        }
    }

    /// Calculates the fluence of a ray bundle that is stored in this hitmap
    /// # Attributes
    /// - `max_fluence`: the maximum allowed fluence on this surface
    /// # Errors
    /// This function errors if no reasonable axlimits  can be estimated due to only non-finite values in the positions
    #[allow(clippy::type_complexity)]
    pub fn calc_fluence(
        &self,
        max_fluence: Fluence,
    ) -> OpmResult<Option<(VoronoiedData, AxLims, AxLims, Fluence, Fluence)>> {
        let mut show_hitmap = false;
        let max_fluence_jcm2 = max_fluence.get::<joule_per_square_centimeter>();
        let mut pos_in_cm = MatrixXx2::<f64>::zeros(self.hit_map.len());
        let mut energy = DVector::<f64>::zeros(self.hit_map.len());
        let mut energy_in_ray_bundle = 0.;

        if self.hit_map.len() < 3 {
            warn!("Too few points on hitmap to calculate fluence!");
            return Ok(None);
        }

        for (row, p) in self.hit_map.iter().enumerate() {
            pos_in_cm[(row, 0)] = p.0.x.get::<centimeter>();
            pos_in_cm[(row, 1)] = p.0.y.get::<centimeter>();
            energy[row] = p.1.get::<joule>();
            energy_in_ray_bundle += energy[row];
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
        let (voronoi, beam_area) = create_voronoi_cells(&pos_in_cm).map_err(|_| {
            OpossumError::Other(
                "Voronoi diagram for fluence estimation could not be created!".into(),
            )
        })?;

        //get the voronoi cells
        let v_cells = voronoi.cells();
        let mut fluence_scatter = DVector::from_element(voronoi.sites.len(), f64::NAN);
        let mut max_fluence_val = f64::NEG_INFINITY;
        for (i, v_cell) in v_cells.iter().enumerate() {
            let v_neighbours = v_cell
                .points()
                .iter()
                .map(|p| Point2::new(p.x, p.y))
                .collect::<Vec<Point2<f64>>>();
            if v_neighbours.len() >= 3 {
                let poly_area = calc_closed_poly_area(&v_neighbours)?;
                fluence_scatter[i] = energy[i] / poly_area;
                if fluence_scatter[i] > max_fluence_jcm2 {
                    if max_fluence_val < fluence_scatter[i] {
                        max_fluence_val = fluence_scatter[i];
                    }
                    show_hitmap = true;
                }
            } else {
                warn!(
                    "polygon could not be created. number of neighbors {}",
                    v_neighbours.len()
                );
            }
        }
        if show_hitmap {
            Ok(Some((
                VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence_scatter)?,
                proj_ax1_lim,
                proj_ax2_lim,
                J_per_cm2!(energy_in_ray_bundle / beam_area),
                J_per_cm2!(max_fluence_val),
            )))
        } else {
            Ok(None)
        }
    }
}

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
/// [`OpticalSurface`](crate::surface::OpticalSurface).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct HitMap {
    /// Stores the hitpoints of the rays on this surface, separated by their bounce level and the individual ray bundle
    hit_map: Vec<BouncedHitMap>,
    /// Stores the fluence and position in the history of the ray bundles that create a critical fluence on this surface. key value is the uuid of the ray bundle
    critical_fluence: HashMap<Uuid, (Fluence, usize)>,
}
impl HitMap {
    /// Returns a reference to the hit map of this [`HitMap`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`](crate::rays::Rays) that hit the surface.
    #[must_use]
    pub fn hit_map(&self) -> &[BouncedHitMap] {
        &self.hit_map
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    ///
    pub fn add_to_hitmap(
        &mut self,
        hit_point: (Point3<Length>, Energy),
        bounce: usize,
        uuid: &Uuid,
    ) {
        // make sure that vector is large enough to insert the data
        if self.hit_map.len() <= bounce {
            for _i in 0..bounce + 1 - self.hit_map.len() {
                self.hit_map.push(BouncedHitMap::default());
            }
        }
        self.hit_map[bounce].add_to_hitmap(hit_point, uuid);
    }
    /// Reset this [`HitMap`].
    ///
    /// This functions clears all point of the map.
    pub fn reset(&mut self) {
        self.hit_map.clear();
    }
    /// Returns `true` the [`HitMap`] is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hit_map.is_empty()
    }

    /// returns a reference to the `critical_fluence` field of this [`HitMap`] which contains:
    /// - the uuid of the ray bundle that causes this critical fluence on this surface as key
    /// - a tuple containing the calculated peak fluence and the index of the position history which can be used to reconstruct the ray-propagation plot later on
    #[must_use]
    pub fn critical_fluences(&self) -> &HashMap<Uuid, (Fluence, usize)> {
        &self.critical_fluence
    }

    ///stores a critical fluence in a hitmap
    pub fn add_critical_fluence(&mut self, uuid: &Uuid, rays_hist_pos: usize, fluence: Fluence) {
        self.critical_fluence
            .insert(*uuid, (fluence, rays_hist_pos));
    }

    ///returns a reference to a [`RaysHitMap`] in this [`HitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: &Uuid) -> Option<&RaysHitMap> {
        self.hit_map[bounce].get_rays_hit_map(uuid)
    }
}
impl From<HitMap> for Proptype {
    fn from(value: HitMap) -> Self {
        Self::HitMap(value)
    }
}
impl Plottable for HitMap {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        //ray plot series
        if self.hit_map.is_empty() {
            Ok(None)
        } else {
            let mut plt_series = Vec::<PlotSeries>::with_capacity(self.hit_map.len());
            let mut xy_positions = Vec::<Vec<Point2<Length>>>::with_capacity(self.hit_map.len());
            let mut x_max = f64::NEG_INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            let mut x_min = f64::INFINITY;
            let mut y_min = f64::INFINITY;

            for (i, bounced_ray_bundles) in self.hit_map.iter().enumerate() {
                xy_positions.push(Vec::<Point2<Length>>::new());
                for rays_hitmap in bounced_ray_bundles.hit_map.values() {
                    for p in &rays_hitmap.hit_map {
                        xy_positions[i].push(Point2::new(p.0.x, p.0.y));

                        x_max = x_max.max(p.0.x.value);
                        y_max = y_max.max(p.0.y.value);
                        x_min = x_min.min(p.0.x.value);
                        y_min = y_min.min(p.0.y.value);
                    }
                }
            }
            let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
            let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
            let y_prefix = get_prefix_for_base_unit(y_max);
            let x_prefix = get_prefix_for_base_unit(x_max);

            plt_type.set_plot_param(&PlotArgs::XLabel(format!("x position ({y_prefix}m)")))?;
            plt_type.set_plot_param(&PlotArgs::YLabel(format!("y position ({x_prefix}m)")))?;

            for (i, xy_pos) in xy_positions.iter().enumerate() {
                if xy_pos.is_empty() {
                    continue;
                }
                let x_vals = xy_pos
                    .iter()
                    .map(|p| get_unit_value_as_length_with_format_by_exponent(p.x, x_exponent))
                    .collect::<Vec<f64>>();
                let y_vals = xy_pos
                    .iter()
                    .map(|p| get_unit_value_as_length_with_format_by_exponent(p.y, y_exponent))
                    .collect::<Vec<f64>>();

                let data = PlotData::Dim2 {
                    xy_data: MatrixXx2::from_columns(&[
                        DVector::from_vec(x_vals),
                        DVector::from_vec(y_vals),
                    ]),
                };

                let gradient = colorous::TURBO;
                let c = if self.hit_map.len() > 10 {
                    gradient.eval_rational(i, self.hit_map.len())
                } else {
                    colorous::CATEGORY10[i]
                };
                let label = format!("Bounce: {i}");
                plt_series.push(PlotSeries::new(
                    &data,
                    RGBAColor(c.r, c.g, c.b, 1.),
                    Some(label),
                ));
            }

            x_max *= f64::powi(10., -x_exponent);
            y_max *= f64::powi(10., -y_exponent);
            x_min *= f64::powi(10., -x_exponent);
            y_min *= f64::powi(10., -y_exponent);

            let x_diff = x_max - x_min;
            let y_diff = y_max - y_min;
            let x_limits = AxLims::create_useful_axlims(
                0.1f64.mul_add(-x_diff, x_min),
                0.1f64.mul_add(x_diff, x_max),
            );
            let y_limits = AxLims::create_useful_axlims(
                0.1f64.mul_add(-y_diff, y_min),
                0.1f64.mul_add(y_diff, y_max),
            );

            plt_type.set_plot_param(&PlotArgs::XLim(x_limits))?;
            plt_type.set_plot_param(&PlotArgs::YLim(y_limits))?;
            Ok(Some(plt_series))
        }
    }
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }
}
