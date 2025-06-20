#![warn(missing_docs)]
//! Helper functions for easier creation of `standard` ray [`Source`]s.
use super::Source;
use crate::{
    degree,
    energy_distributions::UniformDist,
    error::{OpmResult, OpossumError},
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    optic_node::OpticNode,
    position_distributions::{Grid, Hexapolar},
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
};
use num::Zero;
use uom::si::f64::{Angle, Energy, Length};

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
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(radius, nr_of_rings)?.into(),
        energy_dist: UniformDist::new(energy)?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    });
    let mut src = Source::new("collimated line ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    Ok(src)
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
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Grid::new((Length::zero(), size_y), (1, nr_of_points_y))?.into(),
        energy_dist: UniformDist::new(energy)?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    });
    let mut src = Source::new("collimated line ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    Ok(src)
}
/// Create a point [`Source`] on the optical axis with a given cone angle.
///
/// This is a convenience function, which generates a [`Ray`](crate::ray::Ray) [`Source`] containing a hexapolar, cone-shaped ray bundle at 1000 nm
/// and a given energy. The origin of all [`Rays`](crate::rays::Rays) is at the origin of optical axis (0.0, 0.0, 0.0). The direction of the cone is symmetric along the optical axis
/// in positive direction (z-axis). If the given `cone_angle` is zero, this function generates a [`Source`] a single ray along the optical axis.
///
/// # Errors
///
/// This functions returns an error if
///  - the given energy is < 0.0, Nan, or +inf.
///  - the given angle is < 0.0 degrees or >= 180.0 degrees.
pub fn point_ray_source(cone_angle: Angle, energy: Energy) -> OpmResult<Source> {
    if cone_angle.is_sign_negative() || cone_angle >= degree!(180.0) {
        return Err(OpossumError::Other(
            "cone angle must be within (0.0..180.0) degrees range".into(),
        ));
    }
    let size_after_unit_length = (cone_angle / 2.0).tan().value;
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::PointSrc {
        pos_dist: Hexapolar::new(millimeter!(size_after_unit_length), 3)?.into(),
        energy_dist: UniformDist::new(energy)?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        reference_length: millimeter!(1.0),
    });
    let mut src = Source::new("point ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    Ok(src)
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        degree, joule, lightdata::LightData, millimeter, optic_node::OpticNode,
        properties::Proptype, ray::Ray,
    };
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;
    #[test]
    fn test_round_collimated_ray_source() {
        assert!(round_collimated_ray_source(millimeter!(1.0), joule!(-0.1), 3).is_err());
        assert!(round_collimated_ray_source(millimeter!(1.0), joule!(f64::NAN), 3).is_err());
        assert!(round_collimated_ray_source(millimeter!(1.0), joule!(f64::INFINITY), 3).is_err());
        assert!(round_collimated_ray_source(millimeter!(-0.1), joule!(1.0), 3).is_err());
        let src = round_collimated_ray_source(Length::zero(), joule!(1.0), 3).unwrap();
        if let Proptype::LightDataBuilder(light_data_builder) =
            src.properties().get("light data").unwrap()
        {
            let data = light_data_builder.clone().unwrap().build().unwrap();
            if let LightData::Geometric(rays) = data {
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
        let src = round_collimated_ray_source(millimeter!(1.0), joule!(1.0), 3).unwrap();
        if let Proptype::LightDataBuilder(light_data_builder) =
            src.properties().get("light data").unwrap()
        {
            let data = light_data_builder.clone().unwrap().build().unwrap();
            if let LightData::Geometric(rays) = data {
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
        assert!(point_ray_source(degree!(-0.1), Energy::zero()).is_err());
        assert!(point_ray_source(degree!(180.0), Energy::zero()).is_err());
        assert!(point_ray_source(degree!(190.0), Energy::zero()).is_err());
        let src = point_ray_source(Angle::zero(), joule!(1.0)).unwrap();
        if let Proptype::LightDataBuilder(light_data_builder) =
            src.properties().get("light data").unwrap()
        {
            let data = light_data_builder.clone().unwrap().build().unwrap();
            if let LightData::Geometric(rays) = &data {
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
        let src = point_ray_source(degree!(1.0), joule!(1.0)).unwrap();
        if let Proptype::LightData(data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data {
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
        assert!(collimated_line_ray_source(millimeter!(1.0), joule!(-0.1), 1).is_err());
        assert!(collimated_line_ray_source(millimeter!(1.0), joule!(f64::NAN), 1).is_err());
        assert!(collimated_line_ray_source(millimeter!(1.0), joule!(f64::INFINITY), 1).is_err());
        assert!(collimated_line_ray_source(Length::zero(), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_source(millimeter!(-0.1), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_source(millimeter!(f64::NAN), joule!(1.0), 1).is_err());
        assert!(collimated_line_ray_source(millimeter!(f64::INFINITY), joule!(1.0), 1).is_err());

        assert!(collimated_line_ray_source(millimeter!(1.0), joule!(1.0), 0).is_err());

        let s = collimated_line_ray_source(millimeter!(1.0), joule!(1.0), 2).unwrap();
        if let Proptype::LightData(data) = s.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = &data {
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
                assert_eq!(rays.nr_of_rays(true), 2);
                let ray = rays.iter().collect::<Vec<&Ray>>();
                assert_eq!(ray[0].position(), millimeter!(0., -0.5, 0.));
                assert_eq!(ray[1].position(), millimeter!(0., 0.5, 0.));
            } else {
                panic!("cannot unpack light data property");
            }
        }
    }
}
