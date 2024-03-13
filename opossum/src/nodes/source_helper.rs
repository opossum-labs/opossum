#![warn(missing_docs)]
//! Helper functions for easier creation of `standard` ray [`Source`]s.
use super::Source;
use crate::{
    error::OpmResult,
    lightdata::LightData,
    position_distributions::{Grid, Hexapolar},
    rays::Rays,
};
use nalgebra::Point3;
use num::Zero;
use uom::si::{
    f64::{Angle, Energy, Length},
    length::nanometer,
};

/// Create a collimated ray [`Source`].
///
/// This is a convenience function, which generates a [`Source`] consisting of collinear [`Ray`](crate::ray::Ray) bundle symmetrically around the optical axis
/// at 1000 nm and a given energy. The ray distribution is hexapolar with the given number of rings (see [`Hexapolar`] for details). If
/// the `nr_of_rings` is zero, the function genereates a [`Source`] with a single [`Ray`](crate::ray::Ray) on the optical axis.
///
/// # Errors
/// This functions returns an error if
///  - the given energy is ngeative or not finite.
///  - the given radius is negative or not finite.
pub fn round_collimated_ray_source(
    radius: Length,
    energy: Energy,
    nr_of_rings: u8,
) -> OpmResult<Source> {
    let rays = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1000.0),
        energy,
        &Hexapolar::new(radius, nr_of_rings)?,
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("collimated ray source", &light))
}
/// Create a [`Source`] containing a line of collimated rays.
///
/// This functions creates a ray [`Source`] containing a given number of collimated rays evenly spaced along the `y` axis. (one dimensional [`Grid`]).
/// The grid has the given length (`size_y`) and is centered on the optical axis.
///
/// # Errors
///
/// This function will return an error if the
///   - the energy is ngeative of not finite.
///   - the given `size_y` is negative, zero or not finite.
///   - the given `nr_of_points_y` is zero.
pub fn collimated_line_ray_source(
    size_y: Length,
    energy: Energy,
    nr_of_points_y: usize,
) -> OpmResult<Source> {
    let rays = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1000.0),
        energy,
        &Grid::new((Length::zero(), size_y), (1, nr_of_points_y))?,
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("collimated line ray source", &light))
}
/// Create a point [`Source`] on the optical axis with a given cone angle.
///
/// This is a convenience function, which generates a [`Ray`](crate::ray::Ray) [`Source`] containing a hexapolar, cone-shaped ray bundle at 1000 nm
/// and a given energy. The origin of all [`Rays`] is at the origin of optical axis (0.0, 0.0, 0.0). The direction of the cone is symmetric along the optical axis
/// in positive direction (z-axis). If the given `cone_angle` is zero, this function generates a [`Source`] a single ray along the optical axis.
///
/// # Errors
///
/// This functions returns an error if
///  - the given energy is < 0.0, Nan, or +inf.
///  - the given angle is < 0.0 degrees or >= 180.0 degrees.
pub fn point_ray_source(cone_angle: Angle, energy: Energy) -> OpmResult<Source> {
    let rays = Rays::new_hexapolar_point_source(
        Point3::origin(),
        cone_angle,
        3,
        Length::new::<nanometer>(1000.0),
        energy,
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("point ray source", &light))
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{optical::Optical, properties::Proptype, ray::Ray};
    use approx::assert_abs_diff_eq;
    use uom::si::{angle::degree, energy::joule, length::millimeter};
    #[test]
    fn test_round_collimated_ray_source() {
        assert!(round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(-0.1),
            3
        )
        .is_err());
        assert!(round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::NAN),
            3
        )
        .is_err());
        assert!(round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::INFINITY),
            3
        )
        .is_err());
        assert!(round_collimated_ray_source(
            Length::new::<millimeter>(-0.1),
            Energy::new::<joule>(1.0),
            3
        )
        .is_err());
        let src =
            round_collimated_ray_source(Length::zero(), Energy::new::<joule>(1.0), 3).unwrap();
        if let Proptype::LightData(light_data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &light_data.value {
                assert_eq!(rays.nr_of_rays(true), 1);
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
            } else {
                panic!("no LightData::Geometric found")
            }
        } else {
            panic!("property light data has wrong type");
        }
        let src = round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(1.0),
            3,
        )
        .unwrap();
        if let Proptype::LightData(data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data.value {
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
                assert_eq!(rays.nr_of_rays(true), 37);
            } else {
                panic!("error unpacking data");
            }
        } else {
            panic!("error unpacking data");
        }
    }
    #[test]
    fn test_point_ray_source() {
        assert!(point_ray_source(Angle::new::<degree>(-0.1), Energy::zero()).is_err());
        assert!(point_ray_source(Angle::new::<degree>(180.0), Energy::zero()).is_err());
        assert!(point_ray_source(Angle::new::<degree>(190.0), Energy::zero()).is_err());
        let src = point_ray_source(Angle::zero(), Energy::new::<joule>(1.0)).unwrap();
        if let Proptype::LightData(data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data.value {
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
                assert_eq!(rays.nr_of_rays(true), 1);
            } else {
                panic!("cannot unpack light data property");
            }
        } else {
            panic!("cannot unpack light data property");
        }
        let src = point_ray_source(Angle::new::<degree>(1.0), Energy::new::<joule>(1.0)).unwrap();
        if let Proptype::LightData(data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data.value {
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
                assert_eq!(rays.nr_of_rays(true), 37);
            } else {
                panic!("cannot unpack light data property");
            }
        }
    }
    #[test]
    fn test_collimated_line_source() {
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(-0.1),
            1
        )
        .is_err());
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::NAN),
            1
        )
        .is_err());
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::INFINITY),
            1
        )
        .is_err());
        assert!(collimated_line_ray_source(Length::zero(), Energy::new::<joule>(1.0), 1).is_err());
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(-0.1),
            Energy::new::<joule>(1.0),
            1
        )
        .is_err());
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(f64::NAN),
            Energy::new::<joule>(1.0),
            1
        )
        .is_err());
        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(f64::INFINITY),
            Energy::new::<joule>(1.0),
            1
        )
        .is_err());

        assert!(collimated_line_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(1.0),
            0
        )
        .is_err());

        let s = collimated_line_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(1.0),
            2,
        )
        .unwrap();
        if let Proptype::LightData(data) = s.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data.value {
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
                assert_eq!(rays.nr_of_rays(true), 2);
                let ray = rays.iter().collect::<Vec<&Ray>>();
                assert_eq!(
                    ray[0].position(),
                    Point3::new(
                        Length::zero(),
                        Length::new::<millimeter>(-0.5),
                        Length::zero()
                    )
                );
                assert_eq!(
                    ray[1].position(),
                    Point3::new(
                        Length::zero(),
                        Length::new::<millimeter>(0.5),
                        Length::zero()
                    )
                );
            } else {
                panic!("cannot unpack light data property");
            }
        }
    }
}
