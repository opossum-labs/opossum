#![warn(missing_docs)]
//! 2D polynomial basis decomposition for wavefront error maps.
//!
//! Provides orthogonal basis evaluation and least-squares decomposition via SVD.

use nalgebra::{DMatrix, DVector, Point2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

use crate::error::{OpmResult, OpossumError};

/// Trait defining a 2D basis function set for wavefront expansions.
///
/// Designed to be extensible for Hermite-Gaussian, Legendre or Hankel bases in future iterations.
pub trait Basis2D: Send + Sync {
    /// Number of modes used in this expansion.
    fn mode_count(&self) -> usize;

    /// Evaluates the mode at index `idx` for a given physical position (x, y) in millimeters.
    fn evaluate(&self, idx: usize, point: &Point2<f64>) -> f64;

    /// Returns the descriptive name and indices of the mode (e.g. "Z_4 (Defocus)").
    fn mode_label(&self, idx: usize) -> String;
}

/// A single mode with Noll index `j`, radial order `n`, and azimuthal frequency `m`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ZernikeMode {
    /// 1-based Noll index (j = 1, 2, ...).
    pub j: usize,
    /// Radial order n >= 0.
    pub n: usize,
    /// Azimuthal frequency m with |m| <= n and (n - |m|) even.
    pub m: i32,
    /// Standard optical name of the aberration if known.
    pub name: String,
}

/// Zernike polynomial expansion using Noll's standard sequential indexing.
///
/// Normalization follows Noll (1976), where modes are orthonormal over the unit disk:
/// - sqrt(n + 1) for m == 0
/// - sqrt(2 * (n + 1)) for m != 0
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ZernikeNoll {
    max_radial_order: usize,
    norm_radius: Length,
    modes: Vec<ZernikeMode>,
}

impl ZernikeNoll {
    /// Creates a Zernike basis up to radial order `max_n` using a normalization radius in mm.
    pub fn new(max_radial_order: usize, norm_radius: Length) -> Self {
        let modes = Self::generate_noll_modes(max_radial_order);
        Self {
            max_radial_order,
            norm_radius,
            modes,
        }
    }

    /// Returns the normalization radius in millimeters.
    #[must_use]
    pub const fn norm_radius(&self) -> Length {
        self.norm_radius
    }

    /// Returns the list of active modes in this basis.
    #[must_use]
    pub fn modes(&self) -> &[ZernikeMode] {
        &self.modes
    }

    /// Evaluates the radial polynomial R_n^|m|(rho).
    fn radial_poly(n: usize, m_abs: usize, rho: f64) -> f64 {
        if rho > 1.0 + 1e-12 {
            return 0.0;
        }
        let rho_clamped = rho.min(1.0);
        let n_i = n as i32;
        let m_i = m_abs as i32;
        let upper = (n_i - m_i) / 2;
        let mut sum = 0.0;

        for s in 0..=upper {
            let num = (-1.0f64).powi(s) * factorial((n_i - s) as usize);
            let den = factorial(s as usize)
                * factorial(((n_i + m_i) / 2 - s) as usize)
                * factorial(((n_i - m_i) / 2 - s) as usize);
            sum += (num / den) * rho_clamped.powi(n_i - 2 * s);
        }
        sum
    }

    /// Deterministically generates (n, m) tuples according to Noll's ordering rule.
    fn generate_noll_modes(max_n: usize) -> Vec<ZernikeMode> {
        let mut modes = Vec::new();
        let mut current_j = 1;

        for n in 0..=max_n {
            let mut m_magnitudes = Vec::new();
            for m in 0..=n {
                if (n - m) % 2 == 0 {
                    m_magnitudes.push(m);
                }
            }

            for m_abs in m_magnitudes {
                if m_abs == 0 {
                    modes.push(ZernikeMode {
                        j: current_j,
                        n,
                        m: 0,
                        name: Self::standard_name(current_j, n, 0),
                    });
                    current_j += 1;
                } else {
                    // For |m| > 0: two modes with indices current_j and current_j + 1.
                    // Noll convention: even j -> cos (m > 0), odd j -> sin (m < 0).
                    let (j_even, j_odd) = if current_j % 2 == 0 {
                        (current_j, current_j + 1)
                    } else {
                        (current_j + 1, current_j)
                    };

                    let mode_cos = ZernikeMode {
                        j: j_even,
                        n,
                        m: m_abs as i32,
                        name: Self::standard_name(j_even, n, m_abs as i32),
                    };
                    let mode_sin = ZernikeMode {
                        j: j_odd,
                        n,
                        m: -(m_abs as i32),
                        name: Self::standard_name(j_odd, n, -(m_abs as i32)),
                    };

                    if current_j % 2 == 0 {
                        modes.push(mode_cos);
                        modes.push(mode_sin);
                    } else {
                        modes.push(mode_sin);
                        modes.push(mode_cos);
                    }
                    current_j += 2;
                }
            }
        }
        modes
    }

    /// Standard optical names for lower order aberrations.
    fn standard_name(j: usize, n: usize, m: i32) -> String {
        match j {
            1 => "Piston".into(),
            2 => "Tilt X".into(),
            3 => "Tilt Y".into(),
            4 => "Defocus".into(),
            5 => "Astigmatism 45°".into(),
            6 => "Astigmatism 0°".into(),
            7 => "Coma X".into(),
            8 => "Coma Y".into(),
            9 => "Trefoil 30°".into(),
            10 => "Trefoil 0°".into(),
            11 => "Spherical".into(),
            _ => format!("Z{j} (n={n}, m={m})"),
        }
    }
}

fn factorial(k: usize) -> f64 {
    (1..=k).map(|x| x as f64).product::<f64>().max(1.0)
}

impl Basis2D for ZernikeNoll {
    fn mode_count(&self) -> usize {
        self.modes.len()
    }

    fn evaluate(&self, idx: usize, point: &Point2<f64>) -> f64 {
        let mode = &self.modes[idx];
        let rho = point.coords.norm() / self.norm_radius.get::<millimeter>();
        if rho > 1.0 + 1e-12 {
            return 0.0;
        }

        let phi = point.y.atan2(point.x);
        let r_nm = Self::radial_poly(mode.n, mode.m.unsigned_abs() as usize, rho);

        // Noll orthonormalization factor
        let norm_factor = if mode.m == 0 {
            ((mode.n + 1) as f64).sqrt()
        } else {
            (2.0 * (mode.n + 1) as f64).sqrt()
        };

        if mode.m >= 0 {
            norm_factor * r_nm * (mode.m as f64 * phi).cos()
        } else {
            norm_factor * r_nm * (mode.m.unsigned_abs() as f64 * phi).sin()
        }
    }

    fn mode_label(&self, idx: usize) -> String {
        let m = &self.modes[idx];
        format!("Z_{} ({})", m.j, m.name)
    }
}

/// Result of a wavefront polynomial decomposition.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DecompositionResult {
    /// Fitted coefficient for each mode (in fractions of wavelength λ).
    pub coefficients: Vec<f64>,
    /// Mode descriptors corresponding to the coefficients.
    pub modes: Vec<ZernikeMode>,
    /// Root mean square (RMS) of the fit residuals (in λ).
    pub residual_rms: f64,
    /// Normalization radius
    pub norm_radius: Length,
}

/// Performs a least-squares fit of arbitrary 2D basis functions onto discrete wavefront points.
pub fn fit_wavefront_points<B: Basis2D>(
    points: &[Point2<f64>],
    opd_values: &[f64],
    basis: &B,
) -> OpmResult<Vec<f64>> {
    let n_pts = points.len();
    let n_modes = basis.mode_count();

    if n_pts < n_modes {
        return Err(OpossumError::Other(format!(
            "Insufficient wavefront points ({n_pts}) to fit {n_modes} basis modes."
        )));
    }

    let mut design_matrix = DMatrix::zeros(n_pts, n_modes);
    let observation = DVector::from_column_slice(opd_values);

    for (row_idx, pt) in points.iter().enumerate() {
        for col_idx in 0..n_modes {
            design_matrix[(row_idx, col_idx)] = basis.evaluate(col_idx, pt);
        }
    }

    let svd = design_matrix.svd(true, true);
    let coeffs = svd
        .solve(&observation, 1e-7)
        .map_err(|e| OpossumError::Other(format!("SVD wavefront fit failed: {e}")))?;

    Ok(coeffs.as_slice().to_vec())
}
