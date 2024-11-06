//! Parabolic surface
//!
//! This module implements a parabolic surface with a given focal length and a given z position on the optical axis.

use crate::{
    degree,
    error::{OpmResult, OpossumError},
    meter,
    utils::geom_transformation::Isometry,
};
use nalgebra::{vector, Point3, Vector3};
use num::Zero;
use roots::{find_roots_quadratic, Roots};
use uom::si::{
    f64::{Angle, Length, Ratio},
    ratio::ratio,
};

use super::geo_surface::GeoSurface;

#[derive(Clone, Debug)]
/// A parabolic surface with a given focal length and a given z position on the optical axis.
pub struct Parabola {
    focal_length: Length,
    isometry: Isometry,
    off_axis_angles: (Angle, Angle),
}

impl Parabola {
    /// Create a new [`Parabola`] located and oriented by the given [`Isometry`].
    ///
    /// **Note**: A positive focal length leads to a parabolic surface with its "opening" towards the positive z axis.
    ///
    /// # Errors
    ///
    /// This function will return an error if the focal length is 0.0 or not finite.
    pub fn new(focal_length: Length, isometry: &Isometry) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self {
            focal_length,
            isometry: isometry.clone(),
            off_axis_angles: (Angle::zero(), Angle::zero()),
        })
    }
    /// Sets the off-axis angles (full reflection) of this [`Parabola`].
    ///
    /// The `off_axis_angles` tuple denotes the full deflection angle around the x and y axis.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given angles are not finite or their absolute
    /// value is >= 180°.
    pub fn set_off_axis_angles(&mut self, off_axis_angles: (Angle, Angle)) -> OpmResult<()> {
        if !off_axis_angles.0.is_finite() || off_axis_angles.0.abs() >= degree!(180.0) {
            return Err(OpossumError::Other(
                "deflection angle around x axis must be finite and the absolute value < 180°"
                    .into(),
            ));
        }
        if !off_axis_angles.1.is_finite() || off_axis_angles.1.abs() >= degree!(180.0) {
            return Err(OpossumError::Other(
                "deflection angle around y axis must be finite and the absolute value < 180°"
                    .into(),
            ));
        }
        self.off_axis_angles = (off_axis_angles.0, off_axis_angles.1);
        Ok(())
    }
    /// Returns the off axis angles of this [`Parabola`].
    #[must_use]
    pub fn off_axis_angles(&self) -> (Angle, Angle) {
        self.off_axis_angles
    }
    fn calc_oap_decenter(&self) -> (Length, Length) {
        let f_x = 2. * self.focal_length / (Ratio::new::<ratio>(1.) + self.off_axis_angles.0.cos());
        let f_y = 2. * self.focal_length / (Ratio::new::<ratio>(1.) + self.off_axis_angles.1.cos());
        let oad_x = f_y * (self.off_axis_angles.1.sin());
        let oad_y = f_x * (self.off_axis_angles.0.sin());
        (oad_x, oad_y)
    }
}

impl GeoSurface for Parabola {
    fn calc_intersect_and_normal_do(
        &self,
        ray: &crate::ray::Ray,
    ) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        let pos = vector![
            ray.position().x.value,
            ray.position().y.value,
            ray.position().z.value
        ];
        let f_length = self.focal_length.value;
        let is_back_propagating = dir.z.is_sign_negative();
        // parabola formula (at origin)
        // x^2 + y^2 - 4fz = 0
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_y+t*d_y)^2 - 4f*(p_z+t*d_z) = 0
        // This translates into the qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2+d_y^2
        // b = 2* (p_x*d_x + p_y*d_y - 2*f*d_z)
        // c = p_x^2 + p_y^2 - 4f*p_z
        let a = dir.x.mul_add(dir.x, dir.y * dir.y);
        let b = 2. * (2. * f_length).mul_add(-dir.z, pos.x.mul_add(dir.x, pos.y * dir.y));
        let c = (4. * f_length).mul_add(-pos.z, pos.x.mul_add(pos.x, pos.y * pos.y));
        // Solve t of qudaratic equation
        let roots = find_roots_quadratic(a, b, c);
        let intersection_point = match roots {
            // no intersection
            Roots::No(_) => return None,
            // "just touching" intersection
            Roots::One(t) => {
                if t[0] >= 0.0 {
                    pos + t[0] * dir
                } else {
                    return None;
                }
            }
            // "regular" intersection
            Roots::Two(t) => {
                let real_t = if self.focal_length.is_sign_negative() {
                    // concave surface => use max t
                    if is_back_propagating {
                        f64::min(t[0], t[1])
                    } else {
                        f64::max(t[0], t[1])
                    }
                } else {
                    // convex surface => use min t
                    if is_back_propagating {
                        f64::max(t[0], t[1])
                    } else {
                        f64::min(t[0], t[1])
                    }
                };
                if real_t.is_sign_negative() {
                    // surface behind beam
                    return None;
                }
                pos + real_t * dir
            }
            _ => unreachable!(),
        };
        // calc surface normal
        // calculate grad F(x,y,z) =(2 * p_x, 2 * p_y, 4 * f)
        let normal_vector = vector![intersection_point.x, intersection_point.y, -2. * f_length];
        Some((
            meter!(
                intersection_point.x,
                intersection_point.y,
                intersection_point.z
            ),
            normal_vector,
        ))
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    fn set_isometry(&mut self, isometry: &Isometry) {
        let oap_decenter = self.calc_oap_decenter();
        let oap_iso = Isometry::new(
            Point3::new(oap_decenter.0, oap_decenter.1, Length::zero()),
            degree!(0.0, 0.0, 0.0),
        )
        .unwrap();
        let total_iso = isometry.append(&oap_iso);
        self.isometry = total_iso;
    }
}

#[cfg(test)]
mod test {
    use super::Parabola;
    use crate::{
        degree, joule, meter, millimeter, nanometer, ray::Ray, surface::geo_surface::GeoSurface,
        utils::geom_transformation::Isometry,
    };
    use core::f64;
    use nalgebra::vector;
    #[test]
    fn new() {
        assert!(Parabola::new(meter!(0.0), &Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::NAN), &Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::INFINITY), &Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::NEG_INFINITY), &Isometry::identity()).is_err());
        let p = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        assert_eq!(p.off_axis_angles.0, degree!(0.0));
        assert_eq!(p.off_axis_angles.1, degree!(0.0));
    }
    #[test]
    fn intersect() {
        let parabola = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        let ray = Ray::new_collimated(meter!(-1.0, -1.0, -10.0), nanometer!(1000.0), joule!(1.0))
            .unwrap();
        let intersection = parabola.calc_intersect_and_normal_do(&ray).unwrap();
        assert_eq!(intersection.0, meter!(-1., -1., 0.5));
        assert_eq!(intersection.1, vector![-1., -1., -2.]);
    }
    #[test]
    fn intersect_ray_through_focus() {
        let parabola = Parabola::new(meter!(-1.0), &Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 1. - 0.25];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_some());
    }
    #[test]
    fn intersect_touching() {
        let parabola = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, 0.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        let (i_point, r_point) = parabola.calc_intersect_and_normal_do(&ray).unwrap();
        assert_eq!(i_point.x, meter!(0.0));
        assert_eq!(i_point.y, meter!(0.0));
        assert_eq!(i_point.z, meter!(0.0));
        assert_eq!(r_point.normalize(), vector!(0.0, 0.0, -1.0));
    }
    #[test]
    fn intersect_not() {
        let parabola = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_none());

        let direction = vector![0.0, 0.0, -1.0];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_none());
    }
    #[test]
    fn set_off_axis_angles() {
        let mut parabola = Parabola::new(millimeter!(50.0), &Isometry::identity()).unwrap();
        assert!(parabola
            .set_off_axis_angles((degree!(f64::NAN), degree!(0.0)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(f64::INFINITY), degree!(0.0)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(f64::NEG_INFINITY), degree!(0.0)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(180.0), degree!(0.0)))
            .is_err());

        assert!(parabola
            .set_off_axis_angles((degree!(0.0), degree!(f64::NAN)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(0.0), degree!(f64::INFINITY)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(0.0), degree!(f64::NEG_INFINITY)))
            .is_err());
        assert!(parabola
            .set_off_axis_angles((degree!(0.0), degree!(180.0)))
            .is_err());

        parabola.set_off_axis_angles((degree!(10.0), degree!(15.0))).unwrap();
        assert_eq!(parabola.off_axis_angles(), (degree!(10.0), degree!(15.0)));
    }
    #[test]
    fn isometry() {
        let parabola = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        assert_eq!(
            parabola.isometry(),
            &Isometry::new_along_z(meter!(0.0)).unwrap()
        );
    }
    #[test]
    fn set_isometry() {
        let mut parabola = Parabola::new(meter!(1.0), &Isometry::identity()).unwrap();
        parabola.set_isometry(&Isometry::new_along_z(meter!(0.5)).unwrap());
        assert_eq!(
            parabola.isometry(),
            &Isometry::new_along_z(meter!(0.5)).unwrap()
        );
    }
}
