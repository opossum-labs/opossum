//! Data structures and plotting routines for ray propagation histories.
#![warn(missing_docs)]

use std::cmp::Ordering;

use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{millimeter, nanometer},
};

use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
    reporting::plottable::{
        AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable,
    },
};

/// Struct that holds the history of ray positions for rays of a specific spectral bin.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RayPositionHistorySpectrum {
    /// Ray history coordinate matrices.
    pub history: Vec<MatrixXx3<Length>>,
    /// central wavelength for polychromatic ray bundles
    pub center_wavelength: Length,
    /// spectral bin size
    pub wavelength_bin_size: Length,
}

impl RayPositionHistorySpectrum {
    /// Creates a new [`RayPositionHistorySpectrum`] struct.
    ///
    /// # Arguments
    /// - `history`: Position history of the ray bundle.
    /// - `center_wavelength`: Nominal wavelength of this ray bundle.
    /// - `wavelength_bin_size`: Wavelength resolution of this ray bundle.
    ///
    /// # Errors
    /// Returns an error if the center wavelength or bin size is non-finite or negative.
    pub fn new(
        history: Vec<MatrixXx3<Length>>,
        center_wavelength: Length,
        wavelength_bin_size: Length,
    ) -> OpmResult<Self> {
        if !center_wavelength.is_normal() || center_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "center wavelength must be finite, non-zero and non-negative!".into(),
            ));
        }
        if !wavelength_bin_size.is_normal() || wavelength_bin_size.is_sign_negative() {
            return Err(OpossumError::Other(
                "wavelength bin size must be finite, non-zero and non-negative!".into(),
            ));
        }
        Ok(Self {
            history,
            center_wavelength,
            wavelength_bin_size,
        })
    }

    /// Returns the ray-position history stored in this struct.
    #[must_use]
    pub const fn get_history(&self) -> &Vec<MatrixXx3<Length>> {
        &self.history
    }

    /// Returns the center wavelength of the rays in this spectral bin.
    #[must_use]
    pub const fn get_center_wavelength(&self) -> &Length {
        &self.center_wavelength
    }

    /// Returns the wavelength bin size.
    #[must_use]
    pub const fn get_wavelength_bin_size(&self) -> &Length {
        &self.wavelength_bin_size
    }

    /// Projects 3D ray positions onto a 2D plane defined by its normal vector.
    ///
    /// # Arguments
    /// * `plane_normal_vec` - Normal vector of the projection plane.
    ///
    /// # Errors
    /// Returns an error if the normal vector has a length of zero.
    pub fn project_to_plane(
        &self,
        plane_normal_vec: Vector3<f64>,
    ) -> OpmResult<Vec<MatrixXx2<Length>>> {
        let vec_norm = plane_normal_vec.norm();

        if vec_norm < f64::EPSILON {
            return Err(OpossumError::Other(
                "The plane normal vector must have a non-zero length!".into(),
            ));
        }

        let normed_normal_vec = plane_normal_vec / vec_norm;

        // Choose projection basis vectors perpendicular to the normal vector
        let (co_ax_1, co_ax_2) = if plane_normal_vec.cross(&Vector3::x()).norm() < f64::EPSILON {
            (Vector3::z(), Vector3::y())
        } else if plane_normal_vec.cross(&Vector3::y()).norm() < f64::EPSILON {
            (Vector3::z(), Vector3::x())
        } else if plane_normal_vec.cross(&Vector3::z()).norm() < f64::EPSILON {
            (Vector3::x(), Vector3::y())
        } else {
            let x_vec = Vector3::x();
            let mut proj_x = x_vec - x_vec.dot(&normed_normal_vec) * plane_normal_vec;
            proj_x /= proj_x.norm();
            (proj_x, proj_x.cross(&normed_normal_vec))
        };

        let mut rays_pos_projection = Vec::<MatrixXx2<Length>>::with_capacity(self.history.len());
        for ray_pos in &self.history {
            let mut projected_ray_pos = MatrixXx2::<Length>::zeros(ray_pos.column(0).len());
            for (row, pos) in ray_pos.row_iter().enumerate() {
                let pos_t = Vector3::from_vec(
                    pos.iter()
                        .map(uom::si::f64::Length::get::<millimeter>)
                        .collect::<Vec<f64>>(),
                );
                let proj_pos = pos_t - pos_t.dot(&normed_normal_vec) * plane_normal_vec;

                projected_ray_pos[(row, 0)] = millimeter!(proj_pos.dot(&co_ax_1));
                projected_ray_pos[(row, 1)] = millimeter!(proj_pos.dot(&co_ax_2));
            }
            rays_pos_projection.push(projected_ray_pos);
        }
        Ok(rays_pos_projection)
    }
}

/// Struct that holds the history of ray positions across all spectral bins for report generation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RayPositionHistories {
    /// Ray position histories grouped per spectral component.
    pub rays_pos_history: Vec<RayPositionHistorySpectrum>,
    /// Optional view direction vector for 2D plane projection.
    pub plot_view_direction: Option<Vector3<f64>>,
    /// Ray color transparency alpha in [0.0, 1.0].
    pub ray_transparency: f64,
}

impl RayPositionHistories {
    /// Returns the center wavelengths of all spectral bins as a vector.
    #[must_use]
    pub fn get_center_wavelengths(&self) -> Vec<Length> {
        self.rays_pos_history
            .iter()
            .map(|r| *r.get_center_wavelength())
            .collect::<Vec<Length>>()
    }
}

impl Plottable for RayPositionHistories {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        let num_series = self.rays_pos_history.len();
        // Show legend by default for polychromatic setups with <= 5 series
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

        // Sort indices by wavelength to preserve spectral colormap order
        let mut sorted_indices: Vec<usize> = (0..num_series).collect();
        sorted_indices.sort_by(|&a, &b| {
            self.rays_pos_history[a]
                .get_center_wavelength()
                .partial_cmp(self.rays_pos_history[b].get_center_wavelength())
                .unwrap_or(Ordering::Equal)
        });

        let min_wvl = self.rays_pos_history[sorted_indices[0]]
            .get_center_wavelength()
            .get::<nanometer>();
        let max_wvl = self.rays_pos_history[sorted_indices[num_series - 1]]
            .get_center_wavelength()
            .get::<nanometer>();
        let wvl_span = max_wvl - min_wvl;

        // Switch to colorbar if there are more than 5 distinct wavelength series
        let use_colorbar = if num_series > 5 {
            plt_type.set_plot_param(&PlotArgs::CBarLabel("wavelength (nm)".into()))?;
            // Add 170 pixels for the colorbar layout
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

            // Set label only if legend is requested, colorbar is not used, and polychromatic
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
