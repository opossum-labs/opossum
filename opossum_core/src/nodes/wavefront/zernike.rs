#![warn(missing_docs)]
//! 2D polynomial basis decomposition for wavefront error maps.
//!
//! Provides orthogonal basis evaluation and least-squares decomposition via SVD.

use nalgebra::{DMatrix, DVector, Point2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

use crate::{
    error::{OpmResult, OpossumError},
    utils::to_f64,
};

/// Trait defining a 2D basis function set for wavefront expansions.
pub trait Basis2D: Send + Sync {
    /// Family name of the basis system (e.g. "Zernike (Noll)").
    fn basis_name(&self) -> &str;

    /// Number of modes used in this expansion.
    fn mode_count(&self) -> usize;

    /// Evaluates the mode at index `idx` for a given physical position (x, y) in millimeters.
    fn evaluate(&self, idx: usize, point: &Point2<f64>) -> f64;

    /// Returns the descriptive display label of the mode (e.g. "Z4 (Defocus)").
    fn mode_label(&self, idx: usize) -> String;

    /// Indicates whether mode `idx` represents an uncritical static piston / DC offset.
    fn is_piston(&self, _idx: usize) -> bool {
        false
    }

    /// Key-value metadata pairs to be attached to the node report.
    fn report_metadata(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

/// A single decomposed mode with its label, fitted amplitude, and piston flag.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DecomposedMode {
    /// Display label for reporting and UI (e.g. "Z4 (Defocus)").
    pub label: String,
    /// Expansion coefficient in fractions of wavelength λ.
    pub coefficient: f64,
    /// Whether this mode is a global piston / phase offset.
    pub is_piston: bool,
}

/// Generic result of any 2D wavefront polynomial decomposition.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DecompositionResult {
    /// Basis family name.
    pub basis_name: String,
    /// List of fitted modes.
    pub modes: Vec<DecomposedMode>,
    /// Root-mean-square error of the fit residuals in fractions of wavelength λ.
    pub residual_rms: f64,
    /// Basis-specific metadata (e.g. normalization radius).
    pub metadata: Vec<(String, String)>,
}

/// A single mode with Noll index `j`, radial order `n`, azimuthal frequency `m`, and absolute order `m_abs`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZernikeMode {
    /// 1-based Noll index (j = 1, 2, ...).
    pub j: usize,
    /// Radial order n >= 0.
    pub n: usize,
    /// Azimuthal frequency m.
    pub m: i32,
    /// Absolute azimuthal frequency |m| as native usize.
    pub m_abs: usize,
    /// Standard optical name of the aberration if known.
    pub name: String,
}

impl ZernikeMode {
    /// Returns the formatted display label (e.g. "Z4 (Defocus)").
    #[must_use]
    pub fn label(&self) -> String {
        format!("Z{} ({})", self.j, self.name)
    }
}

/// Zernike polynomial expansion using Noll's standard sequential indexing.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ZernikeNoll {
    max_radial_order: usize,
    norm_radius: Length,
    modes: Vec<ZernikeMode>,
}

impl ZernikeNoll {
    /// Creates a Zernike basis up to radial order `max_n` using a normalization radius.
    pub fn new(max_radial_order: usize, norm_radius: Length) -> Self {
        let modes = Self::generate_noll_modes(max_radial_order);
        Self {
            max_radial_order,
            norm_radius,
            modes,
        }
    }

    /// Returns the list of active modes in this basis.
    #[cfg(test)]
    #[must_use]
    pub fn modes(&self) -> &[ZernikeMode] {
        &self.modes
    }

    /// Evaluates the radial polynomial `R_n^|m|(rho)` purely using unsigned indexing.
    fn radial_poly(n: usize, m_abs: usize, rho: f64) -> f64 {
        if rho > 1.0 + 1e-12 || n < m_abs || !(n - m_abs).is_multiple_of(2) {
            return 0.0;
        }
        let rho_clamped = rho.min(1.0);
        let upper = (n - m_abs) / 2;
        let mut sum = 0.0;

        for s in 0..=upper {
            // Avoid (-1.0).powi(s) by checking parity directly
            let sign = if s % 2 == 0 { 1.0 } else { -1.0 };
            let num = sign * factorial(n - s);
            let den =
                factorial(s) * factorial(usize::midpoint(n, m_abs) - s) * factorial(upper - s);

            let power = n - 2 * s;
            let p_i32 = i32::try_from(power).unwrap_or(0);
            sum = (num / den).mul_add(rho_clamped.powi(p_i32), sum);
        }
        sum
    }

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
                let m_signed = i32::try_from(m_abs).unwrap_or(0);

                if m_abs == 0 {
                    modes.push(ZernikeMode {
                        j: current_j,
                        n,
                        m: 0,
                        m_abs: 0,
                        name: Self::standard_name(current_j, n, 0),
                    });
                    current_j += 1;
                } else {
                    let (j_even, j_odd) = if current_j % 2 == 0 {
                        (current_j, current_j + 1)
                    } else {
                        (current_j + 1, current_j)
                    };

                    let mode_cos = ZernikeMode {
                        j: j_even,
                        n,
                        m: m_signed,
                        m_abs,
                        name: Self::standard_name(j_even, n, m_signed),
                    };
                    let mode_sin = ZernikeMode {
                        j: j_odd,
                        n,
                        m: -m_signed,
                        m_abs,
                        name: Self::standard_name(j_odd, n, -m_signed),
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
    (1..=k).fold(1.0, |acc, x| acc * to_f64(x))
}

impl Basis2D for ZernikeNoll {
    fn basis_name(&self) -> &'static str {
        "Zernike"
    }

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
        // Direct access to m_abs as usize without conversions
        let r_nm = Self::radial_poly(mode.n, mode.m_abs, rho);

        let norm_factor = if mode.m == 0 {
            to_f64(mode.n + 1).sqrt()
        } else {
            (2.0 * to_f64(mode.n + 1)).sqrt()
        };

        let m_phi = to_f64(mode.m_abs) * phi;
        if mode.m >= 0 {
            norm_factor * r_nm * m_phi.cos()
        } else {
            norm_factor * r_nm * m_phi.sin()
        }
    }

    fn mode_label(&self, idx: usize) -> String {
        self.modes[idx].label()
    }

    fn is_piston(&self, idx: usize) -> bool {
        self.modes[idx].j == 1
    }

    fn report_metadata(&self) -> Vec<(String, String)> {
        vec![
            (
                "Zernike Norm Radius".into(),
                format!("{:.4} mm", self.norm_radius.get::<millimeter>()),
            ),
            (
                "Zernike Max Order".into(),
                self.max_radial_order.to_string(),
            ),
        ]
    }
}

/// Performs a least-squares fit of arbitrary 2D basis functions onto discrete wavefront points.
pub fn fit_wavefront_points<B: Basis2D>(
    points: &[Point2<f64>],
    opd_values: &[f64],
    basis: &B,
) -> OpmResult<DecompositionResult> {
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

    let svd = design_matrix.clone().svd(true, true);
    let coeffs_vec = svd
        .solve(&observation, 1e-7)
        .map_err(|e| OpossumError::Other(format!("SVD wavefront fit failed: {e}")))?;

    // Residual vector: r = y - M * c
    let fitted = &design_matrix * &coeffs_vec;
    let residuals = &observation - fitted;
    let residual_rms = residuals.norm() / to_f64(n_pts).sqrt();

    let mut modes = Vec::with_capacity(n_modes);
    for (idx, &coeff) in coeffs_vec.iter().enumerate() {
        modes.push(DecomposedMode {
            label: basis.mode_label(idx),
            coefficient: coeff,
            is_piston: basis.is_piston(idx),
        });
    }

    Ok(DecompositionResult {
        basis_name: basis.basis_name().to_string(),
        modes,
        residual_rms,
        metadata: basis.report_metadata(),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;

    #[test]
    fn test_noll_indexing_generation() {
        let basis = ZernikeNoll::new(4, millimeter!(1.0));
        let modes = basis.modes();

        // Standard Noll definitions:
        // j=1: (0,0), j=2: (1,1), j=3: (1,-1), j=4: (2,0), j=5: (2,-2), j=6: (2,2)
        assert_eq!(modes[0].j, 1);
        assert_eq!((modes[0].n, modes[0].m), (0, 0));

        assert_eq!(modes[1].j, 2);
        assert_eq!((modes[1].n, modes[1].m), (1, 1));

        assert_eq!(modes[2].j, 3);
        assert_eq!((modes[2].n, modes[2].m), (1, -1));

        assert_eq!(modes[3].j, 4);
        assert_eq!((modes[3].n, modes[3].m), (2, 0));

        assert_eq!(modes[4].j, 5);
        assert_eq!((modes[4].n, modes[4].m), (2, -2));

        assert_eq!(modes[5].j, 6);
        assert_eq!((modes[5].n, modes[5].m), (2, 2));

        // Up to n=4 there are (4+1)(4+2)/2 = 15 modes
        assert_eq!(modes.len(), 15);
    }
}
