//! Flat surface
//!
//! An infinitely large and perfectly flat 2D surface
use super::GeoSurface;
use crate::{light::Ray, meter};
use nalgebra::{Point2, Point3, Vector3};
use num_traits::Zero;
use uom::si::f64::Length;

/// An infinitely large and perfectly flat surface: the xy plane of its vertex frame.
///
/// Its surface normal is collinear to the local z axis and points against the ray.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plane;

impl GeoSurface for Plane {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let pos = ray.position();
        let dir = ray.direction();

        // A ray parallel to the plane only intersects if it starts *on* the plane.
        if dir.z.is_zero() {
            return if pos.z.value.is_zero() {
                // Ray is on the plane, intersection is its current position.
                Some((pos, Vector3::new(0.0, 0.0, -dir.z.signum())))
            } else {
                // Ray is parallel but not on the plane, so no intersection.
                None
            };
        }

        // 2. Calculate the intersection parameter 't'.
        // Intersection with plane at z=0 happens at t = -pos.z / dir.z
        let t = -pos.z.value / dir.z;

        // 3. Check if the intersection is behind the ray's origin.
        // If t is negative, the plane is "behind" the ray, so no intersection.
        if t < 0.0 {
            return None;
        }

        // 4. Calculate the intersection point and normal vector.
        let intersection_point = pos.map(|c| c.value) + t * dir;
        let normal = Vector3::new(0.0, 0.0, -dir.z.signum());

        Some((
            meter!(
                intersection_point.x,
                intersection_point.y,
                intersection_point.z
            ),
            normal,
        ))
    }
    fn local_z_at(&self, _transversal_position: &Point2<Length>) -> Option<Length> {
        // The local surface *is* the xy plane, so it lies at z = 0 everywhere and reaches
        // arbitrarily far out.
        Some(Length::zero())
    }
    fn is_behind(&self, point: &Point3<Length>) -> bool {
        // The local surface is the xy plane at z = 0.
        point.z >= Length::zero()
    }
    fn reaches(&self, _distance: Length) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "plane"
    }
}

#[cfg(test)]
mod test {
    use crate::{
        degree,
        error::OpmResult,
        geometry::{Face, SurfaceShape},
        joule,
        light::Ray,
        millimeter, nanometer,
        utils::geom_transformation::Isometry,
    };
    use nalgebra::Vector3;

    /// A plane with its vertex frame at the given isometry.
    fn plane_at(isometry: Isometry) -> Face {
        Face::new(SurfaceShape::Plane, isometry)
    }
    #[test]
    fn is_behind() -> OpmResult<()> {
        let s = plane_at(Isometry::new_along_z(millimeter!(10.0))?);
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 10.1)));
        // a point exactly on the surface counts as behind it
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 10.0)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, 9.9)));
        // the plane is unbounded, so the transversal position does not matter
        assert!(s.is_behind(&millimeter!(100.0, -50.0, 10.1)));
        assert!(!s.is_behind(&millimeter!(100.0, -50.0, 9.9)));
        Ok(())
    }
    #[test]
    fn is_behind_tilted() -> OpmResult<()> {
        // a plane tilted by 45 degrees around the y axis: its normal lies in the xz plane
        let iso = Isometry::new(millimeter!(0.0, 0.0, 0.0), degree!(0.0, 45.0, 0.0))?;
        let s = plane_at(iso);
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 1.0)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, -1.0)));
        // along the tilted surface itself the point stays on the surface
        assert!(s.is_behind(&millimeter!(1.0, 0.0, 1.0)));
        assert!(!s.is_behind(&millimeter!(-1.0, 0.0, 0.9)));
        Ok(())
    }
    #[test]
    fn intersect_on_axis() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = plane_at(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 0.0, 10.0), -Vector3::z()))
        );
        Ok(())
    }
    #[test]
    fn intersect_on_axis_behind() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(-10.0))?;
        let s = plane_at(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_zero_dist() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(0.0))?;
        let s = plane_at(iso);
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 0.0, 0.0), -Vector3::z()))
        );
        Ok(())
    }
    #[test]
    fn intersect_off_axis() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = plane_at(iso);
        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 1.0), nanometer!(1053.0), joule!(1.0))?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 1.0, 10.0), -Vector3::z()))
        );
        let ray = Ray::new(
            millimeter!(0.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 11.0, 10.0), -Vector3::z()))
        );
        Ok(())
    }
    #[test]
    fn intersect_on_axis_backwards() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = plane_at(iso);
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 20.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 0.0, 10.0), Vector3::z()))
        );
        Ok(())
    }
}
