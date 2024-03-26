#![warn(missing_docs)]
//!distribution functions which can be applied, for example, for disitributing energies in rays
use nalgebra::Point2;
use uom::si::{angle::radian, f64::Angle};

/// Generate a generalized 2-dimension Gaussian distribution from a vector of input `points`
/// Each point will be assigned the respective value of this Gaussian distribution
/// # Attributes
/// - `points`: Vector of input-point pairs (x, y)
/// - `mu_x`: the mean value in x direction -> Shifts the distribution in x direction to be centered at `mu_x`
/// - `mu_y`: the mean value in y direction -> Shifts the distribution in y direction to be centered at `mu_y`
/// - `sigma_x`: the standard deviation value in x direction
/// - `sigma_y`: the standard deviation value in y direction
/// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
/// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
/// - `rect_flag`: defines if the distribution will be shaped elliptically or rectangularly. Difference between these modes vanishes for power = 1
///
/// # Remarks
/// This function does not check the usefulness of the input arguments,
/// meaning that passing values of NaN, Infinity, zero or negative numbers may result in an unexpected outcome of this function.
/// To avoid non-useful input arguments see [`GeneralGaussian`](../../energy_distributions/general_gaussian/struct.GeneralGaussian.html)
#[must_use]
pub fn general_2d_gaussian(
    points: &[Point2<f64>],
    mu_xy: Point2<f64>,
    sigma_xy: Point2<f64>,
    power: f64,
    theta: Angle,
    rect_flag: bool,
) -> Vec<f64> {
    let mut gaussian = Vec::<f64>::with_capacity(points.len());
    let (sin_theta, cos_theta) = theta.get::<radian>().sin_cos();

    if rect_flag {
        for p in points {
            let x_rot = (p.x - mu_xy.x).mul_add(cos_theta, -((p.y - mu_xy.y) * sin_theta));
            let y_rot = (p.y - mu_xy.y).mul_add(cos_theta, (p.x - mu_xy.x) * sin_theta);
            let g = f64::exp(
                -(0.5 * (x_rot / sigma_xy.x).powi(2)).powf(power)
                    - (0.5 * (y_rot / sigma_xy.y).powi(2)).powf(power),
            );
            gaussian.push(g);
        }
    } else {
        for p in points {
            let x_rot = (p.x - mu_xy.x).mul_add(cos_theta, -((p.y - mu_xy.y) * sin_theta));
            let y_rot = (p.y - mu_xy.y).mul_add(cos_theta, (p.x - mu_xy.x) * sin_theta);
            let g = f64::exp(
                -(0.5
                    * (x_rot / sigma_xy.x)
                        .mul_add(x_rot / sigma_xy.x, (y_rot / sigma_xy.y).powi(2)))
                .powf(power),
            );
            gaussian.push(g);
        }
    }
    gaussian
}
