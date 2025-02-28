//! Flat surface
//!
//! An infinitely large and perfectly flat 2D surface
use super::geo_surface::GeoSurface;
use crate::{meter, ray::Ray, utils::geom_transformation::Isometry};
use nalgebra::{Point3, Vector3};
use num::Zero;
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// An infinitely large and perfectly flat surface
///
/// By default (using `Isometry::identity()`), the surface is oriented
/// with its normal along the optical axis (= xy surface) and positioned at the origin.
/// In addition, the surface normal vector is collinear to the optical axis but
/// pointing to the negative z direction: `vector(0.0, 0.0, -1.0)`.
pub struct Plane {
    isometry: Isometry,
}
impl Plane {
    /// Create a new [`Plane`].
    ///
    /// The located and orientation is defined by the given [`Isometry`]. By default
    /// (using `Isometry::identity()`), the surface is oriented with its normal along the
    /// optical axis (= xy surface) and positioned at the origin (z=0)
    #[must_use]
    pub const fn new(isometry: Isometry) -> Self {
        Self { isometry }
    }
}
impl Default for Plane {
    /// Create a new [`Plane`] aligned in the xy plane at position z = 0.
    fn default() -> Self {
        Self {
            isometry: Isometry::default(),
        }
    }
}
impl GeoSurface for Plane {
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let mut trans_pos_in_m = ray.position().map(|c| c.value);
        let trans_dir = ray.direction();
        // Check, if ray position is on the surface, then directly return position as intersection point
        if !trans_pos_in_m.z.is_zero() {
            let distance_in_z_direction = -trans_pos_in_m.z;
            if distance_in_z_direction.signum() != trans_dir.z.signum() {
                // Ray propagates away from the plane => no intersection
                return None;
            }
            let length_in_ray_dir = distance_in_z_direction / trans_dir.z;
            trans_pos_in_m += length_in_ray_dir * trans_dir;
        }
        Some((
            meter!(trans_pos_in_m.x, trans_pos_in_m.y, trans_pos_in_m.z),
            Vector3::new(0.0, 0.0, -1.0 * trans_dir.z.signum()),
        ))
    }
    fn set_isometry(&mut self, isometry: &Isometry) {
        self.isometry = isometry.clone();
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    fn name(&self) -> String {
        "plane".into()
    }
}
// impl Color for Plane {
//     fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
//         Vector3::new(0.3, 0.3, 0.3)
//     }
// }
// impl SDF for Plane {
//     fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
//         let p_out = self.isometry.inverse_transform_point_f64(p);
//         p_out.x.mul_add(self.normal.x, p_out.y * self.normal.y)
//             + p_out.z.mul_add(self.normal.z, self.shift.value)
//     }
// }
// impl Render<'_> for Plane {}
// impl Renderable<'_> for Plane {}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{degree, joule, millimeter, nanometer};
    #[test]
    fn default() {
        let p = Plane::default();
        let t = p.isometry.translation_vec();
        assert_eq!(t.x, millimeter!(0.0));
        assert_eq!(t.y, millimeter!(0.0));
        assert_eq!(t.z, millimeter!(0.0));
        let r = p.isometry.rotation();
        assert_eq!(r.x, degree!(0.0));
        assert_eq!(r.y, degree!(0.0));
        assert_eq!(r.z, degree!(0.0));
    }
    #[test]
    fn new() {
        let iso = Isometry::new_along_z(millimeter!(1.0)).unwrap();
        let p = Plane::new(iso);
        let t = p.isometry.translation_vec();
        assert_eq!(t.x, millimeter!(0.0));
        assert_eq!(t.y, millimeter!(0.0));
        assert_eq!(t.z, millimeter!(1.0));
    }
    #[test]
    fn set_isometry() {
        let mut p = Plane::default();
        let iso = Isometry::new_along_z(millimeter!(1.0)).unwrap();
        p.set_isometry(&iso);
        let t = p.isometry.translation_vec();
        assert_eq!(t.x, millimeter!(0.0));
        assert_eq!(t.y, millimeter!(0.0));
        assert_eq!(t.z, millimeter!(1.0));
    }
    #[test]
    fn intersect_on_axis() {
        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let s = Plane::new(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 0.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
    #[test]
    fn intersect_on_axis_behind() {
        let iso = Isometry::new_along_z(millimeter!(-10.0)).unwrap();
        let s = Plane::new(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_zero_dist() {
        let iso = Isometry::new_along_z(millimeter!(0.0)).unwrap();
        let s = Plane::new(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
    #[test]
    fn intersect_off_axis() {
        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let s = Plane::new(iso);
        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 1.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 1.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
        let ray = Ray::new(
            millimeter!(0.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 11.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
}
