#![warn(missing_docs)]
//! Helper functions for easier creation of `standard` [`RayDataBuilder`].
use crate::{
    degree,
    distributions::{
        energy::UniformDist,
        position::{Grid, Hexapolar},
        spectral::LaserLines,
    },
    error::{OpmResult, OpossumError},
    lightdata::{
        ray_data_builder::RayDataBuilder,
        ray_data_source::{CollimatedSrc, PointSrc, RayDataSource},
    },
    meter, millimeter, nanometer,
};
use nalgebra::Point2;
use num::Zero;
use uom::si::f64::{Angle, Energy, Length};

/// Create a collimated [`RayDataBuilder`].
///
/// This is a convenience function, which generates a [`RayDataBuilder`] consisting of collinear [`Ray`](crate::ray::Ray) bundle symmetrically around the optical axis
/// at 1000 nm and a given energy. The ray distribution is hexapolar with the given number of rings (see [`Hexapolar`] for details). If
/// the `nr_of_rings` is zero, the function genereates a [`RayDataBuilder`] with a single [`Ray`](crate::ray::Ray) on the optical axis.
///
/// # Errors
/// This functions returns an error if
///  - the given energy is ngeative or not finite.
///  - the given radius is negative or not finite.
pub fn round_collimated_ray_builder(
    radius: Length,
    energy: Energy,
    nr_of_rings: u8,
) -> OpmResult<RayDataBuilder> {
    Ok(RayDataSource::Collimated(CollimatedSrc::new(
        Hexapolar::new(radius, nr_of_rings)?.into(),
        UniformDist::new(energy)?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    ))
    .into())
}
/// Create a [`RayDataBuilder`] constructing a line of collimated rays.
///
/// This helper functions creates a ray [`RayDataBuilder`] containing a given number of collimated rays evenly
/// spaced along the `y` axis. (one dimensional [`Grid`]).
/// The grid has the given length (`size_y`) and is centered on the optical axis.
///
/// # Errors
///
/// This function will return an error if the
///   - the energy is ngeative of not finite.
///   - the given `size_y` is negative, zero or not finite.
///   - the given `nr_of_points_y` is zero.
pub fn collimated_line_ray_builder(
    size_y: Length,
    energy: Energy,
    nr_of_points_y: usize,
) -> OpmResult<RayDataBuilder> {
    Ok(RayDataSource::Collimated(CollimatedSrc::new(
        Grid::new(
            Point2::new(Length::zero(), size_y),
            Point2::new(1, nr_of_points_y),
        )?
        .into(),
        UniformDist::new(energy)?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    ))
    .into())
}
/// Create a point [`RayDataBuilder`] on the optical axis with a given cone angle.
///
/// This is a convenience function, which generates a [`RayDataBuilder`] containing a hexapolar, cone-shaped ray bundle at 1000 nm
/// and a given energy. The origin of all [`Rays`](crate::rays::Rays) is at the origin of optical axis (0.0, 0.0, 0.0). The direction of the cone
/// is symmetric along the optical axis in positive direction (z-axis). If the given `cone_angle` is zero, this function generates a
/// a single ray along the optical axis.
///
/// # Errors
///
/// This functions returns an error if
///  - the given energy is < 0.0, Nan, or +inf.
///  - the given angle is < 0.0 degrees or >= 180.0 degrees.
pub fn point_ray_builder(cone_angle: Angle, energy: Energy) -> OpmResult<RayDataBuilder> {
    if cone_angle.is_sign_negative() || cone_angle >= degree!(180.0) {
        return Err(OpossumError::Other(
            "cone angle must be within (0.0..180.0) degrees range".into(),
        ));
    }
    let size_after_unit_length = (cone_angle / 2.0).tan().value;
    Ok(RayDataSource::PointSrc(PointSrc::new(
        Hexapolar::new(meter!(size_after_unit_length), 4)?.into(),
        UniformDist::new(energy)?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        millimeter!(1000.),
    )?)
    .into())
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{degree, joule, millimeter, ray::Ray};
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;
    #[test]
    fn test_round_collimated_ray_source() {
        assert!(round_collimated_ray_builder(millimeter!(1.0), joule!(-0.1), 3).is_err());
        assert!(round_collimated_ray_builder(millimeter!(1.0), joule!(f64::NAN), 3).is_err());
        assert!(round_collimated_ray_builder(millimeter!(1.0), joule!(f64::INFINITY), 3).is_err());
        assert!(round_collimated_ray_builder(millimeter!(-0.1), joule!(1.0), 3).is_err());
        let rays = round_collimated_ray_builder(Length::zero(), joule!(1.0), 3)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(rays.nr_of_rays(true), 1);
        assert_abs_diff_eq!(
            rays.total_energy().get::<joule>(),
            1.0,
            epsilon = 10.0 * f64::EPSILON
        );

        let rays = round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 3)
            .unwrap()
            .build()
            .unwrap();
        assert_abs_diff_eq!(
            rays.total_energy().get::<joule>(),
            1.0,
            epsilon = 10.0 * f64::EPSILON
        );
        assert_eq!(rays.nr_of_rays(true), 37);
    }
    #[test]
    fn test_point_ray_source() {
        assert!(point_ray_builder(degree!(-0.1), Energy::zero()).is_err());
        assert!(point_ray_builder(degree!(180.0), Energy::zero()).is_err());
        assert!(point_ray_builder(degree!(190.0), Energy::zero()).is_err());
        let rays = point_ray_builder(Angle::zero(), joule!(1.0))
            .unwrap()
            .build()
            .unwrap();
        assert_abs_diff_eq!(
            rays.total_energy().get::<joule>(),
            1.0,
            epsilon = 10.0 * f64::EPSILON
        );
        assert_eq!(rays.nr_of_rays(true), 1);

        let rays = point_ray_builder(degree!(1.0), joule!(1.0))
            .unwrap()
            .build()
            .unwrap();
        assert_abs_diff_eq!(
            rays.total_energy().get::<joule>(),
            1.0,
            epsilon = 10.0 * f64::EPSILON
        );
        assert_eq!(rays.nr_of_rays(true), 61);
    }
    #[test]
    fn test_point_ray_source_cone_angle_correctness() {
        let cone_angle = degree!(10.0);
        let rays = point_ray_builder(cone_angle, joule!(1.0))
            .unwrap()
            .build()
            .unwrap();

        let expected_half_angle = (cone_angle / 2.0).value;
        let axis = nalgebra::Vector3::new(0.0, 0.0, 1.0);

        // Collect angles of all rays relative to the axis
        let angles: Vec<f64> = rays
            .iter()
            .map(|ray| {
                let dir = ray.direction().normalize();
                dir.dot(&axis).acos()
            })
            .collect();

        let max_angle = angles.iter().cloned().fold(0., f64::max); // find maximum

        // Assert the maximum ray angle is approximately the expected half-angle
        assert!(
            (max_angle - expected_half_angle).abs() <= 1e-12,
            "maximum ray angle {} does not match expected half-angle {}",
            max_angle,
            expected_half_angle
        );
    }
    #[test]
    fn test_collimated_line_source() {
        assert!(collimated_line_ray_builder(millimeter!(1.0), joule!(-0.1), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(1.0), joule!(f64::NAN), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(1.0), joule!(f64::INFINITY), 1).is_err());
        assert!(collimated_line_ray_builder(Length::zero(), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(-0.1), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(f64::NAN), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(f64::INFINITY), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_builder(millimeter!(1.0), joule!(1.0), 0).is_err());

        let rays = collimated_line_ray_builder(millimeter!(1.0), joule!(1.0), 2)
            .unwrap()
            .build()
            .unwrap();
        assert_abs_diff_eq!(
            rays.total_energy().get::<joule>(),
            1.0,
            epsilon = 10.0 * f64::EPSILON
        );
        assert_eq!(rays.nr_of_rays(true), 2);
        let ray = rays.iter().collect::<Vec<&Ray>>();
        assert_eq!(ray[0].position(), millimeter!(0., -0.5, 0.));
        assert_eq!(ray[1].position(), millimeter!(0., 0.5, 0.));
    }
}
