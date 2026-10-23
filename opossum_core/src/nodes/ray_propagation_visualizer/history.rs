//! Data structures and plotting routines for ray propagation histories.
#![warn(missing_docs)]

use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
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
