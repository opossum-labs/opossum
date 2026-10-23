//! Plottable implementations for core optical data types.

use super::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable};
use colorous;
use nalgebra::{DMatrix, DVector, DVectorView, Matrix2xX, MatrixXx2, MatrixXx3, Point2, Point3};
use opossum_core::{
    analyzers::ghostfocus::GhostFocusHistory,
    apertures::{Aperture, ApertureShape, CircleShape},
    core_optics::hit_map::HitMap,
    error::{OpmResult, OpossumError},
    light::Spectrum,
    meter,
    nodes::{
        SpotDiagramData, WaveFrontMap, fluence_detector::fluence_data::FluenceData,
        ray_propagation_visualizer::RayPositionHistories,
    },
    prelude::Isometry,
    utils::{
        griddata::{create_linspace_axes, grid_interpolate_3d_scatter_data, linspace},
        math_distribution_functions::ellipse,
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
};
use plotters::style::RGBAColor;
use uom::si::{
    f64::Length,
    length::{meter, millimeter, nanometer},
    radiant_exposure::joule_per_square_centimeter,
};

// --- FluenceData ---

impl Plottable for FluenceData {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x position (mm)".into()))?
            .set(&PlotArgs::YLabel("y position (mm)".into()))?
            .set(&PlotArgs::CBarLabel("fluence (J/cm²)".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?
            .set(&PlotArgs::ExpandBounds(false))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?;

        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::ColorMesh(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let (nrows, ncols) = self.interp_distribution().shape();

        match plt_type {
            PlotType::ColorMesh(_) => {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: linspace(
                        self.x_range().start.get::<millimeter>(),
                        self.x_range().end.get::<millimeter>(),
                        self.interp_distribution().ncols(),
                    )?,
                    y_dat_m: linspace(
                        self.y_range().start.get::<millimeter>(),
                        self.y_range().end.get::<millimeter>(),
                        self.interp_distribution().nrows(),
                    )?,
                    z_dat_nxm: DMatrix::from_iterator(
                        nrows,
                        ncols,
                        self.interp_distribution()
                            .iter()
                            .map(uom::si::f64::RadiantExposure::get::<joule_per_square_centimeter>),
                    ),
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }
}

// --- SpotDiagramData ---

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

// --- WaveFrontMap ---

impl Plottable for WaveFrontMap {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        let min_x = self.x().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("y position in mm".into()))?
                .set(&PlotArgs::CBarLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::ExpandBounds(false))?
                .set(&PlotArgs::AxisEqual(true))?
                .set(&PlotArgs::PlotAutoSize(true))?;
        } else if range_x > eps {
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        } else if range_y > eps {
            plt_params
                .set(&PlotArgs::XLabel("y position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        }
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        let min_x = self.x().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            let mut plt_type = PlotType::ColorMesh(plt_params.clone());
            let legend = plt_params.get_legend_flag().unwrap_or(false);

            if let Some(plt_series) = &self.get_plot_series(&mut plt_type, legend).unwrap_or(None)
                && !plt_series.is_empty()
            {
                let ranges = plt_series[0].define_data_based_axes_bounds(false);
                let z_bounds = ranges
                    .get_z_bounds()
                    .unwrap_or_else(|| AxLims::new(-0.5e-3, 0.5e-3).unwrap());
                if z_bounds.min > -1e-3 && z_bounds.max < 1e-3 {
                    _ = plt_type.set_plot_param(&PlotArgs::ZLim(Some(AxLims {
                        min: -1e-3,
                        max: 1e-3,
                    })));
                }
            }
            plt_type
        } else {
            PlotType::Line2D(plt_params.clone())
        }
    }

    fn get_plot_series(
        &self,
        _plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let min_x = self.x().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y().iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y().iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x <= eps && range_y <= eps {
            log::warn!("Wavefront data has zero dimension in X and Y. Cannot plot.");
            return Ok(None);
        }

        if range_x <= eps || range_y <= eps {
            let mut data: Vec<(f64, f64)> = if range_x > eps {
                self.x()
                    .iter()
                    .zip(self.opd().iter())
                    .map(|(&x, &z)| (x, z))
                    .collect()
            } else {
                self.y()
                    .iter()
                    .zip(self.opd().iter())
                    .map(|(&y, &z)| (y, z))
                    .collect()
            };

            data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut mat = MatrixXx2::zeros(data.len());
            for (i, (coord, val)) in data.iter().enumerate() {
                mat[(i, 0)] = *coord;
                mat[(i, 1)] = *val;
            }

            let plt_series = PlotSeries::new(
                &PlotData::Dim2 { xy_data: mat },
                RGBAColor(255, 0, 0, 1.),
                None,
            );
            return Ok(Some(vec![plt_series]));
        }

        if let (Ok((x_interp, _)), Ok((y_interp, _))) = (
            create_linspace_axes(
                DVectorView::from(&DVector::from_vec(self.x().to_vec())),
                100,
            ),
            create_linspace_axes(
                DVectorView::from(&DVector::from_vec(self.y().to_vec())),
                100,
            ),
        ) {
            let scattered_data = MatrixXx3::from_columns(&[
                DVector::from_vec(self.x().to_vec()),
                DVector::from_vec(self.y().to_vec()),
                DVector::from_vec(self.opd().to_vec()),
            ]);
            if let Ok((interp_dat, _)) =
                grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)
            {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: x_interp,
                    y_dat_m: y_interp,
                    z_dat_nxm: interp_dat,
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            } else {
                log::warn!(
                    "Could not create interpolated wavefront map for plotting! Returning no plot data."
                );
                Ok(None)
            }
        } else {
            log::warn!("Could not create axes from provided data! Returning no plot data.");
            Ok(None)
        }
    }
}

// --- HitMap ---

impl Plottable for HitMap {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.hit_map().is_empty() {
            Ok(None)
        } else {
            let mut plt_series = Vec::<PlotSeries>::with_capacity(self.hit_map().len());
            let mut xy_positions = Vec::<Vec<Point2<Length>>>::with_capacity(self.hit_map().len());
            let mut x_max = f64::NEG_INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            let mut x_min = f64::INFINITY;
            let mut y_min = f64::INFINITY;

            for (i, bounced_ray_bundles) in self.hit_map().iter().enumerate() {
                xy_positions.push(Vec::<Point2<Length>>::new());
                for rays_hitmap in bounced_ray_bundles.hit_map().values() {
                    for p in rays_hitmap.hit_map().positions() {
                        xy_positions[i].push(Point2::new(p.x, p.y));
                        x_max = x_max.max(p.x.value);
                        y_max = y_max.max(p.y.value);
                        x_min = x_min.min(p.x.value);
                        y_min = y_min.min(p.y.value);
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
                let c = if self.hit_map().len() > 10 {
                    gradient.eval_rational(i, self.hit_map().len())
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

            x_max *= f64::powi(10.0, -x_exponent);
            y_max *= f64::powi(10.0, -y_exponent);
            x_min *= f64::powi(10.0, -x_exponent);
            y_min *= f64::powi(10.0, -y_exponent);

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

// --- Spectrum ---

impl Plottable for Spectrum {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = self.data();
        let mut spec_mat = MatrixXx2::zeros(data.len());
        for (i, s) in data.iter().enumerate() {
            spec_mat[(i, 0)] = s.0 * 1000.0;
            spec_mat[(i, 1)] = s.1;
        }
        match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) | PlotType::Histogram2D(_) => {
                let plt_series = PlotSeries::new(
                    &PlotData::Dim2 { xy_data: spec_mat },
                    RGBAColor(255, 0, 0, 1.),
                    None,
                );
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }

    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("wavelength in nm".into()))?
            .set(&PlotArgs::YLabel("spectrum in arb. units".into()))?
            .set(&PlotArgs::PlotSize((1200, 800)))?
            .set(&PlotArgs::AxisEqual(false))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Histogram2D(plt_params.clone())
    }
}

// --- RayPositionHistories ---

impl Plottable for RayPositionHistories {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        let num_series = self.rays_pos_history.len();
        let show_legend = num_series > 1 && num_series <= 5;

        plt_params
            .set(&PlotArgs::XLabel("position in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("position in mm (y axis)".into()))?
            .set(&PlotArgs::PlotSize((1200, 1200)))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::Legend(show_legend))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::MultiLine2D(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.rays_pos_history.is_empty() {
            return Ok(None);
        }

        let num_series = self.rays_pos_history.len();

        let mut sorted_indices: Vec<usize> = (0..num_series).collect();
        sorted_indices.sort_by(|&a, &b| {
            self.rays_pos_history[a]
                .get_center_wavelength()
                .partial_cmp(self.rays_pos_history[b].get_center_wavelength())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let min_wvl = self.rays_pos_history[sorted_indices[0]]
            .get_center_wavelength()
            .get::<nanometer>();
        let max_wvl = self.rays_pos_history[sorted_indices[num_series - 1]]
            .get_center_wavelength()
            .get::<nanometer>();
        let wvl_span = max_wvl - min_wvl;

        let use_colorbar = if num_series > 5 {
            plt_type.set_plot_param(&PlotArgs::CBarLabel("wavelength (nm)".into()))?;
            plt_type.set_plot_param(&PlotArgs::PlotSize((1370, 1200)))?;

            let z_lim = if wvl_span.abs() < f64::EPSILON {
                AxLims::new(min_wvl - 0.5, max_wvl + 0.5)
            } else {
                AxLims::new(min_wvl, max_wvl)
            };
            plt_type.set_plot_param(&PlotArgs::ZLim(z_lim))?;
            plt_type.set_plot_param(&PlotArgs::Legend(false))?;
            true
        } else {
            false
        };

        let color_grad = colorous::TURBO;
        let wvl_range = if num_series <= 1 || wvl_span.abs() < f64::EPSILON {
            1.0
        } else {
            wvl_span * 2.0
        };

        let Some(plot_view_direction) = self.plot_view_direction else {
            return Err(OpossumError::Other(
                "cannot get plot series for raypropagationvisualizer, plot_view_direction not defined".into(),
            ));
        };

        let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

        for &idx in &sorted_indices {
            let ray_pos_hist = &self.rays_pos_history[idx];
            let wvl = ray_pos_hist.get_center_wavelength().get::<nanometer>();
            let grad_val = 0.42 + (wvl - min_wvl) / wvl_range;
            let rgbcolor = color_grad.eval_continuous(grad_val);

            let projected_positions = ray_pos_hist.project_to_plane(plot_view_direction)?;
            let mut proj_pos_mm = Vec::<MatrixXx2<f64>>::with_capacity(projected_positions.len());
            for ray_pos in &projected_positions {
                proj_pos_mm.push(MatrixXx2::from_vec(
                    ray_pos
                        .iter()
                        .map(uom::si::f64::Length::get::<millimeter>)
                        .collect::<Vec<f64>>(),
                ));
            }

            let plt_data = PlotData::MultiDim2 {
                vec_of_xy_data: proj_pos_mm,
            };

            let series_label = if legend && !use_colorbar && num_series > 1 {
                Some(format!("{wvl:.1} nm"))
            } else {
                None
            };

            plt_series.push(PlotSeries::new(
                &plt_data,
                RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, self.ray_transparency),
                series_label,
            ));
        }

        match plt_type {
            PlotType::MultiLine2D(_) => Ok(Some(plt_series)),
            _ => Ok(None),
        }
    }
}

// --- GhostFocusHistory ---

impl Plottable for GhostFocusHistory {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("position in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("position in mm (y axis)".into()))?
            .set(&PlotArgs::PlotSize((1200, 1200)))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::Legend(false))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::MultiLine2D(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        _plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.rays_pos_history.is_empty() {
            return Ok(None);
        }

        let Some(plot_view_direction) = self.plot_view_direction else {
            return Err(OpossumError::Other(
                "cannot get plot series for raypropagationvisualizer, plot_view_direction not defined".into(),
            ));
        };

        let projected_positions = self.project_to_plane(plot_view_direction)?;
        let mut plt_series = Vec::<PlotSeries>::with_capacity(projected_positions.len());

        for (i, bounce_positions) in projected_positions.iter().enumerate() {
            let mut proj_pos_mm = Vec::<MatrixXx2<f64>>::with_capacity(projected_positions.len());
            for rays_in_bounce in bounce_positions {
                for ray_pos in rays_in_bounce {
                    proj_pos_mm.push(MatrixXx2::from_vec(
                        ray_pos
                            .iter()
                            .map(uom::si::f64::Length::get::<millimeter>)
                            .collect::<Vec<f64>>(),
                    ));
                }
            }
            let gradient = colorous::TURBO;

            let c = if projected_positions.len() > 10 {
                gradient.eval_rational(i, projected_positions.len())
            } else {
                colorous::CATEGORY10[i]
            };

            let plt_data = PlotData::MultiDim2 {
                vec_of_xy_data: proj_pos_mm,
            };
            let series_label = format!("Bounce: {i}");

            plt_series.push(PlotSeries::new(
                &plt_data,
                RGBAColor(c.r, c.g, c.b, 0.2),
                Some(series_label),
            ));
        }
        Ok(Some(plt_series))
    }
}

// --- Aperture ---

fn plot_circle(conf: CircleShape, isometry: Option<&Isometry>) -> OpmResult<Vec<PlotSeries>> {
    let (trans_x, trans_y) = isometry.map_or((0.0, 0.0), |iso| {
        (
            iso.translation().x.get::<millimeter>(),
            iso.translation().y.get::<millimeter>(),
        )
    });

    let circle_points = ellipse(
        (trans_x, trans_y),
        (
            conf.radius().get::<millimeter>(),
            conf.radius().get::<millimeter>(),
        ),
        100,
    )?;
    let plt_dat = PlotData::Dim2 {
        xy_data: Matrix2xX::from_vec(
            circle_points
                .iter()
                .flat_map(|p| vec![p.x, p.y])
                .collect::<Vec<f64>>(),
        )
        .transpose(),
    };
    Ok(vec![PlotSeries::new(
        &plt_dat,
        RGBAColor(0, 0, 0, 1.),
        Some("Aperture".to_owned()),
    )])
}

impl Plottable for Aperture {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let iso = self.isometry();
        let plt_series_opt = match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) => match self.shape() {
                ApertureShape::Open => None,
                ApertureShape::BinaryCircle(conf) => Some(plot_circle(*conf, iso)?),
                ApertureShape::BinaryRectangle(conf) => {
                    let mut points = [
                        Point2::new(conf.width() / 2., conf.height() / 2.),
                        Point2::new(-conf.width() / 2., conf.height() / 2.),
                        Point2::new(-conf.width() / 2., -conf.height() / 2.),
                        Point2::new(conf.width() / 2., -conf.height() / 2.),
                    ];
                    for p in &mut points {
                        if let Some(iso) = iso {
                            let transformed_p =
                                iso.transform_point(&Point3::new(p.x, p.y, meter!(0.0)));
                            p.x = transformed_p.x;
                            p.y = transformed_p.y;
                        }
                    }
                    let plt_data_points = points
                        .iter()
                        .flat_map(|p| vec![p.x.get::<millimeter>(), p.y.get::<millimeter>()])
                        .collect::<Vec<f64>>();
                    let plt_dat = PlotData::Dim2 {
                        xy_data: Matrix2xX::<f64>::from_vec(plt_data_points).transpose(),
                    };

                    let series_label = if legend {
                        Some("Aperture".to_owned())
                    } else {
                        None
                    };
                    Some(vec![PlotSeries::new(
                        &plt_dat,
                        RGBAColor(0, 0, 0, 1.),
                        series_label,
                    )])
                }
                ApertureShape::BinaryPolygon(conf) => {
                    let mut xy_data = MatrixXx2::from_element(conf.points().len(), 0.);
                    for (row, p) in conf.points().iter().enumerate() {
                        xy_data[(row, 0)] = p.x.get::<millimeter>();
                        xy_data[(row, 1)] = p.y.get::<millimeter>();
                    }
                    Some(vec![PlotSeries::new(
                        &PlotData::Dim2 { xy_data },
                        RGBAColor(0, 0, 0, 1.),
                        Some("Aperture".to_owned()),
                    )])
                }
                ApertureShape::Gaussian(conf) => {
                    let (trans_x, trans_y) = iso.map_or((0.0, 0.0), |iso| {
                        (
                            iso.translation().x.get::<millimeter>(),
                            iso.translation().y.get::<millimeter>(),
                        )
                    });

                    let circle_points = ellipse(
                        (trans_x, trans_y),
                        (
                            conf.sigma().0.get::<millimeter>() * 2.,
                            conf.sigma().1.get::<millimeter>() * 2.,
                        ),
                        100,
                    )?;
                    let xy_data = Matrix2xX::from_vec(
                        circle_points
                            .iter()
                            .flat_map(|p| vec![p.x, p.y])
                            .collect::<Vec<f64>>(),
                    )
                    .transpose();
                    Some(vec![PlotSeries::new(
                        &PlotData::Dim2 { xy_data },
                        RGBAColor(0, 0, 0, 1.),
                        Some("Gaussian Aperture 2-sigma".to_owned()),
                    )])
                }
                ApertureShape::Stack(conf) => {
                    let mut aperture_series_vec =
                        Vec::<PlotSeries>::with_capacity(conf.apertures().len());
                    for aperture in conf.apertures() {
                        if let Some(plt_series_vec) = aperture.get_plot_series(plt_type, legend)? {
                            aperture_series_vec.extend(plt_series_vec);
                        }
                    }
                    Some(aperture_series_vec)
                }
            },
            _ => None,
        };

        Ok(plt_series_opt)
    }

    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("position in mm".into()))?
            .set(&PlotArgs::YLabel("position in mm".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Line2D(plt_params.clone())
    }
}
