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
//! exit surface and a transversal cross section. Since the bounding surfaces are the ones a node
//! already uses for refraction, the usual optical volumes are simply different parameter sets of
//! the same body: a disk is two [`Plane`](crate::geometry::Plane)s with a circular cross section, a
//! slab two planes with a rectangular one, a lens two [`Sphere`](crate::geometry::Sphere)s.
//!
//! The cross section is expressed as an [`Aperture`](crate::apertures::Aperture), but only those
//! apertures that actually delimit a region qualify, which is what
//! [`ValidatedCrossSection`] guarantees — an aperture is in general a transmission mask, and
//! softening, inverting or omitting the transmission edge says nothing about where the material
//! ends.

use crate::{
    apertures::{ApertureShape, CircleShape},
    error::OpmResult,
    geometry::geo_surface::GeoSurfaceRef,
    light::Ray,
    millimeter,
    types::validated_type_definitions::ValidatedCrossSection,
    utils::{LockExt, geom_transformation::Isometry, math_utils::distance_3d_point},
};
use nalgebra::Point3;
use std::fmt::Debug;
use uom::si::f64::Length;

/// Name of the property holding the transversal extent of a volume node.
///
/// The clear aperture is the size the material is actually available in — the figure a supplier
/// quotes next to the curvatures and the thickness. It is a property of the component rather than
/// of one of its ports, which is what distinguishes it from the port
/// [`Aperture`](crate::apertures::Aperture): the latter states how much light a surface transmits
/// where and may soften or invert that transmission, while the clear aperture states where the
/// medium ends. The two are independent — putting a pinhole in front of a lens does not make the
/// lens smaller.
///
/// Every volume node has one: a component of unstated size has no volume to speak of, and two
/// curved surfaces only happen to close on their own if they are curved strongly enough. Shapes
/// that leave the extent undefined — [`ApertureShape::Open`] among them — are therefore rejected.
pub const CLEAR_APERTURE: &str = "clear aperture";

/// The clear aperture a volume node starts out with if nothing is defined.
///
/// A circle of 12.5 mm radius: the 25 mm (1 inch) mount most catalogue optics come in.
///
/// # Returns
///
/// The default transversal extent of a volume node.
///
/// # Panics
///
/// Panics if the hard-coded radius is rejected by [`CircleShape`], which cannot happen.
#[must_use]
pub fn default_clear_aperture() -> ApertureShape {
    CircleShape::new(millimeter!(12.5))
        .expect("12.5 mm is a valid aperture radius")
        .into()
}

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

/// A [`Body`] bounded by an entrance surface, an exit surface and a transversal cross section.
///
/// A point is inside this body if it lies behind the entrance surface (see
/// [`is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind)), not behind the exit surface
/// and within the cross section. Both bounding surfaces are held as [`GeoSurfaceRef`]s, i.e. the
/// body shares them with whoever else holds them rather than copying their geometry.
///
/// **Note**: The lateral boundary is the cross section alone — it is not a surface rays can
/// interact with. A ray leaving the volume sideways is therefore reported as not passing through
/// the body rather than being reflected on a barrel surface.
#[derive(Debug, Clone)]
pub struct SurfaceBoundedBody {
    entrance: GeoSurfaceRef,
    exit: GeoSurfaceRef,
    cross_section: ValidatedCrossSection,
    isometry: Isometry,
}

impl SurfaceBoundedBody {
    /// Create a new [`SurfaceBoundedBody`].
    ///
    /// # Arguments
    ///
    /// - `entrance`: the surface bounding the body towards -z
    /// - `exit`: the surface bounding the body towards +z
    /// - `cross_section`: the transversal boundary of the body
    /// - `isometry`: the frame the cross section is defined in, usually the isometry of the node
    ///   the body belongs to
    #[must_use]
    pub const fn new(
        entrance: GeoSurfaceRef,
        exit: GeoSurfaceRef,
        cross_section: ValidatedCrossSection,
        isometry: Isometry,
    ) -> Self {
        Self {
            entrance,
            exit,
            cross_section,
            isometry,
        }
    }
    /// Return whether the given point lies within the transversal cross section of this body.
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, given in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the point lies within the cross section.
    fn is_within_cross_section(&self, point: &Point3<Length>) -> bool {
        let local_point = self.isometry.inverse_transform_point(point);
        // The cross section is a binary hole, so a transmission above zero means "inside".
        self.cross_section.get().apodize(&local_point) > 0.0
    }
    /// Intersect the given [`Ray`] with one of the bounding surfaces of this body.
    ///
    /// Hits outside the transversal cross section are discarded, since they lie on the unbounded
    /// part of the surface which is not part of the body.
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
            .filter(|point| self.is_within_cross_section(point)))
    }
}

impl Body for SurfaceBoundedBody {
    fn contains(&self, point: &Point3<Length>) -> OpmResult<bool> {
        if !self.is_within_cross_section(point) {
            return Ok(false);
        }
        if !self.entrance.0.lock_opm()?.is_behind(point) {
            return Ok(false);
        }
        Ok(!self.exit.0.lock_opm()?.is_behind(point))
    }
    fn path_length_inside(&self, ray: &Ray) -> OpmResult<Option<Length>> {
        // Every point at which the ray can enter or leave the body is a candidate: its own starting
        // point if it already lies inside, and its hits on both bounding surfaces. Treating the
        // starting point as a candidate of its own keeps the case of a ray starting *on* a bounding
        // surface correct: such a ray hits that surface at zero distance, which must not be
        // mistaken for the point where it leaves the body again.
        let ray_position = ray.position();
        let direction = ray.direction();
        let position_along_ray =
            |point: &Point3<Length>| (point - ray_position).map(|c| c.value).dot(&direction);
        // Only the outermost two candidates matter, so they are tracked directly instead of being
        // collected and sorted. Their number is counted rather than derived from their positions:
        // two candidates may well coincide — a ray starting on a bounding surface hits that surface
        // at zero distance — and such a ray does travel a (zero) length inside the body, whereas a
        // single candidate means it never entered.
        let mut candidates = 0_usize;
        let mut first: Option<(f64, Point3<Length>)> = None;
        let mut last: Option<(f64, Point3<Length>)> = None;
        let mut consider = |point: Point3<Length>| {
            candidates += 1;
            let position = position_along_ray(&point);
            if first.is_none_or(|(first_position, _)| position < first_position) {
                first = Some((position, point));
            }
            if last.is_none_or(|(last_position, _)| position > last_position) {
                last = Some((position, point));
            }
        };
        if self.contains(&ray_position)? {
            consider(ray_position);
        }
        for surface in [&self.entrance, &self.exit] {
            if let Some(point) = self.bounded_intersection(surface, ray)? {
                consider(point);
            }
        }
        let (Some((_, first_point)), Some((_, last_point))) = (first, last) else {
            return Ok(None);
        };
        if candidates < 2 {
            // The ray either misses the body altogether or leaves it through its lateral boundary,
            // which is not part of the model.
            return Ok(None);
        }
        // Guard against bodies the ray leaves and re-enters, e.g. a strongly curved meniscus: the
        // section between the outermost two candidates is only fully inside if its center is.
        let center_point = first_point + (last_point - first_point).map(|c| c * 0.5);
        if !self.contains(&center_point)? {
            return Ok(None);
        }
        Ok(Some(distance_3d_point(&first_point, &last_point)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        geometry::{Plane, Sphere},
        joule, millimeter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;
    use std::sync::{Arc, Mutex};

    /// Create a circular cross section of the given radius.
    fn circular_cross_section(radius: Length) -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new_circle(radius, ApertureType::Hole, None)?)
    }
    /// Create the two plane surfaces of a plate of the given thickness.
    fn plate_surfaces(thickness: Length) -> OpmResult<(GeoSurfaceRef, GeoSurfaceRef)> {
        Ok((
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                thickness,
            )?)))),
        ))
    }
    /// Create a disk of the given thickness and radius, with its entrance surface at z = 0.
    fn disk(thickness: Length, radius: Length) -> OpmResult<SurfaceBoundedBody> {
        let (entrance, exit) = plate_surfaces(thickness)?;
        Ok(SurfaceBoundedBody::new(
            entrance,
            exit,
            circular_cross_section(radius)?,
            Isometry::identity(),
        ))
    }
    /// Create a ray at the given position, propagating along the given direction.
    fn test_ray(position: Point3<Length>, direction: Vector3<f64>) -> OpmResult<Ray> {
        Ray::new(position, direction, nanometer!(1053.0), joule!(1.0))
    }
    /// Trace the given ray through the given body and require it to pass through.
    fn path_length(body: &SurfaceBoundedBody, ray: &Ray) -> OpmResult<Length> {
        body.path_length_inside(ray)?
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))
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
        // outside the transversal cross section
        assert!(body.contains(&millimeter!(4.9, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(5.1, 0.0, 5.0))?);
        assert!(!body.contains(&millimeter!(0.0, -5.1, 5.0))?);
        Ok(())
    }
    #[test]
    fn contains_slab() -> OpmResult<()> {
        // a slab is the same body with a rectangular instead of a circular cross section
        let (entrance, exit) = plate_surfaces(millimeter!(10.0))?;
        let body = SurfaceBoundedBody::new(
            entrance,
            exit,
            ValidatedCrossSection::try_new(Aperture::new_rectangle(
                millimeter!(20.0),
                millimeter!(4.0),
                ApertureType::Hole,
                None,
                None,
            )?)?,
            Isometry::identity(),
        );
        assert!(body.contains(&millimeter!(9.0, 1.9, 5.0))?);
        assert!(!body.contains(&millimeter!(11.0, 1.9, 5.0))?);
        assert!(!body.contains(&millimeter!(9.0, 2.1, 5.0))?);
        Ok(())
    }
    #[test]
    fn cross_section_has_to_delimit_a_region() -> OpmResult<()> {
        // A Gaussian aperture attenuates everywhere, an open one does not restrict anything at all
        // and an obstruction is transparent outside its shape. None of them states where the
        // material ends, so none of them can bound a body.
        let soft_edged = Aperture::new_gaussian(
            (millimeter!(5.0), millimeter!(5.0)),
            ApertureType::Hole,
            None,
            None,
        )?;
        let unrestricted = Aperture::default();
        let inverted = Aperture::new_circle(millimeter!(5.0), ApertureType::Obstruction, None)?;
        for aperture in [soft_edged, unrestricted, inverted] {
            assert!(ValidatedCrossSection::try_new(aperture).is_err());
        }
        // ... while a binary hole does
        assert!(
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                None
            )?)
            .is_ok()
        );
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
            circular_cross_section(millimeter!(10.0))?,
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
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_oblique() -> OpmResult<()> {
        // a ray at 45 degrees travels sqrt(2) times the thickness through the disk
        let body = disk(millimeter!(10.0), millimeter!(20.0))?;
        let ray = test_ray(millimeter!(0.0, -5.0, -5.0), Vector3::new(0.0, 1.0, 1.0))?;
        let path_length = path_length(&body, &ray)?;
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
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_on_the_exit_surface() -> OpmResult<()> {
        // the same for a ray entering the body backwards through its exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 10.0), -Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_inside() -> OpmResult<()> {
        // a ray starting inside the body only travels the remaining distance to the exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(6.0).value);
        Ok(())
    }
    #[test]
    fn path_length_backwards() -> OpmResult<()> {
        // a backwards propagating ray leaves the body through its entrance surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), -Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
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
