#![warn(missing_docs)]
//! Spot diagram data structures and plotting routines.

use crate::{
    error::OpmResult,
    light::Rays,
    reporting::plottable::{
        AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable,
    },
    utils::{
        geom_transformation::Isometry,
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
};
use nalgebra::{DVector, MatrixXx2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{meter, nanometer},
};

/// A single spectral series containing 2D spot coordinates at a specific wavelength.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WavelengthSpotSeries {
    /// Nominal wavelength for this spectral bin.
    pub wavelength: Length,
    /// 2D positions (x, y) of ray hits on the detector surface.
    pub spots: Vec<(Length, Length)>,
}

impl WavelengthSpotSeries {
    /// Creates a new spectral spot series.
    #[must_use]
    pub fn new(wavelength: Length, spots: Vec<(Length, Length)>) -> Self {
        Self { wavelength, spots }
    }
}

/// Compact 2D spot diagram data binned by wavelength.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SpotDiagramData {
    /// Spectral series containing ray spots.
    pub series: Vec<WavelengthSpotSeries>,
}

impl SpotDiagramData {
    /// Creates a new [`SpotDiagramData`].
    #[must_use]
    pub const fn new(series: Vec<WavelengthSpotSeries>) -> Self {
        Self { series }
    }

    /// Extracts spot diagram data from a ray bundle, grouped into wavelength bins.
    ///
    /// # Errors
    /// Returns an error if ray splitting by wavelength fails.
    pub fn from_rays(rays: &Rays, iso: &Isometry, wavelength_bin_width: Length) -> OpmResult<Self> {
        if rays.is_empty() {
            return Ok(Self::default());
        }

        let (split_rays_bundles, wavelengths) =
            rays.split_ray_bundle_by_wavelength(wavelength_bin_width, true)?;

        let mut series = Vec::with_capacity(split_rays_bundles.len());
        for (bundle, &wvl) in split_rays_bundles.iter().zip(wavelengths.iter()) {
            let xy_pos = bundle.get_xy_rays_pos(true, iso);
            let mut spots = Vec::with_capacity(xy_pos.nrows());
            for row in 0..xy_pos.nrows() {
                spots.push((xy_pos[(row, 0)], xy_pos[(row, 1)]));
            }
            series.push(WavelengthSpotSeries {
                wavelength: wvl,
                spots,
            });
        }

        Ok(Self { series })
    }

    /// Returns the total number of ray spots across all spectral bins.
    #[must_use]
    pub fn total_spots(&self) -> usize {
        self.series.iter().map(|s| s.spots.len()).sum()
    }

    /// Returns `true` if no spots are contained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.series.is_empty() || self.total_spots() == 0
    }
}

impl Plottable for SpotDiagramData {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x position (m)".into()))?
            .set(&PlotArgs::YLabel("y position (m)".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.is_empty() {
            return Ok(None);
        }

        let num_series = self.series.len();
        let use_colorbar = if num_series > 5 {
            plt_type.set_plot_param(&PlotArgs::CBarLabel("wavelength (nm)".into()))?;
            plt_type.set_plot_param(&PlotArgs::PlotSize((970, 800)))?;
            plt_type.set_plot_param(&PlotArgs::ZLim(AxLims::new(
                self.series[0].wavelength.get::<nanometer>(),
                self.series[num_series - 1].wavelength.get::<nanometer>(),
            )))?;
            true
        } else {
            false
        };

        let color_grad = colorous::TURBO;
        let wvl_span =
            (self.series[num_series - 1].wavelength - self.series[0].wavelength).get::<nanometer>();
        let wvl_range = if num_series <= 1 || wvl_span.abs() < f64::EPSILON {
            1.0
        } else {
            wvl_span * 2.0
        };

        let mut x_max = f64::NEG_INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        for s in &self.series {
            for &(x, y) in &s.spots {
                x_max = x_max.max(x.get::<meter>().abs());
                y_max = y_max.max(y.get::<meter>().abs());
            }
        }

        let min_window = self.series[0].wavelength.get::<meter>() / 2.0;
        x_max = x_max.max(min_window);
        y_max = y_max.max(min_window);

        let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
        let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
        let x_prefix = get_prefix_for_base_unit(x_max);
        let y_prefix = get_prefix_for_base_unit(y_max);

        plt_type.set_plot_param(&PlotArgs::XLabel(format!("x in {x_prefix}m")))?;
        plt_type.set_plot_param(&PlotArgs::YLabel(format!("y in {y_prefix}m")))?;

        let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

        for s in &self.series {
            if s.spots.is_empty() {
                continue;
            }

            let grad_val =
                0.42 + (s.wavelength - self.series[0].wavelength).get::<nanometer>() / wvl_range;
            let rgbcolor = color_grad.eval_continuous(grad_val);

            let x_vals: Vec<f64> = s
                .spots
                .iter()
                .map(|(x, _)| get_unit_value_as_length_with_format_by_exponent(*x, x_exponent))
                .collect();
            let y_vals: Vec<f64> = s
                .spots
                .iter()
                .map(|(_, y)| get_unit_value_as_length_with_format_by_exponent(*y, y_exponent))
                .collect();

            let data = PlotData::Dim2 {
                xy_data: MatrixXx2::from_columns(&[
                    DVector::from_vec(x_vals),
                    DVector::from_vec(y_vals),
                ]),
            };

            let series_label = if legend && !use_colorbar {
                Some(format!("{:.1} nm", s.wavelength.get::<nanometer>()))
            } else {
                None
            };

            plt_series.push(PlotSeries::new(
                &data,
                RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, 1.0),
                series_label,
            ));
        }

        x_max *= f64::powi(10.0, -x_exponent);
        y_max *= f64::powi(10.0, -y_exponent);

        plt_type.set_plot_param(&PlotArgs::XLim(AxLims::new(-x_max * 1.1, 1.1 * x_max)))?;
        plt_type.set_plot_param(&PlotArgs::YLim(AxLims::new(-y_max * 1.1, 1.1 * y_max)))?;

        match plt_type {
            PlotType::Scatter2D(_) => Ok(Some(plt_series)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{distributions::position::Hexapolar, joule, millimeter, nanometer};

    #[test]
    fn spot_data_creation_and_bounds() -> OpmResult<()> {
        let rays = Rays::new_uniform_collimated(
            nanometer!(800.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.0), 2)?,
        )?;

        let iso = Isometry::identity();
        let data = SpotDiagramData::from_rays(&rays, &iso, nanometer!(1.0))?;

        assert_eq!(data.series.len(), 1);
        assert!(data.total_spots() > 0);
        assert!(!data.is_empty());
        assert_eq!(data.series[0].wavelength.get::<nanometer>(), 800.0);

        let mut plt_params = PlotParameters::default();
        data.add_plot_specific_params(&mut plt_params)?;
        let mut plt_type = data.get_plot_type(&plt_params);
        let series = data.get_plot_series(&mut plt_type, false)?;
        assert!(series.is_some());
        assert_eq!(series.unwrap().len(), 1);
        Ok(())
    }
}
