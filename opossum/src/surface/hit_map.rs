//! Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
//! [`OpticalSurface`](crate::surface::OpticalSurface).
use nalgebra::{DVector, MatrixXx2, Point2, Point3};
use plotters::style::RGBAColor;
use uom::si::{
    f64::{Energy, Length},
    length::meter,
};

use crate::{
    error::OpmResult,
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    utils::unit_format::{
        get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
        get_unit_value_as_length_with_format_by_exponent,
    },
};

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
/// [`OpticalSurface`](crate::surface::OpticalSurface).
#[derive(Default, Debug, Clone)]
pub struct HitMap {
    hit_map: Vec<(Point3<Length>, Energy)>,
}
impl HitMap {
    /// Returns a reference to the hit map of this [`HitMap`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`](crate::rays::Rays) that hit the surface.
    #[must_use]
    pub fn hit_map(&self) -> &[(Point3<Length>, Energy)] {
        &self.hit_map
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    ///
    pub fn add_point(&mut self, hit_point: (Point3<Length>, Energy)) {
        self.hit_map.push(hit_point);
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

impl Plottable for HitMap {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        //ray plot series
        let mut x_max = f64::NEG_INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        let xy_pos: Vec<Point2<Length>> = self
            .hit_map
            .iter()
            .map(|p| Point2::new(p.0.x, p.0.y))
            .collect();
        x_max = xy_pos
            .iter()
            .map(|p| p.x.get::<meter>())
            .fold(x_max, |arg0, x| if x.abs() > arg0 { x.abs() } else { arg0 });
        y_max = xy_pos
            .iter()
            .map(|p| p.y.get::<meter>())
            .fold(y_max, |arg0, y| if y.abs() > arg0 { y.abs() } else { arg0 });
        let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
        let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
        let y_prefix = get_prefix_for_base_unit(y_max);
        let x_prefix = get_prefix_for_base_unit(x_max);

        plt_type.set_plot_param(&PlotArgs::YLabel(format!("x position ({y_prefix}m)")))?;
        plt_type.set_plot_param(&PlotArgs::XLabel(format!("y position ({x_prefix}m)")))?;

        let mut plt_series = Vec::<PlotSeries>::with_capacity(1);
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

        plt_series.push(PlotSeries::new(&data, RGBAColor(255, 0, 0, 1.), None));
        x_max *= f64::powi(10., -x_exponent);
        y_max *= f64::powi(10., -y_exponent);

        plt_type.set_plot_param(&PlotArgs::XLim(AxLims::new(-x_max * 1.1, 1.1 * x_max)))?;
        plt_type.set_plot_param(&PlotArgs::YLim(AxLims::new(-y_max * 1.1, 1.1 * y_max)))?;
        Ok(Some(plt_series))
    }
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x position (m)".into()))?
            .set(&PlotArgs::YLabel("y position (m)".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }
}
