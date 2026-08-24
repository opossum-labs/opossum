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
    error::{OpmResult, OpossumError},
    geometry::geo_surface::GeoSurfaceRef,
    light::Ray,
    meter, millimeter,
    types::validated_type_definitions::ValidatedCrossSection,
    utils::{LockExt, geom_transformation::Isometry, math_utils::distance_3d_point},
};
use nalgebra::{Point2, Point3, Vector3};
use num::Zero;
use std::{fmt::Debug, ops::Range};
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

/// An axis-aligned box enclosing a [`Body`].
///
/// The box is expressed in the body's own frame ([`Body::isometry`]) rather than in global
/// coordinates: a discretisation of the body has to be axis-aligned with the component it describes,
/// not with the laboratory, or a tilted component would be sampled on a staircase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    min: Point3<Length>,
    max: Point3<Length>,
}

impl BoundingBox {
    /// Create a new [`BoundingBox`] from two opposite corners.
    ///
    /// # Arguments
    ///
    /// - `min`: the corner with the smallest coordinate on every axis
    /// - `max`: the corner with the largest coordinate on every axis
    ///
    /// # Returns
    ///
    /// The box spanned by the two corners. A box may be flat on an axis (a body of zero thickness
    /// is still a body), but not inverted.
    ///
    /// # Errors
    ///
    /// This function returns an error if any coordinate is not finite or if `min` exceeds `max` on
    /// any axis.
    pub fn new(min: Point3<Length>, max: Point3<Length>) -> OpmResult<Self> {
        if min.iter().chain(max.iter()).any(|c| !c.is_finite()) {
            return Err(OpossumError::Other(
                "the corners of a bounding box must be finite".into(),
            ));
        }
        if min.iter().zip(max.iter()).any(|(low, high)| low > high) {
            return Err(OpossumError::Other(
                "the lower corner of a bounding box must not exceed its upper corner".into(),
            ));
        }
        Ok(Self { min, max })
    }
    /// Return the extent of this [`BoundingBox`] along the x axis.
    #[must_use]
    pub fn x_range(&self) -> Range<Length> {
        self.min.x..self.max.x
    }
    /// Return the extent of this [`BoundingBox`] along the y axis.
    #[must_use]
    pub fn y_range(&self) -> Range<Length> {
        self.min.y..self.max.y
    }
    /// Return the extent of this [`BoundingBox`] along the z axis.
    #[must_use]
    pub fn z_range(&self) -> Range<Length> {
        self.min.z..self.max.z
    }
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
    /// Return an axis-aligned box that encloses this [`Body`].
    ///
    /// [`Body::contains`] answers where the body is *not*, one point at a time; this states where it
    /// is worth asking at all. Anything discretising the body needs both: the box gives the domain
    /// to lay a grid over, [`Body::contains`] carves the body out of it.
    ///
    /// # Returns
    ///
    /// A [`BoundingBox`] containing the whole body, expressed in the body's own frame (see
    /// [`Body::isometry`]). It is not required to be the tightest such box — a grid masks its cells
    /// individually anyway — but a body must never reach outside it.
    ///
    /// # Errors
    ///
    /// This function returns an error if an internal mutex of a bounding surface cannot be locked or
    /// if the extent of the body cannot be determined.
    fn bounding_box(&self) -> OpmResult<BoundingBox>;
    /// Return the frame this [`Body`] is placed in.
    ///
    /// This is the frame [`Body::bounding_box`] is expressed in, so it is what turns that box back
    /// into the global coordinates [`Body::contains`] expects. It is the frame of the component the
    /// body belongs to, which is what makes a grid over the body follow the component when it is
    /// moved or tilted.
    fn isometry(&self) -> &Isometry;
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
///
/// **On cost**: every query locks the mutex of each bounding surface it needs, so a query is a few
/// lock/unlock pairs plus the geometry itself. That is deliberate — the surfaces are shared with
/// the node they came from, which may realign them between queries. A caller sweeping a whole ray
/// bundle or a discretisation grid can amortise this once a hot path exists, but it must not hold
/// both guards at the same time: a node with a single surface hands out the same
/// [`GeoSurfaceRef`] twice, and locking it twice would deadlock.
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
    /// Return the points the transversal cross section of this body reaches farthest in, in the
    /// body's own frame.
    ///
    /// Both the axis-aligned transversal bounds and the largest distance from the body's axis follow
    /// from these points. They are not a tessellation of the outline: a circle is described by four
    /// points plus, if it is shifted off the axis, the single point of it lying farthest out — a
    /// direction none of the axis-aligned extremes points in.
    ///
    /// # Returns
    ///
    /// The extreme points of the cross section, with the isometry of its
    /// [`Aperture`](crate::apertures::Aperture) already applied.
    ///
    /// # Errors
    ///
    /// This function returns an error if the cross section is not one of the binary shapes. Since
    /// [`ValidatedCrossSection`] admits nothing else, that cannot happen for a body built through
    /// its constructor.
    fn cross_section_outline(&self) -> OpmResult<Vec<Point2<Length>>> {
        let cross_section = self.cross_section.get();
        let transform = |point: Point2<Length>| {
            cross_section.isometry().map_or(point, |iso| {
                let transformed =
                    iso.transform_point(&Point3::new(point.x, point.y, Length::zero()));
                Point2::new(transformed.x, transformed.y)
            })
        };
        match cross_section.shape() {
            ApertureShape::BinaryCircle(circle) => {
                // A circle is indifferent to the rotation of its aperture, so only its center
                // moves. The axis-aligned bounds follow from that center alone, while the point
                // farthest from the body's axis lies on the far side of the shifted circle.
                let center = transform(Point2::origin());
                let radius = circle.radius();
                let mut outline = vec![
                    Point2::new(center.x + radius, center.y),
                    Point2::new(center.x - radius, center.y),
                    Point2::new(center.x, center.y + radius),
                    Point2::new(center.x, center.y - radius),
                ];
                let shift = center.x.value.hypot(center.y.value);
                if shift > 0.0 {
                    let stretch = 1.0 + radius.value / shift;
                    outline.push(Point2::new(center.x * stretch, center.y * stretch));
                }
                Ok(outline)
            }
            ApertureShape::BinaryRectangle(rectangle) => {
                let half_width = rectangle.width() / 2.0;
                let half_height = rectangle.height() / 2.0;
                Ok([
                    Point2::new(half_width, half_height),
                    Point2::new(-half_width, half_height),
                    Point2::new(-half_width, -half_height),
                    Point2::new(half_width, -half_height),
                ]
                .map(transform)
                .to_vec())
            }
            ApertureShape::BinaryPolygon(polygon) => {
                Ok(polygon.points().iter().map(|p| transform(*p)).collect())
            }
            shape => Err(OpossumError::Other(format!(
                "the extent of a body bounded by a '{shape}' cross section is undefined"
            ))),
        }
    }
    /// Return the range of longitudinal positions one of the bounding surfaces spans over the cross
    /// section of this body, in the body's own frame.
    ///
    /// The surface is asked where it lies above its own axis — its vertex — and how far it deviates
    /// from that towards the rim ([`GeoSurface::local_z_at`]). Both answers are given in the
    /// surface's own frame, which need not be the body's: a wedge tilts its exit surface against the
    /// body it bounds.
    ///
    /// The result is exact for a surface that is either flat or aligned with the body's axis, which
    /// is every surface the volume nodes build. For one that is both curved *and* tilted, the
    /// transversal reach derived below neglects that the sag itself tilts out of the cross section,
    /// which would make the range slightly too narrow — such a surface is therefore rejected rather
    /// than silently truncating the body.
    ///
    /// # Arguments
    ///
    /// - `surface`: the bounding surface to measure
    /// - `transversal_reach`: the largest distance from the body's axis its cross section reaches
    ///
    /// # Returns
    ///
    /// The longitudinal range the surface spans over the body's cross section.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface's mutex cannot be locked, if the surface runs
    /// parallel to the body's axis, if it does not reach as far out as the cross section, or if it
    /// is both curved and tilted against the body.
    fn surface_z_range(
        &self,
        surface: &GeoSurfaceRef,
        transversal_reach: Length,
    ) -> OpmResult<Range<Length>> {
        // Everything the surface itself has to answer happens inside this block, so its lock is
        // released again before the result is assembled.
        let (anchor, from_tilt, sag_span) = {
            let surface = surface.0.lock_opm()?;
            let relative = Isometry::new_from_transform(
                self.isometry.get_inv_transform() * surface.isometry().get_transform(),
            );
            // The body's own z axis, written in the surface's frame: dotted with a point of the
            // surface it yields that point's longitudinal position in the body. Its transversal
            // part states how far the surface is tilted against the body, its z part how much of
            // the surface's own sag survives into the body's z.
            let body_axis = relative.inverse_transform_vector_f64(&Vector3::z());
            let tilt = body_axis.x.hypot(body_axis.y);
            let alignment = body_axis.z;
            if alignment == 0.0 {
                return Err(OpossumError::Other(format!(
                    "the '{}' surface runs parallel to the body's axis and does not bound it",
                    surface.name()
                )));
            }
            // A tilted surface has to be looked at further out to still span the cross section.
            let reach = transversal_reach / alignment.abs();
            let out_of_reach = || {
                OpossumError::Other(format!(
                    "the '{}' surface does not reach as far out as the cross section of the body",
                    surface.name()
                ))
            };
            let vertex = surface
                .local_z_at(&Point2::origin())
                .ok_or_else(out_of_reach)?;
            // The sag grows monotonically with the distance from the axis for every surface
            // modelled here, so its extremes over the cross section are attained at the rim. Both
            // axes are sampled because a Cylinder is curved along one of them only.
            let mut lowest_sag = Length::zero();
            let mut highest_sag = Length::zero();
            for rim in [
                Point2::new(reach, Length::zero()),
                Point2::new(-reach, Length::zero()),
                Point2::new(Length::zero(), reach),
                Point2::new(Length::zero(), -reach),
            ] {
                let sag = surface.local_z_at(&rim).ok_or_else(out_of_reach)? - vertex;
                lowest_sag = Length::min(lowest_sag, sag);
                highest_sag = Length::max(highest_sag, sag);
            }
            if tilt > 0.0 && (lowest_sag != Length::zero() || highest_sag != Length::zero()) {
                return Err(OpossumError::Other(format!(
                    "the extent of a body bounded by the curved '{}' surface tilted against it is \
                     not supported",
                    surface.name()
                )));
            }
            let anchor = relative
                .transform_point(&Point3::new(Length::zero(), Length::zero(), vertex))
                .z;
            let (one_sag, other_sag) = (lowest_sag * alignment, highest_sag * alignment);
            (
                anchor,
                reach * tilt,
                Length::min(one_sag, other_sag)..Length::max(one_sag, other_sag),
            )
        };
        Ok(anchor - from_tilt + sag_span.start..anchor + from_tilt + sag_span.end)
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
    fn bounding_box(&self) -> OpmResult<BoundingBox> {
        let outline = self.cross_section_outline()?;
        let (Some(x_range), Some(y_range)) = (
            span(outline.iter().map(|point| point.x)),
            span(outline.iter().map(|point| point.y)),
        ) else {
            return Err(OpossumError::Other(
                "the cross section of the body has no extent at all".into(),
            ));
        };
        let transversal_reach = outline.iter().fold(Length::zero(), |reach, point| {
            Length::max(reach, meter!(point.x.value.hypot(point.y.value)))
        });
        // The two surfaces are measured one after the other and never held at once: a node with a
        // single surface hands out the same `GeoSurfaceRef` twice, which would deadlock.
        let entrance_z = self.surface_z_range(&self.entrance, transversal_reach)?;
        let exit_z = self.surface_z_range(&self.exit, transversal_reach)?;
        // Which of the two bounds the body from below is not assumed here: an inverted node
        // encloses the same volume as an upright one, so the two are simply united.
        BoundingBox::new(
            Point3::new(
                x_range.start,
                y_range.start,
                Length::min(entrance_z.start, exit_z.start),
            ),
            Point3::new(
                x_range.end,
                y_range.end,
                Length::max(entrance_z.end, exit_z.end),
            ),
        )
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
}

/// Return the smallest and the largest of the given lengths.
///
/// # Arguments
///
/// - `values`: the lengths to be spanned
///
/// # Returns
///
/// The range from the smallest to the largest value, or `None` if there are no values at all.
fn span(values: impl Iterator<Item = Length>) -> Option<Range<Length>> {
    values.fold(None, |spanned: Option<Range<Length>>, value| {
        Some(spanned.map_or(value..value, |range| {
            Length::min(range.start, value)..Length::max(range.end, value)
        }))
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        geometry::{Cylinder, Plane, Sphere, geo_surface::GeoSurface},
        joule, millimeter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;
    use std::sync::{Arc, Mutex};
    use uom::si::{f64::Angle, length::millimeter};

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
    /// Assert that the given range runs between the two given positions, in millimeter.
    fn assert_spans(range: &Range<Length>, start: f64, end: f64) {
        assert_abs_diff_eq!(range.start.value, millimeter!(start).value, epsilon = 1e-9);
        assert_abs_diff_eq!(range.end.value, millimeter!(end).value, epsilon = 1e-9);
    }
    #[test]
    fn bounding_box_of_a_disk() -> OpmResult<()> {
        // Both bounding surfaces are flat and the cross section is centered, so the box is the
        // body: the plate thickness in z, the aperture radius transversally.
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -5.0, 5.0);
        assert_spans(&bounds.y_range(), -5.0, 5.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_lens() -> OpmResult<()> {
        // The same biconvex lens as `contains_lens`: its two vertices are the extreme points along
        // z, since both surfaces curve *into* the body from there. The sag at the rim is 1.0102 mm,
        // so a box derived from the surfaces without their curvature would be that much too large.
        let entrance = Sphere::new_at_position(millimeter!(50.0), millimeter!(0.0, 0.0, 0.0))?;
        let exit = Sphere::new_at_position(millimeter!(-50.0), millimeter!(0.0, 0.0, 10.0))?;
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(exit))),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        );
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -10.0, 10.0);
        assert_spans(&bounds.y_range(), -10.0, 10.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_wedge() -> OpmResult<()> {
        // A wedge tilts its exit surface against the body, so that surface alone reaches further
        // than the center thickness in both directions - by the aperture radius times the tangent
        // of the wedge angle.
        let angle = degree!(30.0);
        let exit_iso = Isometry::new_along_z(millimeter!(10.0))?.append(&Isometry::new(
            Point3::origin(),
            Point3::new(angle, Angle::zero(), Angle::zero()),
        )?);
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(exit_iso)))),
            circular_cross_section(millimeter!(5.0))?,
            Isometry::identity(),
        );
        let overhang = 5.0 * angle.value.tan();
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -5.0, 5.0);
        assert_spans(&bounds.y_range(), -5.0, 5.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0 + overhang);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_rectangular_cross_section() -> OpmResult<()> {
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
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -10.0, 10.0);
        assert_spans(&bounds.y_range(), -2.0, 2.0);
        Ok(())
    }
    #[test]
    fn bounding_box_follows_a_decentred_cross_section() -> OpmResult<()> {
        // The cross section may carry an isometry of its own, which shifts the medium off the axis
        // of the node. A box ignoring it would disagree with `contains`.
        let (entrance, exit) = plate_surfaces(millimeter!(10.0))?;
        let body = SurfaceBoundedBody::new(
            entrance,
            exit,
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                Some(Point2::new(millimeter!(3.0), millimeter!(4.0))),
            )?)?,
            Isometry::identity(),
        );
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -2.0, 8.0);
        assert_spans(&bounds.y_range(), -1.0, 9.0);
        // ... and the point of the medium farthest from the axis is the far side of that circle
        assert!(body.contains(&millimeter!(7.5, 6.0, 5.0))?);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_body_rotated_about_its_own_axis() -> OpmResult<()> {
        // A cylindric lens rotates its curved surface about the optical axis to choose the
        // direction it focuses in. That is not a tilt against the body, so the box stays exact.
        let curved_iso = Isometry::new(
            Point3::origin(),
            Point3::new(Angle::zero(), Angle::zero(), degree!(90.0)),
        )?;
        let mut entrance = Cylinder::new(millimeter!(50.0), Isometry::identity())?;
        entrance.set_isometry(curved_iso);
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        );
        let bounds = body.bounding_box()?;
        // same sag as the spherical lens above, since the radius and the aperture are the same
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        assert_abs_diff_eq!(
            bounds.z_range().end.value - bounds.z_range().start.value,
            millimeter!(10.0).value,
            epsilon = 1e-9
        );
        Ok(())
    }
    #[test]
    fn bounding_box_rejects_a_curved_surface_tilted_against_the_body() -> OpmResult<()> {
        // For such a surface the sag itself tilts out of the cross section, which the transversal
        // reach below does not account for - so the box would come out too small. That is refused
        // rather than silently truncating the body.
        let tilted_iso = Isometry::new(
            Point3::origin(),
            Point3::new(degree!(30.0), Angle::zero(), Angle::zero()),
        )?;
        let mut entrance = Cylinder::new(millimeter!(50.0), Isometry::identity())?;
        entrance.set_isometry(tilted_iso);
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        );
        assert!(body.bounding_box().is_err());
        Ok(())
    }
    #[test]
    fn bounding_box_rejects_a_cross_section_beyond_the_curvature() -> OpmResult<()> {
        // A body wider than the radius of curvature of its bounding surface is not closed by that
        // surface at all - the very case `GeoSurface::is_behind` already warns about.
        let entrance = Sphere::new_at_position(millimeter!(5.0), millimeter!(0.0, 0.0, 0.0))?;
        let body = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(entrance))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        );
        assert!(body.bounding_box().is_err());
        Ok(())
    }
    #[test]
    fn bounding_box_is_expressed_in_the_body_frame() -> OpmResult<()> {
        // Moving and tilting the whole body must not change its box: it is stated in the body's own
        // frame, which is exactly what lets a grid over it follow the component.
        let (entrance, exit) = plate_surfaces(millimeter!(10.0))?;
        let placement = Isometry::new(
            millimeter!(20.0, -5.0, 100.0),
            Point3::new(degree!(15.0), degree!(-10.0), Angle::zero()),
        )?;
        let placed = SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(placement)))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(
                placement.append(&Isometry::new_along_z(millimeter!(10.0))?),
            )))),
            circular_cross_section(millimeter!(5.0))?,
            placement,
        );
        let upright = SurfaceBoundedBody::new(
            entrance,
            exit,
            circular_cross_section(millimeter!(5.0))?,
            Isometry::identity(),
        );
        let (placed_bounds, upright_bounds) = (placed.bounding_box()?, upright.bounding_box()?);
        for (placed_range, upright_range) in [
            (placed_bounds.x_range(), upright_bounds.x_range()),
            (placed_bounds.y_range(), upright_bounds.y_range()),
            (placed_bounds.z_range(), upright_bounds.z_range()),
        ] {
            assert_spans(
                &placed_range,
                upright_range.start.get::<millimeter>(),
                upright_range.end.get::<millimeter>(),
            );
        }
        Ok(())
    }
    #[test]
    fn a_bounding_box_may_be_flat_but_not_inverted() {
        assert!(BoundingBox::new(millimeter!(0.0, 0.0, 0.0), millimeter!(1.0, 1.0, 0.0)).is_ok());
        assert!(BoundingBox::new(millimeter!(0.0, 0.0, 1.0), millimeter!(1.0, 1.0, 0.0)).is_err());
        assert!(
            BoundingBox::new(millimeter!(0.0, 0.0, f64::NAN), millimeter!(1.0, 1.0, 1.0)).is_err()
        );
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
