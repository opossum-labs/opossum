#![warn(missing_docs)]
//! distribution functions which can be applied, for example, for disitributing energies in rays
use std::f64::consts::PI;

use nalgebra::Point2;
use num::Zero;
use uom::si::{Dimension, Quantity, Units, angle::radian, f64::Angle};

use crate::error::OpmResult;

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
/// To avoid non-useful input arguments see [`General"dGaussian`](crate::energy_distributions::general_gaussian::General2DGaussian).
#[must_use]
pub fn general_2d_super_gaussian_points(
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
            gaussian.push(general_2d_super_gaussian_point_rectangular(
                p, mu_xy, sigma_xy, power, sin_theta, cos_theta,
            ));
        }
    } else {
        for p in points {
            gaussian.push(general_2d_super_gaussian_point_elliptical(
                p, mu_xy, sigma_xy, power, sin_theta, cos_theta,
            ));
        }
    }
    gaussian
}

/// Get the value of a point at position `point` of a generalized 2-dimension Gaussian distribution with rectangular shape
/// Each point will be assigned the respective value of this Gaussian distribution
/// # Attributes
/// - `points`: Vector of input-point pairs (x, y)
/// - `mu_x`: the mean value in x direction -> Shifts the distribution in x direction to be centered at `mu_x`
/// - `mu_y`: the mean value in y direction -> Shifts the distribution in y direction to be centered at `mu_y`
/// - `sigma_x`: the standard deviation value in x direction
/// - `sigma_y`: the standard deviation value in y direction
/// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
/// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
///
/// # Remarks
/// This function does not check the usefulness of the input arguments,
/// meaning that passing values of NaN, Infinity, zero or negative numbers may result in an unexpected outcome of this function.
/// To avoid non-useful input arguments see [`GeneralGaussian`](../../energy_distributions/general_gaussian/struct.GeneralGaussian.html)
#[must_use]
pub fn general_2d_super_gaussian_point_rectangular(
    point: &Point2<f64>,
    mu_xy: Point2<f64>,
    sigma_xy: Point2<f64>,
    power: f64,
    sin_theta: f64,
    cos_theta: f64,
) -> f64 {
    let x_rot = (point.x - mu_xy.x).mul_add(cos_theta, -((point.y - mu_xy.y) * sin_theta));
    let y_rot = (point.y - mu_xy.y).mul_add(cos_theta, (point.x - mu_xy.x) * sin_theta);
    f64::exp(
        -(0.5 * (x_rot / sigma_xy.x).powi(2)).powf(power)
            - (0.5 * (y_rot / sigma_xy.y).powi(2)).powf(power),
    )
}

/// Calculates the value at a point, corresponding to a 2d Gaussian distribution
///
/// This function calculates the values of a Gaussian distribution using quantities. For example, a fluence distribution.
///
/// # Attributes
/// - `peak_val`: peak value of quantity Q1 of the gaussian distribution at its center
/// - `p`: reference to a `Point2` of quantity Q2 at which the Gaussian should be evaluated      
/// - `mu`: Center position of the Gaussian in x and y. `Point2` of quantity Q2
/// - `sigma`: Standard deviation of the Gaussian in x and y. `Point2` of quantity Q2
/// - `theta`: rotation angle of the Gaussian.
#[must_use]
pub fn quantity_2d_gaussian_point<D1, D2, U1, U2>(
    peak_val: Quantity<D1, U1, f64>,
    p: &Point2<Quantity<D2, U2, f64>>,
    mu: Point2<Quantity<D2, U2, f64>>,
    sigma: Point2<Quantity<D2, U2, f64>>,
    theta: Angle,
) -> Quantity<D1, U1, f64>
where
    D2: Dimension + ?Sized + 'static,
    <D2 as Dimension>::Kind: uom::marker::Mul,
    D1: Dimension + ?Sized + 'static,
    <D1 as Dimension>::Kind: uom::marker::Mul,
    U1: Units<f64> + ?Sized + 'static,
    U2: Units<f64> + ?Sized + 'static,
{
    let sin_theta = theta.sin().value;
    let cos_theta = theta.cos().value;
    let x_rot =
        (p.x.value - mu.x.value).mul_add(cos_theta, -((p.y.value - mu.y.value) * sin_theta));
    let y_rot = (p.y.value - mu.y.value).mul_add(cos_theta, (p.x.value - mu.x.value) * sin_theta);
    let exp_val = (-0.5
        * (x_rot / sigma.x.value).mul_add(x_rot / sigma.x.value, (y_rot / sigma.y.value).powi(2)))
    .exp();
    peak_val * exp_val
}

/// Get the value of a point at position `point` of a generalized 2-dimension Gaussian distribution
/// Each point will be assigned the respective value of this Gaussian distribution
/// # Attributes
/// - `points`: Vector of input-point pairs (x, y)
/// - `mu_x`: the mean value in x direction -> Shifts the distribution in x direction to be centered at `mu_x`
/// - `mu_y`: the mean value in y direction -> Shifts the distribution in y direction to be centered at `mu_y`
/// - `sigma_x`: the standard deviation value in x direction
/// - `sigma_y`: the standard deviation value in y direction
/// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
/// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
///
/// # Remarks
/// This function does not check the usefulness of the input arguments,
/// meaning that passing values of NaN, Infinity, zero or negative numbers may result in an unexpected outcome of this function.
/// To avoid non-useful input arguments see [`GeneralGaussian`](../../energy_distributions/general_gaussian/struct.GeneralGaussian.html)
#[must_use]
pub fn general_2d_super_gaussian_point_elliptical(
    point: &Point2<f64>,
    mu_xy: Point2<f64>,
    sigma_xy: Point2<f64>,
    power: f64,
    sin_theta: f64,
    cos_theta: f64,
) -> f64 {
    let x_rot = (point.x - mu_xy.x).mul_add(cos_theta, -((point.y - mu_xy.y) * sin_theta));
    let y_rot = (point.y - mu_xy.y).mul_add(cos_theta, (point.x - mu_xy.x) * sin_theta);

    f64::exp(
        -(0.5 * (x_rot / sigma_xy.x).mul_add(x_rot / sigma_xy.x, (y_rot / sigma_xy.y).powi(2)))
            .powf(power),
    )
}

/// Generate a 1-dimensional Gaussian distribution from a vector of input `points`
/// Each point will be assigned the respective value of this Gaussian distribution
/// # Attributes
/// - `points`: Vector of input-point pairs (x, y)
/// - `mu`: the mean value -> Shifts the distribution to be centered at `mu`
/// - `fwhm`: the full-width at half maximum of the distribution
/// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
///
/// # Remarks
/// This function does not check the usefulness of the input arguments,
/// meaning that passing values of NaN, Infinity, zero or negative numbers may result in an unexpected outcome of this function.
#[must_use]
pub fn gaussian(points: &[f64], mu: f64, fwhm: f64, power: f64) -> Vec<f64> {
    let mut gaussian = Vec::<f64>::with_capacity(points.len());
    let sigma = fwhm / (2. * (2. * (f64::ln(2.)).powf(1. / power)).sqrt());
    for p in points {
        let g = f64::exp(-(0.5 * ((p - mu) / sigma).powi(2)).powf(power));
        gaussian.push(g);
    }
    gaussian
}

/// Creates Points that lie on a circle with given radius and center
///
/// # Errors
/// This function returns an error if
///   - the `center_point` components are not finite.
///   - the `radii` are not finite.
pub fn ellipse(
    center_point: (f64, f64),
    radii: (f64, f64),
    num_points: u32,
) -> OpmResult<Vec<Point2<f64>>> {
    if !center_point.0.is_finite() || !center_point.1.is_finite() {
        return Err(crate::error::OpossumError::Other(
            "center point coordinates must be finite".into(),
        ));
    }
    if !radii.0.is_finite() || !radii.1.is_finite() {
        return Err(crate::error::OpossumError::Other(
            "radii must be finite".into(),
        ));
    }
    if num_points.is_zero() {
        return Err(crate::error::OpossumError::Other(
            "num_points must be > 0".into(),
        ));
    }
    let mut xy_data = Vec::<Point2<f64>>::with_capacity(num_points as usize);
    let angle_step = 2. * PI / f64::from(num_points);
    for point_num in 0..num_points {
        let angle = f64::from(point_num) * angle_step;
        xy_data.push(Point2::new(
            radii.0.mul_add(f64::cos(angle), center_point.0),
            radii.1.mul_add(f64::sin(angle), center_point.1),
        ));
    }
    Ok(xy_data)
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_abs_diff_eq;
    use nalgebra::point;
    #[test]
    fn test_gaussian() {
        let x_values = vec![-1.0, 0.0, 1.0, 2.0];
        let y_values = gaussian(&x_values, 1.0, 2.0, 1.0);
        assert_eq!(y_values, vec![0.0625, 0.5, 1.0, 0.5]);
    }
    #[test]
    fn ellipse_wrong() {
        for val in vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ellipse((val, 0.0), (0.0, 0.0), 1).is_err());
            assert!(ellipse((val, 0.0), (0.0, 0.0), 1).is_err());
            assert!(ellipse((val, 0.0), (0.0, 0.0), 1).is_err());

            assert!(ellipse((0.0, val), (0.0, 0.0), 1).is_err());
            assert!(ellipse((0.0, val), (0.0, 0.0), 1).is_err());
            assert!(ellipse((0.0, val), (0.0, 0.0), 1).is_err());

            assert!(ellipse((0.0, 0.0), (val, 0.0), 1).is_err());
            assert!(ellipse((0.0, 0.0), (val, 0.0), 1).is_err());
            assert!(ellipse((0.0, 0.0), (val, 0.0), 1).is_err());

            assert!(ellipse((0.0, 0.0), (0.0, val), 1).is_err());
            assert!(ellipse((0.0, 0.0), (0.0, val), 1).is_err());
            assert!(ellipse((0.0, 0.0), (0.0, val), 1).is_err());
        }
        assert!(ellipse((0.0, 0.0), (0.0, 0.0), 0).is_err());
    }
    #[test]
    fn ellipse_ok() {
        let points = ellipse((1.0, 2.0), (1.0, 2.0), 4).unwrap();
        assert_eq!(points.len(), 4);
        assert_abs_diff_eq!(points[0], point![2.0, 2.0]);
        assert_abs_diff_eq!(points[1], point![1.0, 4.0]);
        assert_abs_diff_eq!(points[2], point![0.0, 2.0], epsilon = 2. * f64::EPSILON);
        assert_abs_diff_eq!(points[3], point![1.0, 0.0]);
    }
}
