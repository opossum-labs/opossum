#![warn(missing_docs)]
//! Module for handling bounded bodies.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is an unbounded interface: it answers
//! where a [`Ray`] hits it, but it has no notion of an inside. A [`Body`] is the complementary
//! concept — a closed volume that knows which points lie within it and how far a [`Ray`] travels
//! through it. This is the domain any volumetric quantity (such as an inversion density) is defined
//! on.
//!
//! The only implementation is [`SurfaceBoundedBody`], a volume bounded by an entrance surface, an
//! exit surface and a transversal [`Aperture`]. Since the bounding surfaces are the ones a node
//! already uses for refraction, the usual optical volumes are simply different parameter sets of
//! the same body: a disk is two [`Plane`](crate::geometry::Plane)s with a circular aperture, a slab
//! two planes with a rectangular one, a lens two [`Sphere`](crate::geometry::Sphere)s.

use crate::{
    apertures::Aperture,
    error::OpmResult,
    geometry::geo_surface::GeoSurfaceRef,
    light::Ray,
    utils::{LockExt, geom_transformation::Isometry, math_utils::distance_3d_point},
};
use nalgebra::Point3;
use std::fmt::Debug;
use uom::si::f64::Length;

/// Trait for handling closed bodies.
///
/// A body is the volumetric counterpart of a
/// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface): it defines an inside and can state how
/// far a [`Ray`] travels within it.
pub trait Body: Debug + Send + Sync {
    /// Return whether the given point lies inside this [`Body`].
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, given in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the point lies inside the body. The body is half-open along the direction of
    /// propagation: a point lying exactly on the entrance boundary counts as inside, one lying
    /// exactly on the exit boundary does not. This way neighbouring bodies sharing a surface do not
    /// both claim the points on it.
    ///
    /// # Errors
    ///
    /// This function returns an error if an internal mutex of a bounding surface cannot be locked.
    fn contains(&self, point: &Point3<Length>) -> OpmResult<bool>;
    /// Return the geometrical path length of the given [`Ray`] inside this [`Body`].
    ///
    /// The ray is not refracted on the way — the returned length is the straight chord between the
    /// point where the ray enters the body (or its starting point, if it already starts inside) and
    /// the point where it leaves the body again.
    ///
    /// # Arguments
    ///
    /// - `ray`: the ray to be traced through the body.
    ///
    /// # Returns
    ///
    /// The path length inside the body or `None` if the ray does not pass through it.
    ///
    /// # Errors
    ///
    /// This function returns an error if an internal mutex of a bounding surface cannot be locked.
    fn path_length_inside(&self, ray: &Ray) -> OpmResult<Option<Length>>;
}

/// A [`Body`] bounded by an entrance surface, an exit surface and a transversal [`Aperture`].
///
/// A point is inside this body if it lies behind the entrance surface (see
/// [`is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind)), not behind the exit surface
/// and within the aperture. Both bounding surfaces are held as [`GeoSurfaceRef`]s, i.e. the body
/// shares them with whoever else holds them rather than copying their geometry.
///
/// **Note**: The lateral boundary is the aperture alone — it is not a surface rays can interact
/// with. A ray leaving the volume sideways is therefore reported as not passing through the body
/// rather than being reflected on a barrel surface.
#[derive(Debug, Clone)]
pub struct SurfaceBoundedBody {
    entrance: GeoSurfaceRef,
    exit: GeoSurfaceRef,
    aperture: Aperture,
    isometry: Isometry,
}

impl SurfaceBoundedBody {
    /// Create a new [`SurfaceBoundedBody`].
    ///
    /// # Arguments
    ///
    /// - `entrance`: the surface bounding the body towards -z
    /// - `exit`: the surface bounding the body towards +z
    /// - `aperture`: the transversal boundary of the body
    /// - `isometry`: the frame the aperture is defined in, usually the isometry of the node the
    ///   body belongs to
    #[must_use]
    pub const fn new(
        entrance: GeoSurfaceRef,
        exit: GeoSurfaceRef,
        aperture: Aperture,
        isometry: Isometry,
    ) -> Self {
        Self {
            entrance,
            exit,
            aperture,
            isometry,
        }
    }
    /// Return whether the given point lies within the transversal aperture of this body.
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, given in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the aperture transmits at the transversal position of the given point.
    fn is_within_aperture(&self, point: &Point3<Length>) -> bool {
        let local_point = self.isometry.inverse_transform_point(point);
        self.aperture.apodize(&local_point) > 0.0
    }
    /// Intersect the given [`Ray`] with one of the bounding surfaces of this body.
    ///
    /// Hits outside the transversal aperture are discarded, since they lie on the unbounded part of
    /// the surface which is not part of the body.
    ///
    /// # Arguments
    ///
    /// - `surface`: the bounding surface to intersect with
    /// - `ray`: the ray to be intersected
    ///
    /// # Returns
    ///
    /// The intersection point in global coordinates or `None` if the ray misses the bounded part of
    /// the surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the internal mutex of the given surface cannot be locked.
    fn bounded_intersection(
        &self,
        surface: &GeoSurfaceRef,
        ray: &Ray,
    ) -> OpmResult<Option<Point3<Length>>> {
        let intersection = surface.0.lock_opm()?.calc_intersect_and_normal(ray);
        Ok(intersection
            .map(|(point, _)| point)
            .filter(|point| self.is_within_aperture(point)))
    }
}

impl Body for SurfaceBoundedBody {
    fn contains(&self, point: &Point3<Length>) -> OpmResult<bool> {
        if !self.is_within_aperture(point) {
            return Ok(false);
        }
        let behind_entrance = self.entrance.0.lock_opm()?.is_behind(point);
        let behind_exit = self.exit.0.lock_opm()?.is_behind(point);
        Ok(behind_entrance && !behind_exit)
    }
    fn path_length_inside(&self, ray: &Ray) -> OpmResult<Option<Length>> {
        // Collect all points at which the ray can enter or leave the body: its own starting point
        // if it already lies inside, and its hits on both bounding surfaces. Treating the starting
        // point as a candidate of its own keeps the case of a ray starting *on* a bounding surface
        // correct: such a ray hits that surface at zero distance, which must not be mistaken for
        // the point where it leaves the body again.
        let ray_position = ray.position();
        let mut boundary_points = Vec::with_capacity(3);
        if self.contains(&ray_position)? {
            boundary_points.push(ray_position);
        }
        for surface in [&self.entrance, &self.exit] {
            if let Some(point) = self.bounded_intersection(surface, ray)? {
                boundary_points.push(point);
            }
        }
        if boundary_points.len() < 2 {
            // The ray either misses the body altogether or leaves it through its lateral boundary,
            // which is not part of the model.
            return Ok(None);
        }
        // Sort the candidates along the ray in order to get the first and the last one.
        let direction = ray.direction();
        boundary_points.sort_by(|point_a, point_b| {
            let position_along_ray =
                |point: &Point3<Length>| (point - ray_position).map(|c| c.value).dot(&direction);
            position_along_ray(point_a).total_cmp(&position_along_ray(point_b))
        });
        let (Some(first_point), Some(last_point)) =
            (boundary_points.first(), boundary_points.last())
        else {
            return Ok(None);
        };
        // Guard against bodies the ray leaves and re-enters, e.g. a strongly curved meniscus: the
        // section between the outermost two candidates is only fully inside if its center is.
        let center_point = Point3::new(
            (first_point.x + last_point.x) * 0.5,
            (first_point.y + last_point.y) * 0.5,
            (first_point.z + last_point.z) * 0.5,
        );
        if !self.contains(&center_point)? {
            return Ok(None);
        }
        Ok(Some(distance_3d_point(first_point, last_point)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::ApertureType,
        geometry::{Plane, Sphere},
        joule, millimeter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;
    use std::sync::{Arc, Mutex};

    /// Create a disk of the given thickness and radius, with its entrance surface at z = 0.
    fn disk(thickness: Length, radius: Length) -> OpmResult<SurfaceBoundedBody> {
        let entrance = Plane::new(Isometry::identity());
        let exit = Plane::new(Isometry::new_along_z(thickness)?);
        Ok(SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(exit))),
            Aperture::new_circle(radius, ApertureType::Hole, None)?,
            Isometry::identity(),
        ))
    }
    /// Create a ray at the given position, propagating along the given direction.
    fn test_ray(position: Point3<Length>, direction: Vector3<f64>) -> OpmResult<Ray> {
        Ray::new(position, direction, nanometer!(1053.0), joule!(1.0))
    }
    #[test]
    fn contains_disk() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        assert!(body.contains(&millimeter!(0.0, 0.0, 5.0))?);
        // the body is half-open: the entrance surface belongs to it, the exit surface does not
        assert!(body.contains(&millimeter!(0.0, 0.0, 0.0))?);
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.0))?);
        // in front of the entrance surface / behind the exit surface
        assert!(!body.contains(&millimeter!(0.0, 0.0, -0.1))?);
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.1))?);
        // outside the transversal aperture
        assert!(body.contains(&millimeter!(4.9, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(5.1, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(0.0, -5.1, 5.0))?);
        Ok(())
    }
    #[test]
    fn contains_slab() -> OpmResult<()> {
        // a slab is the same body with a rectangular instead of a circular aperture
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            Aperture::new_rectangle(
                millimeter!(20.0),
                millimeter!(4.0),
                ApertureType::Hole,
                None,
                None,
            )?,
            Isometry::identity(),
        );
        assert!(body.contains(&millimeter!(9.0, 1.9, 5.0))?);
        assert!(!body.contains(&millimeter!(11.0, 1.9, 5.0))?);
        assert!(!body.contains(&millimeter!(9.0, 2.1, 5.0))?);
        Ok(())
    }
    #[test]
    fn contains_lens() -> OpmResult<()> {
        // a biconvex lens: both centers of curvature lie inside the body
        let entrance = Sphere::new_at_position(millimeter!(50.0), millimeter!(0.0, 0.0, 0.0))?;
        let exit = Sphere::new_at_position(millimeter!(-50.0), millimeter!(0.0, 0.0, 10.0))?;
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(exit))),
            Aperture::new_circle(millimeter!(10.0), ApertureType::Hole, None)?,
            Isometry::identity(),
        );
        assert!(body.contains(&millimeter!(0.0, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(0.0, 0.0, -0.1))?);
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.1))?);
        // the sag at x = 10 mm is 1.0102 mm, so the body is thinner towards its rim
        assert!(body.contains(&millimeter!(10.0, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(10.0, 0.0, 1.0))?);
        assert!(!body.contains(&millimeter!(10.0, 0.0, 9.0))?);
        Ok(())
    }
    #[test]
    fn path_length_on_axis() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, -20.0), Vector3::z())?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_oblique() -> OpmResult<()> {
        // a ray at 45 degrees travels sqrt(2) times the thickness through the disk
        let body = disk(millimeter!(10.0), millimeter!(20.0))?;
        let ray = test_ray(millimeter!(0.0, -5.0, -5.0), Vector3::new(0.0, 1.0, 1.0))?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(
            path_length.value,
            millimeter!(10.0 * f64::sqrt(2.0)).value,
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn path_length_starting_on_the_entrance_surface() -> OpmResult<()> {
        // a ray refracted at the entrance surface starts exactly on it, so it hits that surface at
        // zero distance. That hit must not be mistaken for the point where it leaves the body.
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 0.0), Vector3::z())?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_on_the_exit_surface() -> OpmResult<()> {
        // the same for a ray entering the body backwards through its exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 10.0), -Vector3::z())?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_inside() -> OpmResult<()> {
        // a ray starting inside the body only travels the remaining distance to the exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), Vector3::z())?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(path_length.value, millimeter!(6.0).value);
        Ok(())
    }
    #[test]
    fn path_length_backwards() -> OpmResult<()> {
        // a backwards propagating ray leaves the body through its entrance surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), -Vector3::z())?;
        let path_length = body
            .path_length_inside(&ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))?;
        assert_abs_diff_eq!(path_length.value, millimeter!(4.0).value);
        Ok(())
    }
    #[test]
    fn path_length_missing_the_body() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        // passing by outside the aperture
        let ray = test_ray(millimeter!(6.0, 0.0, -20.0), Vector3::z())?;
        assert_eq!(body.path_length_inside(&ray)?, None);
        // starting behind the body
        let ray = test_ray(millimeter!(0.0, 0.0, 20.0), Vector3::z())?;
        assert_eq!(body.path_length_inside(&ray)?, None);
        Ok(())
    }
    #[test]
    fn path_length_leaving_sideways() -> OpmResult<()> {
        // a ray leaving through the barrel of the body is not modelled and thus reported as a miss
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 1.0), Vector3::new(0.0, 1.0, 0.1))?;
        assert_eq!(body.path_length_inside(&ray)?, None);
        Ok(())
    }
}
