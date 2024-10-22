//! Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
//! [`OpticalSurface`](crate::surface::OpticalSurface).
use nalgebra::{DVector, MatrixXx2, Point2, Point3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Energy, Length};

use crate::{
    error::OpmResult,
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::unit_format::{
        get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
        get_unit_value_as_length_with_format_by_exponent,
    },
};

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
/// [`OpticalSurface`](crate::surface::OpticalSurface).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct HitMap {
    /// Stores the hitpoints of the rays on this surface, separated by their bounce level and the individual ray bundle
    /// First Vec stores a vector of hitmaps from different ray bundles, sorted by their bounce level
    /// Second Vec stores the hitmaps of the individual ray bundle. No sorting.
    /// Last vec stores the hitpoints of one ray bundle on this surface
    #[allow(clippy::type_complexity)]
    hit_map: Vec<Vec<Vec<(Point3<Length>, Energy)>>>,
}
impl HitMap {
    /// Returns a reference to the hit map of this [`HitMap`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`](crate::rays::Rays) that hit the surface.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn hit_map(&self) -> &[Vec<Vec<(Point3<Length>, Energy)>>] {
        &self.hit_map
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    ///
    pub fn add_to_hitmap(&mut self, hit_point: (Point3<Length>, Energy), bounce: usize) {
        if self.hit_map.len() <= bounce {
            for _i in 0..=bounce {
                if self.hit_map.len() < bounce {
                    self.hit_map.push(vec![vec![]]);
                }
            }
            self.hit_map.push(vec![vec![hit_point]]);
        } else {
            let hm_len = self.hit_map[bounce].len();
            if hm_len > 0 {
                self.hit_map[bounce][hm_len - 1].push(hit_point);
            }
        }
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

            for bounced_ray_bundles in &self.hit_map {
                for ray_bundle in bounced_ray_bundles {
                    let mut xy_pos = Vec::<Point2<Length>>::with_capacity(ray_bundle.len());
                    for p in ray_bundle {
                        xy_pos.push(Point2::new(p.0.x, p.0.y));

                        x_max = x_max.max(p.0.x.value);
                        y_max = y_max.max(p.0.y.value);
                        x_min = x_min.min(p.0.x.value);
                        y_min = y_min.min(p.0.y.value);
                    }
                    xy_positions.push(xy_pos);
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
            // .set(&PlotArgs::XLabel("x position (b)".into()))?
            // .set(&PlotArgs::YLabel("y position (b)".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }
}
