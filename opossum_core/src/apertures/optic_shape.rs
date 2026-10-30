#![warn(missing_docs)]
//! The value of a component's optic shape: an outline on its axis, possibly with a bore through it.
//!
//! A component has an edge and, possibly, one bore through it, such as a mirror with a hole for the
//! beam to pass. That is all the optic shape states: unlike a general [`Aperture`], it has no
//! soft edges, no obstructions and no stack of arbitrary shapes. The geometry works with the
//! [`Outline`] directly (see [`OpticShape::outline`]); an [`Aperture`] is made of it only to be
//! plotted.

use super::{
    Aperture, ApertureShape, ApertureType, CircleShape, OutlineMesh, PolygonShape, RectangleShape,
    Shape, resample_ring,
};
use crate::{
    error::{OpmResult, OpossumError},
    meter,
    properties::Proptype,
    utils::{default_from_name::DefaultFromName, math_utils::to_f64},
};
use nalgebra::{Point2, Point3, Rotation3, Vector3};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use std::{f64::consts::TAU, fmt::Display};
use strum::EnumIter;
use uom::si::f64::{Angle, Length};

/// How many points the edge of a circle is sampled with when the bore is checked.
const CIRCLE_POINTS: usize = 64;
/// How close two edges may come, in meters, before they count as touching.
const TOUCHING: f64 = 1e-9;
/// How many steps the axis of a bore may take to settle on where it meets a surface.
const MEETING_STEPS: usize = 100;
/// How far, in meters, the axis of a bore may still move in a step once it has settled on where
/// it meets a surface.
const SETTLED: f64 = 1e-12;

/// A shape with an edge: the outline of a component, or of a bore through it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
pub enum OutlineShape {
    /// A circle around the origin.
    Circle(CircleShape),
    /// A rectangle around the origin.
    Rectangle(RectangleShape),
    /// A polygon.
    Polygon(PolygonShape),
}
impl DefaultFromName for OutlineShape {}

impl From<CircleShape> for OutlineShape {
    fn from(shape: CircleShape) -> Self {
        Self::Circle(shape)
    }
}
impl From<RectangleShape> for OutlineShape {
    fn from(shape: RectangleShape) -> Self {
        Self::Rectangle(shape)
    }
}
impl From<PolygonShape> for OutlineShape {
    fn from(shape: PolygonShape) -> Self {
        Self::Polygon(shape)
    }
}
impl From<OutlineShape> for ApertureShape {
    fn from(shape: OutlineShape) -> Self {
        match shape {
            OutlineShape::Circle(shape) => Self::BinaryCircle(shape),
            OutlineShape::Rectangle(shape) => Self::BinaryRectangle(shape),
            OutlineShape::Polygon(shape) => Self::BinaryPolygon(shape),
        }
    }
}
impl OutlineShape {
    /// Return whether a point lies on or within this shape, in its own frame.
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame of the shape.
    fn contains(&self, point: Point2<Length>) -> bool {
        let shape: &dyn Shape = match self {
            Self::Circle(circle) => circle,
            Self::Rectangle(rectangle) => rectangle,
            Self::Polygon(polygon) => polygon,
        };
        shape.transmission_factor(&Point3::new(point.x, point.y, Length::zero())) > 0.0
    }
    /// Return the points this shape reaches farthest in, in its own frame.
    ///
    /// The axis-aligned bounds and the largest distance from the origin follow from them. A circle
    /// is described exactly by its four points on the axes, which its sampled [`Self::corners`]
    /// only approximate.
    fn extreme_points(&self) -> Vec<Point2<Length>> {
        match self {
            Self::Circle(circle) => {
                let (radius, zero) = (circle.radius(), Length::zero());
                vec![
                    Point2::new(radius, zero),
                    Point2::new(-radius, zero),
                    Point2::new(zero, radius),
                    Point2::new(zero, -radius),
                ]
            }
            Self::Rectangle(_) | Self::Polygon(_) => self.corners(),
        }
    }
    /// The corners of the edge of this shape, in its own frame; a circle is sampled at
    /// [`CIRCLE_POINTS`] points.
    fn corners(&self) -> Vec<Point2<Length>> {
        match self {
            Self::Circle(circle) => (0..CIRCLE_POINTS)
                .map(|step| {
                    let angle = TAU * to_f64(step) / to_f64(CIRCLE_POINTS);
                    Point2::new(circle.radius() * angle.cos(), circle.radius() * angle.sin())
                })
                .collect(),
            Self::Rectangle(rectangle) => rectangle.corners().to_vec(),
            Self::Polygon(polygon) => polygon.points().to_vec(),
        }
    }
    /// Return the edge of this shape as a closed polygon, in its own frame.
    ///
    /// Where [`Outline::contains`] answers whether a single point lies inside, this states where
    /// the inside *ends*: the edge, walked counter-clockwise. The first point is not repeated at
    /// the end — the ring closes from the last point back to the first.
    ///
    /// `segments` is the number of points to aim for, not a promise: every corner of a rectangle or
    /// polygon is always kept, and the remaining points are spread over the edges by length, so the
    /// count comes out near `segments` rather than exactly on it.
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of points to aim for, at least 3
    ///
    /// # Returns
    ///
    /// The points of the edge in counter-clockwise order.
    ///
    /// # Errors
    ///
    /// This function returns an error if fewer than three segments are asked for, or if the shape
    /// encloses no area at all.
    pub fn outline_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        if segments < 3 {
            return Err(OpossumError::Other(format!(
                "an outline of {segments} points encloses nothing, at least 3 are needed"
            )));
        }
        match self {
            Self::Circle(circle) => circle.outline_points(segments),
            Self::Rectangle(_) | Self::Polygon(_) => resample_ring(&self.corners(), segments),
        }
    }
}

/// A bore through a component: the region of a shape, shifted off the axis, turned about its
/// center and possibly tilted against the axis, that holds no material.
///
/// The bore is a prism: its shape is the cross-section across the axis of the bore, as a drill
/// leaves it, and it runs along that axis through the whole component. Its center is where the
/// axis of the bore crosses the plane of the outline. Untilted, it runs along the axis of the
/// component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bore {
    shape: OutlineShape,
    center: Point2<Length>,
    rotation: Angle,
    #[serde(default = "upright", skip_serializing_if = "is_upright")]
    tilt: (Angle, Angle),
}

/// The tilt of a bore that runs along the axis of its component.
fn upright() -> (Angle, Angle) {
    (Angle::zero(), Angle::zero())
}

/// Whether a bore with the given tilt runs along the axis of its component.
fn is_upright(tilt: &(Angle, Angle)) -> bool {
    tilt.0.is_zero() && tilt.1.is_zero()
}

impl Bore {
    /// Create a new [`Bore`] along the axis of the component.
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the bore, around its own center.
    /// * `center` - where the center of the bore lies, relative to the axis of the component.
    /// * `rotation` - how far the bore is turned about its center.
    pub fn new(shape: impl Into<OutlineShape>, center: Point2<Length>, rotation: Angle) -> Self {
        Self {
            shape: shape.into(),
            center,
            rotation,
            tilt: upright(),
        }
    }
    /// Tilt this [`Bore`] against the axis of the component.
    ///
    /// The angles turn the bore about the x and the y axis as the alignment of a node turns the
    /// node, but the other way round: a bore tilted by the angles a node is aligned by runs along
    /// the axis the light arrives on. Whether the tilt is less than 90° is checked when the bore is
    /// put into an [`Outline`].
    ///
    /// # Arguments
    ///
    /// * `about_x` - how far the bore is tilted about the x axis.
    /// * `about_y` - how far the bore is tilted about the y axis.
    #[must_use]
    pub const fn with_tilt(mut self, about_x: Angle, about_y: Angle) -> Self {
        self.tilt = (about_x, about_y);
        self
    }
    /// Returns the shape of this [`Bore`].
    #[must_use]
    pub const fn shape(&self) -> &OutlineShape {
        &self.shape
    }
    /// Returns where the centre of this [`Bore`] lies, relative to the axis of the component.
    #[must_use]
    pub const fn center(&self) -> Point2<Length> {
        self.center
    }
    /// Returns how far this [`Bore`] is turned about its centre.
    #[must_use]
    pub const fn rotation(&self) -> Angle {
        self.rotation
    }
    /// Returns how far this [`Bore`] is tilted about the x and the y axis (see [`Bore::with_tilt`]).
    #[must_use]
    pub const fn tilt(&self) -> (Angle, Angle) {
        self.tilt
    }
    /// The tilt of this bore as a turn, which turns the axis of the bore onto the axis of the
    /// component.
    fn tilting(&self) -> Rotation3<f64> {
        Rotation3::from_euler_angles(self.tilt.0.value, self.tilt.1.value, 0.0)
    }
    /// The turn from the bore's own frame into the frame of the outline: about the axis of the
    /// bore by its rotation, then tilted.
    fn turn(&self) -> Rotation3<f64> {
        self.tilting().inverse()
            * Rotation3::from_axis_angle(&Vector3::z_axis(), self.rotation.value)
    }
    /// The direction the bore runs along, in the frame of the outline.
    #[must_use]
    pub fn axis(&self) -> Vector3<f64> {
        self.tilting().inverse() * Vector3::z()
    }
    /// The region of this bore as an [`Aperture`] of the given type, placed where the bore is.
    fn aperture(&self, aperture_type: ApertureType) -> OpmResult<Aperture> {
        Aperture::new(
            self.shape.clone().into(),
            aperture_type,
            Some(self.center),
            Some(self.rotation),
        )
    }
    /// Move a point of the bore's own cross-section to where the bore crosses the plane of the
    /// outline: turned and tilted, then along the axis of the bore onto the plane, shifted off the
    /// axis of the component.
    #[must_use]
    pub fn placed(&self, point: Point2<Length>) -> Point2<Length> {
        let across = self.turn() * Vector3::new(point.x.value, point.y.value, 0.0);
        let axis = self.axis();
        let onto_plane = across - axis * (across.z / axis.z);
        Point2::new(
            meter!(self.center.x.value + onto_plane.x),
            meter!(self.center.y.value + onto_plane.y),
        )
    }
    /// The corners of the edge of this bore, placed where the bore is.
    fn corners(&self) -> Vec<Point2<Length>> {
        self.shape
            .corners()
            .into_iter()
            .map(|corner| self.placed(corner))
            .collect()
    }
    /// Follow the axis of this bore from where it crosses the plane of the outline to where it
    /// meets a surface above the plane.
    ///
    /// The surface is asked how high it lies above a position, the line along the axis is
    /// followed up to that height, and that is repeated from where the line got to until it stays
    /// put. A flat surface square to the axis of the component is met at the second step; a
    /// curved one closes in on the point.
    ///
    /// # Arguments
    ///
    /// - `start`: where the line along the axis of the bore crosses the plane of the outline.
    /// - `height`: how far above a position of the plane the surface lies.
    ///
    /// # Returns
    ///
    /// The position in the plane right below where the line meets the surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface cannot say how high it lies, or if the line
    /// does not settle on it.
    pub fn meeting(
        &self,
        start: Point2<Length>,
        height: &impl Fn(Point2<Length>) -> OpmResult<Length>,
    ) -> OpmResult<Point2<Length>> {
        let axis = self.axis();
        let (slope_x, slope_y) = (axis.x / axis.z, axis.y / axis.z);
        let mut position = start;
        for _ in 0..MEETING_STEPS {
            let above = height(position)?.value;
            let next = Point2::new(
                meter!(above.mul_add(slope_x, start.x.value)),
                meter!(above.mul_add(slope_y, start.y.value)),
            );
            let moved = (next.x - position.x)
                .value
                .hypot((next.y - position.y).value);
            position = next;
            if moved <= SETTLED {
                return Ok(position);
            }
        }
        Err(OpossumError::Other(
            "the axis of the bore does not settle on where it meets the surface".into(),
        ))
    }
    /// Return whether a point lies on or within this bore: across the axis of the bore, it lies
    /// within its shape.
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame of the outline: its plane at z = 0, the axis of the
    ///   component along z.
    fn contains_at(&self, point: Point3<Length>) -> bool {
        let from_center = Vector3::new(
            point.x.value - self.center.x.value,
            point.y.value - self.center.y.value,
            point.z.value,
        );
        let own = self.turn().inverse() * from_center;
        self.shape
            .contains(Point2::new(meter!(own.x), meter!(own.y)))
    }
    /// The edge of this bore as a closed, counter-clockwise polygon, placed where the bore is (see
    /// [`OutlineShape::outline_points`]).
    fn outline_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        Ok(self
            .shape
            .outline_points(segments)?
            .into_iter()
            .map(|point| self.placed(point))
            .collect())
    }
}

/// The edge of a component, centered on its axis, and the bore through it, if it has one.
///
/// The edge and the bore bound a region, and the bore lies within the edge; that is checked
/// whenever an [`Outline`] is made, also when one is read from a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OutlineFields")]
pub struct Outline {
    shape: OutlineShape,
    #[serde(skip_serializing_if = "Option::is_none")]
    bore: Option<Bore>,
}

/// The fields of an [`Outline`] as a file states them, before they are checked.
#[derive(Deserialize)]
struct OutlineFields {
    shape: OutlineShape,
    #[serde(default)]
    bore: Option<Bore>,
}

impl TryFrom<OutlineFields> for Outline {
    type Error = OpossumError;

    fn try_from(fields: OutlineFields) -> OpmResult<Self> {
        Self::new(fields.shape, fields.bore)
    }
}

impl Outline {
    /// Create a new [`Outline`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the edge of the component, centered on its axis.
    /// * `bore` - the bore through the component, if it has one.
    ///
    /// # Errors
    ///
    /// This function returns an error if the edge or the bore crosses or touches itself, so that it
    /// bounds no region, or if the bore reaches beyond the edge or touches it. The edges are
    /// compared as polygons, a circle sampled at 64 points; edges closer than 1 nm touch.
    pub fn new(shape: impl Into<OutlineShape>, bore: Option<Bore>) -> OpmResult<Self> {
        let outline = Self {
            shape: shape.into(),
            bore,
        };
        outline.check()?;
        Ok(outline)
    }
    /// Returns the edge of the component.
    #[must_use]
    pub const fn shape(&self) -> &OutlineShape {
        &self.shape
    }
    /// Returns the bore through the component, if it has one.
    #[must_use]
    pub const fn bore(&self) -> Option<&Bore> {
        self.bore.as_ref()
    }
    /// Return whether a point holds material: seen along the axis of the component it lies on or
    /// within the edge, and it lies outside the bore, which runs along its own axis (see
    /// [`Bore`]).
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame of the outline: its plane at z = 0, the axis of the
    ///   component along z.
    #[must_use]
    pub(crate) fn contains_at(&self, point: Point3<Length>) -> bool {
        self.within_edge(Point2::new(point.x, point.y)) && !self.in_bore_at(point)
    }
    /// Return whether a point of the plane lies on or within the edge, where the component
    /// reaches, whether it holds material there or lies in the bore.
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the plane of the outline with the axis at its origin.
    #[must_use]
    pub fn within_edge(&self, point: Point2<Length>) -> bool {
        self.shape.contains(point)
    }
    /// Return whether a point lies on or within the bore, if there is one, at whatever depth
    /// along the axis of the component: the bore runs along its own axis (see [`Bore`]).
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame of the outline: its plane at z = 0, the axis of the
    ///   component along z.
    #[must_use]
    pub fn in_bore_at(&self, point: Point3<Length>) -> bool {
        self.bore
            .as_ref()
            .is_some_and(|bore| bore.contains_at(point))
    }
    /// Return the points the component reaches farthest in, in the plane of the outline.
    ///
    /// They are the extreme points of the edge (see [`OutlineShape::extreme_points`]): the bore
    /// only takes material away.
    #[must_use]
    pub(crate) fn extreme_points(&self) -> Vec<Point2<Length>> {
        self.shape.extreme_points()
    }
    /// Return how far the component reaches from its axis.
    ///
    /// # Returns
    ///
    /// The largest distance of a point of the edge from the axis.
    #[must_use]
    pub(crate) fn transversal_reach(&self) -> Length {
        self.extreme_points()
            .iter()
            .fold(Length::zero(), |reach, point| {
                Length::max(reach, meter!(point.x.value.hypot(point.y.value)))
            })
    }
    /// Triangulate the area of this outline: the region within its edge and outside its bore.
    ///
    /// The edge and the bore are sampled with about `segments` points each (see
    /// [`OutlineShape::outline_points`]); those rims are held fixed and the inside is filled (see
    /// [`OutlineMesh`]).
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of points to aim for on each rim, at least 3
    ///
    /// # Returns
    ///
    /// The triangulated area, in the plane of the outline.
    ///
    /// # Errors
    ///
    /// This function returns an error if an edge cannot be sampled, or if the sampled rims do not
    /// bound the area, such as a bore that the coarser sampling of the edge cuts into.
    pub fn triangulate(&self, segments: usize) -> OpmResult<OutlineMesh> {
        self.triangulate_on(segments, |_| Ok(Length::zero()))
    }
    /// Triangulate the area of this outline where it meets a surface above its plane: the region
    /// within its edge and outside its bore, each taken where it meets the surface.
    ///
    /// The edge runs along the axis of the component and meets the surface right above where it
    /// lies in the plane. The bore runs along its own axis (see [`Bore`]), so a tilted bore meets
    /// the surface beside where it crosses the plane. Every rim keeps the number and the order of
    /// its points whatever the surface, which is what lets the two faces of a body be joined along
    /// them.
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of points to aim for on each rim, at least 3
    /// - `height`: how far above a position of the plane the surface lies, along the axis of the
    ///   component
    ///
    /// # Returns
    ///
    /// The triangulated area, its points the positions in the plane right below the surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if an edge cannot be sampled, if the surface cannot be told
    /// where the bore meets it, or if the rims do not bound the area.
    pub(crate) fn triangulate_on(
        &self,
        segments: usize,
        height: impl Fn(Point2<Length>) -> OpmResult<Length>,
    ) -> OpmResult<OutlineMesh> {
        let mut rims = vec![self.shape.outline_points(segments)?];
        if let Some(bore) = &self.bore {
            let mut rim = bore
                .outline_points(segments)?
                .into_iter()
                .map(|point| bore.meeting(point, &height))
                .collect::<OpmResult<Vec<_>>>()?;
            // The bore takes its area away, so its rim runs clockwise.
            rim.reverse();
            rims.push(rim);
        }
        OutlineMesh::from_rims(&rims, |point| {
            self.within_edge(point)
                && !self.bore.as_ref().is_some_and(|bore| {
                    height(point)
                        .is_ok_and(|above| bore.contains_at(Point3::new(point.x, point.y, above)))
                })
        })
    }
    /// Check that the edge and the bore bound a region, that the bore is tilted by less than 90°
    /// and that it crosses the plane of the outline within the edge without touching it.
    ///
    /// A shape bounds a region if its edge neither crosses nor touches itself. A bore within the
    /// edge has edges that do not meet the edge of the component and a corner inside it.
    fn check(&self) -> OpmResult<()> {
        let edge_ring = self.shape.corners();
        if ring_meets_itself(&edge_ring) {
            return Err(OpossumError::Properties(
                "the edge of the component crosses or touches itself".into(),
            ));
        }
        let Some(bore) = &self.bore else {
            return Ok(());
        };
        let along_axis = bore.axis().z;
        if !along_axis.is_finite() || along_axis <= f64::EPSILON {
            return Err(OpossumError::Properties(
                "the bore must be tilted by less than 90° against the axis of the component".into(),
            ));
        }
        let ring = bore.corners();
        if ring_meets_itself(&ring) {
            return Err(OpossumError::Properties(
                "the bore crosses or touches itself".into(),
            ));
        }
        self.check_bore_ring(&edge_ring, &ring)
    }
    /// Check that the bore meets a surface above the plane of this outline within the edge
    /// without touching it, as it has to cross the plane itself (see [`Outline::new`]): a tilted
    /// bore meets the far face of a body beside where it crosses the plane.
    ///
    /// # Arguments
    ///
    /// - `height`: how far above a position of the plane the surface lies, along the axis of the
    ///   component
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface cannot be told where the bore meets it, or
    /// if the bore meets it on or beyond the edge.
    pub(crate) fn check_bore_on(
        &self,
        height: impl Fn(Point2<Length>) -> OpmResult<Length>,
    ) -> OpmResult<()> {
        let Some(bore) = &self.bore else {
            return Ok(());
        };
        let ring = bore
            .corners()
            .into_iter()
            .map(|corner| bore.meeting(corner, &height))
            .collect::<OpmResult<Vec<_>>>()?;
        self.check_bore_ring(&self.shape.corners(), &ring)
    }
    /// Check that the corners of the bore, where it meets a plane or a surface, lie within the
    /// edge without touching it: the two rings do not meet and a corner of the bore lies inside.
    fn check_bore_ring(
        &self,
        edge_ring: &[Point2<Length>],
        ring: &[Point2<Length>],
    ) -> OpmResult<()> {
        if rings_meet(edge_ring, ring) || !ring.first().is_some_and(|&c| self.shape.contains(c)) {
            return Err(OpossumError::Properties(
                "the bore does not lie within the edge of the component without touching it".into(),
            ));
        }
        Ok(())
    }
}

impl From<OutlineShape> for Outline {
    fn from(shape: OutlineShape) -> Self {
        Self { shape, bore: None }
    }
}

/// The edges of a closed polygon, each given by its end points in meters, in the order the
/// polygon runs through them.
fn ring_edges(ring: &[Point2<Length>]) -> Vec<(Point2<f64>, Point2<f64>)> {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(from, to)| (from.map(|c| c.value), to.map(|c| c.value)))
        .collect()
}

/// The region of an outline as an [`Aperture`], to be plotted: its edge as a hole, and its bore
/// as an obstruction in it, by its cross-section and without its tilt.
///
/// # Errors
///
/// This function returns an error if the bore cannot be placed as an [`Aperture`].
impl TryFrom<&Outline> for Aperture {
    type Error = OpossumError;

    fn try_from(outline: &Outline) -> OpmResult<Self> {
        let edge = Self::new(outline.shape.clone().into(), ApertureType::Hole, None, None)?;
        let Some(bore) = &outline.bore else {
            return Ok(edge);
        };
        Self::new_stack(
            vec![edge, bore.aperture(ApertureType::Obstruction)?],
            ApertureType::Hole,
            None,
            None,
        )
    }
}

/// Whether the edges of two closed polygons cross or touch each other: come closer than
/// [`TOUCHING`].
fn rings_meet(one: &[Point2<Length>], other: &[Point2<Length>]) -> bool {
    let other_edges = ring_edges(other);
    ring_edges(one).iter().any(|&edge| {
        other_edges
            .iter()
            .any(|&other_edge| segments_meet(edge, other_edge))
    })
}

/// Whether the edge of a closed polygon crosses or touches itself, so that it bounds no region.
///
/// Two edges following each other share a corner and may meet nowhere else; any other two edges
/// may not meet at all. That also refuses a corner given twice and corners all on one line.
fn ring_meets_itself(ring: &[Point2<Length>]) -> bool {
    let edges = ring_edges(ring);
    let last = edges.len().saturating_sub(1);
    edges.iter().enumerate().any(|(number, &edge)| {
        edges
            .iter()
            .enumerate()
            .skip(number + 1)
            .any(|(other_number, &other)| {
                if other_number == number + 1 {
                    neighbors_meet(edge, other)
                } else if number == 0 && other_number == last {
                    neighbors_meet(other, edge)
                } else {
                    segments_meet(edge, other)
                }
            })
    })
}

/// Whether an edge and the edge following it on a polygon meet anywhere but in the corner they
/// share: the far end of either comes closer than [`TOUCHING`] to the other.
fn neighbors_meet(
    (from, corner): (Point2<f64>, Point2<f64>),
    (next_corner, to): (Point2<f64>, Point2<f64>),
) -> bool {
    distance_to_segment(from, next_corner, to) < TOUCHING
        || distance_to_segment(to, from, corner) < TOUCHING
}

/// Whether two segments, each given by its end points in meters, cross or come closer than
/// [`TOUCHING`].
fn segments_meet(
    (from, to): (Point2<f64>, Point2<f64>),
    (other_from, other_to): (Point2<f64>, Point2<f64>),
) -> bool {
    // Which side of the line through `start` and `end` a point lies on, by the sign.
    let side = |start: Point2<f64>, end: Point2<f64>, point: Point2<f64>| {
        (end - start).perp(&(point - start))
    };
    let apart = |one: f64, other: f64| one * other < 0.0;
    let cross = apart(side(from, to, other_from), side(from, to, other_to))
        && apart(
            side(other_from, other_to, from),
            side(other_from, other_to, to),
        );
    cross
        || [
            distance_to_segment(from, other_from, other_to),
            distance_to_segment(to, other_from, other_to),
            distance_to_segment(other_from, from, to),
            distance_to_segment(other_to, from, to),
        ]
        .into_iter()
        .any(|distance| distance < TOUCHING)
}

/// The distance of a point from the segment between two end points.
fn distance_to_segment(point: Point2<f64>, from: Point2<f64>, to: Point2<f64>) -> f64 {
    let run = to - from;
    let along = if run.norm_squared() > 0.0 {
        ((point - from).dot(&run) / run.norm_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (from + run * along - point).norm()
}

/// The value of a node's optic shape: how far the component reaches transversally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OpticShape {
    /// Unbounded, which only an idealization, a detector or a dummy may be.
    Open,
    /// Bounded by an edge on the axis, possibly with a bore through it.
    Bounded(Outline),
}

impl OpticShape {
    /// Return the outline of this optic shape, which the geometry bounds the component with.
    ///
    /// # Returns
    ///
    /// The outline, or `None` if the optic shape is open.
    #[must_use]
    pub const fn outline(&self) -> Option<&Outline> {
        match self {
            Self::Open => None,
            Self::Bounded(outline) => Some(outline),
        }
    }
}

impl Display for OutlineShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Circle(_) => write!(f, "Circle"),
            Self::Rectangle(_) => write!(f, "Rectangle"),
            Self::Polygon(_) => write!(f, "Polygon"),
        }
    }
}
impl Display for OpticShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::Bounded(outline) => match outline.bore {
                None => write!(f, "{}", outline.shape),
                Some(_) => write!(f, "{} with a bore", outline.shape),
            },
        }
    }
}

impl From<Outline> for OpticShape {
    fn from(outline: Outline) -> Self {
        Self::Bounded(outline)
    }
}
impl From<CircleShape> for OpticShape {
    fn from(shape: CircleShape) -> Self {
        Self::Bounded(OutlineShape::from(shape).into())
    }
}
impl From<RectangleShape> for OpticShape {
    fn from(shape: RectangleShape) -> Self {
        Self::Bounded(OutlineShape::from(shape).into())
    }
}
impl From<PolygonShape> for OpticShape {
    fn from(shape: PolygonShape) -> Self {
        Self::Bounded(OutlineShape::from(shape).into())
    }
}
impl From<OpticShape> for Proptype {
    fn from(optic_shape: OpticShape) -> Self {
        Self::OpticShape(optic_shape)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{degree, millimeter, utils::test_helper::helper::l_shape_corners};
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use std::f64::consts::PI;

    fn circle(radius: f64) -> OpmResult<CircleShape> {
        CircleShape::new(millimeter!(radius))
    }
    fn rectangle(width: f64, height: f64) -> OpmResult<RectangleShape> {
        RectangleShape::new(millimeter!(width), millimeter!(height))
    }
    fn triangle(corners: [(f64, f64); 3]) -> OpmResult<PolygonShape> {
        PolygonShape::new(corners.map(|(x, y)| millimeter!(x, y)).to_vec())
    }
    /// A bore of the given shape centred at (x, y) mm and turned by the given angle in degrees.
    fn bore(shape: impl Into<OutlineShape>, x: f64, y: f64, rotation: f64) -> Bore {
        Bore::new(shape, millimeter!(x, y), degree!(rotation))
    }
    /// The outline of a bounded optic shape.
    fn region(optic_shape: &OpticShape) -> OpmResult<Outline> {
        optic_shape
            .outline()
            .cloned()
            .ok_or_else(|| OpossumError::Other("the optic shape is open".into()))
    }
    /// Whether the region holds material at (x, y) mm, and thus transmits there.
    fn transmits(region: &Outline, x: f64, y: f64) -> bool {
        region.contains_at(millimeter!(x, y, 0.0))
    }
    /// The area in mm² the region transmits on, counted on a grid of 0.05 mm over the square of
    /// the given half width in mm.
    fn area(region: &Outline, half_width: u32) -> f64 {
        let step = 0.05;
        let steps = 40 * half_width;
        let at = |index: u32| (f64::from(index) + 0.5).mul_add(step, -f64::from(half_width));
        let transmitting = (0..steps)
            .flat_map(|i| (0..steps).map(move |j| (i, j)))
            .filter(|&(i, j)| transmits(region, at(i), at(j)))
            .count();
        to_f64(transmitting) * step * step
    }

    /// A ring transmits where the stack of a hole and a central obstruction it replaces did, on an
    /// area of π (R² - r²), and is plotted as that stack.
    #[test]
    fn a_ring_is_the_region_of_the_stack_it_replaces() -> OpmResult<()> {
        let ring = OpticShape::from(Outline::new(
            circle(12.5)?,
            Some(bore(circle(2.0)?, 0.0, 0.0, 0.0)),
        )?);
        let ring = region(&ring)?;
        let stack = Aperture::new_stack(
            vec![
                Aperture::new_circle(millimeter!(12.5), ApertureType::Hole, None)?,
                Aperture::new_circle(millimeter!(2.0), ApertureType::Obstruction, None)?,
            ],
            ApertureType::Hole,
            None,
            None,
        )?;
        assert_eq!(Aperture::try_from(&ring)?, stack);
        for x in (-130..=130).map(|tenth| f64::from(tenth) / 10.0) {
            for y in [0.0, 1.0, 1.9, 2.1, 7.0, 12.4] {
                assert_eq!(
                    transmits(&ring, x, y),
                    stack.apodize(&millimeter!(x, y, 0.0)) > 0.0,
                    "({x}, {y}) mm"
                );
            }
        }
        assert_relative_eq!(
            area(&ring, 13),
            PI * (12.5_f64.powi(2) - 2.0_f64.powi(2)),
            max_relative = 2e-3
        );
        Ok(())
    }
    /// A square plate of 20 mm with a turned slot beside its axis holds material on the plate but
    /// not in the slot, so it transmits on 400 mm² less the slot.
    #[test]
    fn a_plate_transmits_beside_its_shifted_and_turned_bore() -> OpmResult<()> {
        let plate = region(&OpticShape::from(Outline::new(
            rectangle(20.0, 20.0)?,
            Some(bore(rectangle(3.0, 1.0)?, -4.0, -3.0, 30.0)),
        )?))?;
        assert!(transmits(&plate, 0.0, 0.0));
        assert!(transmits(&plate, 9.9, -9.9));
        assert!(!transmits(&plate, 10.1, 0.0));
        assert!(!transmits(&plate, -4.0, -3.0));
        // 1.3 mm from the centre of the slot along it, which an unturned slot would not reach, and
        // as far turned the other way.
        let (sin, cos) = 30.0_f64.to_radians().sin_cos();
        let x = 1.3_f64.mul_add(cos, -4.0);
        assert!(!transmits(&plate, x, 1.3_f64.mul_add(sin, -3.0)));
        assert!(transmits(&plate, x, 1.3_f64.mul_add(-sin, -3.0)));
        assert_relative_eq!(area(&plate, 11), 400.0 - 3.0, max_relative = 2e-3);
        Ok(())
    }
    /// A triangular plate holds material within its edge but not in its turned square bore.
    #[test]
    fn a_polygon_holds_material_beside_its_bore() -> OpmResult<()> {
        let plate = region(&OpticShape::from(Outline::new(
            triangle([(-10.0, -10.0), (10.0, -10.0), (0.0, 10.0)])?,
            Some(bore(rectangle(2.0, 2.0)?, 0.0, -4.0, 45.0)),
        )?))?;
        for ((x, y), holds) in [
            ((0.0, 0.0), true),
            ((-9.9, -9.9), true),
            ((0.0, 9.9), true),
            ((5.0, 0.5), false),
            ((0.0, -10.1), false),
            ((0.0, -4.0), false),
            // inside the corner of the turned square, outside an unturned one
            ((0.0, -2.7), false),
            // inside an unturned square, outside the turned one
            ((0.9, -3.1), true),
        ] {
            assert_eq!(transmits(&plate, x, y), holds, "({x}, {y}) mm");
        }
        Ok(())
    }
    /// An outline reaches as far as its edge, wherever its bore is: a ring as far as its outer
    /// circle, a rectangle to its corners, a polygon to its farthest corner. Its axis-aligned
    /// bounds are those of its edge.
    #[test]
    fn an_outline_reaches_as_far_as_its_edge() -> OpmResult<()> {
        let reach_and_bounds = |optic_shape: OpticShape| -> OpmResult<(f64, [f64; 4])> {
            let outline = region(&optic_shape)?;
            let points = outline.extreme_points();
            let bound = |pick: fn(&Point2<Length>) -> f64, lowest: bool| {
                points.iter().map(pick).fold(
                    if lowest {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    },
                    |bound, value| {
                        if lowest {
                            bound.min(value)
                        } else {
                            bound.max(value)
                        }
                    },
                )
            };
            Ok((
                outline.transversal_reach().value,
                [
                    bound(|point| point.x.value, true),
                    bound(|point| point.x.value, false),
                    bound(|point| point.y.value, true),
                    bound(|point| point.y.value, false),
                ],
            ))
        };
        let ring = Outline::new(circle(12.5)?, Some(bore(circle(2.0)?, 3.0, 0.0, 0.0)))?;
        assert_eq!(
            reach_and_bounds(ring.into())?,
            (0.0125, [-0.0125, 0.0125, -0.0125, 0.0125])
        );
        let (reach, bounds) = reach_and_bounds(rectangle(20.0, 10.0)?.into())?;
        assert_relative_eq!(reach, 0.01_f64.hypot(0.005), max_relative = 1e-15);
        assert_eq!(bounds, [-0.01, 0.01, -0.005, 0.005]);
        let (reach, bounds) =
            reach_and_bounds(triangle([(-3.0, -4.0), (6.0, 0.0), (0.0, 2.0)])?.into())?;
        assert_relative_eq!(reach, 0.006, max_relative = 1e-15);
        assert_eq!(bounds, [-0.003, 0.006, -0.004, 0.002]);
        Ok(())
    }
    /// A bore tilted by 45° about y runs along its own axis, (−sin 45°, 0, cos 45°): its circle of
    /// 1 mm radius is the cross-section across that axis, so it crosses the plane of the outline
    /// in an ellipse reaching 1/cos 45° = 1.414 mm along x and 1 mm along y, and 6 mm deeper its
    /// center lies 6 mm further towards −x, the bore reaching from −7.414 mm to −4.586 mm there.
    #[test]
    fn a_tilted_bore_runs_along_its_own_axis() -> OpmResult<()> {
        let tilted = Outline::new(
            rectangle(20.0, 20.0)?,
            Some(bore(circle(1.0)?, 0.0, 0.0, 0.0).with_tilt(degree!(0.0), degree!(45.0))),
        )?;
        for ((x, y), inside) in [
            ((1.3, 0.0), true),
            ((-1.3, 0.0), true),
            ((1.5, 0.0), false),
            ((0.0, 0.9), true),
            ((0.0, 1.1), false),
        ] {
            assert_eq!(
                tilted.in_bore_at(millimeter!(x, y, 0.0)),
                inside,
                "({x}, {y}) mm"
            );
        }
        for (x, inside) in [
            (-6.0, true),
            (-4.7, true),
            (-7.3, true),
            (-4.4, false),
            (0.0, false),
        ] {
            assert_eq!(
                tilted.in_bore_at(millimeter!(x, 0.0, 6.0)),
                inside,
                "({x}, 0, 6) mm"
            );
        }
        Ok(())
    }
    /// A bore tilted by 90° or more would run along the plane of the outline or out of it again:
    /// it is refused.
    #[test]
    fn a_bore_tilted_by_90_degrees_or_more_is_refused() -> OpmResult<()> {
        for (about_x, about_y) in [(90.0, 0.0), (0.0, 90.0), (0.0, 120.0), (-95.0, 0.0)] {
            let tilted =
                bore(circle(1.0)?, 0.0, 0.0, 0.0).with_tilt(degree!(about_x), degree!(about_y));
            let error = Outline::new(rectangle(20.0, 20.0)?, Some(tilted))
                .expect_err("a bore tilted that far was accepted")
                .to_string();
            assert!(error.contains("90°"), "({about_x}°, {about_y}°): {error}");
        }
        Ok(())
    }
    /// An untilted bore is written without a tilt, as before bores could be tilted; a tilted one
    /// keeps its tilt through a file.
    #[test]
    fn a_bore_states_its_tilt_only_if_it_has_one() -> OpmResult<()> {
        let written = |bore: Bore| -> OpmResult<(Outline, String)> {
            let outline = Outline::new(rectangle(20.0, 20.0)?, Some(bore))?;
            let file = ron::to_string(&outline).map_err(|e| OpossumError::Other(e.to_string()))?;
            Ok((outline, file))
        };
        let (_, file) = written(bore(circle(1.0)?, 0.0, 0.0, 0.0))?;
        assert!(!file.contains("tilt"), "{file}");
        let (tilted, file) =
            written(bore(circle(1.0)?, 0.0, 0.0, 0.0).with_tilt(degree!(10.0), degree!(-20.0)))?;
        assert!(file.contains("tilt"), "{file}");
        assert_eq!(ron::from_str::<Outline>(&file).ok(), Some(tilted));
        Ok(())
    }
    /// The bore lies within the edge without touching it. A bore 1 µm from the edge lies within.
    #[test]
    fn a_bore_lies_within_the_edge() -> OpmResult<()> {
        let plate = |bore: Bore| Outline::new(rectangle(20.0, 20.0)?, Some(bore));
        assert!(plate(bore(rectangle(2.0, 2.0)?, 8.999, 0.0, 0.0)).is_ok());
        for (case, bore) in [
            ("outside the edge", bore(circle(1.0)?, 15.0, 0.0, 0.0)),
            ("across the edge", bore(circle(2.0)?, 9.5, 0.0, 0.0)),
            (
                "turned across the edge",
                bore(rectangle(1.0, 6.0)?, 8.0, 0.0, 45.0),
            ),
            (
                "touching the edge",
                bore(rectangle(2.0, 2.0)?, 9.0, 0.0, 0.0),
            ),
            ("around the plate", bore(circle(20.0)?, 0.0, 0.0, 0.0)),
            (
                "touching the edge with a side",
                bore(
                    triangle([(8.0, 0.0), (10.0, 1.0), (10.0, -1.0)])?,
                    0.0,
                    0.0,
                    0.0,
                ),
            ),
        ] {
            assert!(plate(bore).is_err(), "a bore {case} is accepted");
        }
        Ok(())
    }
    /// The edge and the bore bound a region: a polygon that crosses or touches itself, runs back
    /// on itself or lies on one line encloses nothing and is refused, as the edge and as the bore.
    /// A star and a pentagon bound a region.
    #[test]
    fn every_shape_of_an_outline_bounds_a_region() -> OpmResult<()> {
        let polygon = |corners: &[(f64, f64)]| {
            PolygonShape::new(corners.iter().map(|&(x, y)| millimeter!(x, y)).collect())
        };
        let with_bore = |shape: PolygonShape| -> OpmResult<Outline> {
            Outline::new(rectangle(20.0, 20.0)?, Some(bore(shape, 0.0, 0.0, 0.0)))
        };
        for (case, corners) in [
            (
                "crossing itself",
                vec![(-2.0, -2.0), (2.0, 2.0), (2.0, -2.0), (-2.0, 2.0)],
            ),
            (
                "touching itself in a corner",
                vec![
                    (0.0, 0.0),
                    (-3.0, 2.0),
                    (-3.0, -2.0),
                    (0.0, 0.0),
                    (3.0, -2.0),
                    (3.0, 2.0),
                ],
            ),
            (
                "with a corner twice in a row",
                vec![(0.0, 0.0), (3.0, 0.0), (3.0, 0.0), (0.0, 3.0)],
            ),
            ("on one line", vec![(0.0, 0.0), (1.0, 0.0), (3.0, 0.0)]),
            (
                "running back on itself",
                vec![(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (4.0, 1.0)],
            ),
        ] {
            let shape = polygon(&corners)?;
            let error = Outline::new(shape.clone(), None)
                .expect_err(case)
                .to_string();
            assert!(error.contains("the edge"), "an edge {case}: {error}");
            let error = with_bore(shape).expect_err(case).to_string();
            assert!(error.contains("the bore"), "a bore {case}: {error}");
        }
        let star = PolygonShape::new(
            (0..10)
                .map(|corner| {
                    let angle = to_f64(corner) * PI / 5.0;
                    let radius = if corner % 2 == 0 { 4.0 } else { 1.6 };
                    millimeter!(radius * angle.cos(), radius * angle.sin())
                })
                .collect(),
        )?;
        let pentagon = polygon(&[
            (0.0, 4.0),
            (-3.804, 1.236),
            (-2.351, -3.236),
            (2.351, -3.236),
            (3.804, 1.236),
        ])?;
        for shape in [star, pentagon] {
            assert!(Outline::new(shape.clone(), None).is_ok(), "{shape:?}");
            assert!(with_bore(shape.clone()).is_ok(), "{shape:?}");
        }
        Ok(())
    }
    /// A file cannot state a bore the type refuses: it is checked when the outline is read.
    #[test]
    fn an_outline_read_from_a_file_is_checked() -> OpmResult<()> {
        let within = Outline::new(
            rectangle(20.0, 20.0)?,
            Some(bore(circle(2.0)?, -4.0, 0.0, 0.0)),
        )?;
        let file = ron::to_string(&within).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(ron::from_str::<Outline>(&file).ok(), Some(within));
        assert!(file.contains("-0.004"));
        let across_the_edge = file.replace("-0.004", "-0.0095");
        assert!(ron::from_str::<Outline>(&across_the_edge).is_err());
        Ok(())
    }
    /// An outline without a bore is written without one, and the bore list older files state
    /// is no field of an outline anymore: it is read past.
    #[test]
    fn an_outline_without_a_bore_states_none() -> OpmResult<()> {
        let plate = Outline::new(rectangle(20.0, 20.0)?, None)?;
        let file = ron::to_string(&plate).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!file.contains("bore"), "{file}");
        let older = file
            .strip_suffix(')')
            .map(|fields| format!("{fields},bores:[])"))
            .ok_or_else(|| OpossumError::Other(format!("not a struct: {file}")))?;
        assert_eq!(ron::from_str::<Outline>(&older).ok(), Some(plate));
        Ok(())
    }
    #[test]
    fn display() -> OpmResult<()> {
        let plate = |bore| -> OpmResult<OpticShape> {
            Ok(Outline::new(rectangle(20.0, 20.0)?, bore)?.into())
        };
        assert_eq!(OpticShape::Open.to_string(), "Open");
        assert_eq!(OpticShape::from(circle(5.0)?).to_string(), "Circle");
        assert_eq!(plate(None)?.to_string(), "Rectangle");
        assert_eq!(
            plate(Some(bore(circle(1.0)?, 4.0, 0.0, 0.0)))?.to_string(),
            "Rectangle with a bore"
        );
        Ok(())
    }

    /// Shortest distance from a point to the closed ring through `corners`, in meters.
    ///
    /// Used to check that a resampled edge really stayed on the edge it describes.
    fn distance_to_ring(point: &Point2<Length>, corners: &[Point2<Length>]) -> f64 {
        (0..corners.len())
            .map(|from| {
                let to = (from + 1) % corners.len();
                let edge = corners[to] - corners[from];
                let offset = point - corners[from];
                let edge_length_squared = edge.x.value.mul_add(edge.x.value, edge.y.value.powi(2));
                let along = if edge_length_squared == 0.0 {
                    0.0
                } else {
                    (offset
                        .x
                        .value
                        .mul_add(edge.x.value, offset.y.value * edge.y.value)
                        / edge_length_squared)
                        .clamp(0.0, 1.0)
                };
                along
                    .mul_add(-edge.x.value, offset.x.value)
                    .hypot(along.mul_add(-edge.y.value, offset.y.value))
            })
            .fold(f64::INFINITY, f64::min)
    }
    #[test]
    fn outline_points_of_a_circle_lie_on_it() -> OpmResult<()> {
        let radius = millimeter!(12.5);
        let outline = OutlineShape::from(CircleShape::new(radius)?).outline_points(64)?;
        assert_eq!(outline.len(), 64);
        for point in &outline {
            assert_abs_diff_eq!(
                point.x.value.hypot(point.y.value),
                radius.value,
                epsilon = 1e-12
            );
        }
        assert!(
            super::super::doubled_signed_area(&outline) > 0.0,
            "must run counter-clockwise"
        );
        assert_ne!(
            outline[0],
            outline[outline.len() - 1],
            "the ring must not repeat its first point"
        );
        Ok(())
    }
    #[test]
    fn outline_points_of_a_rectangle_keep_its_corners() -> OpmResult<()> {
        let (width, height) = (millimeter!(30.0), millimeter!(10.0));
        let outline = OutlineShape::from(RectangleShape::new(width, height)?).outline_points(40)?;
        let corners = [
            Point2::new(width / 2., height / 2.),
            Point2::new(-width / 2., height / 2.),
            Point2::new(-width / 2., -height / 2.),
            Point2::new(width / 2., -height / 2.),
        ];
        for corner in &corners {
            assert!(
                outline.contains(corner),
                "corner {corner:?} is missing from the outline"
            );
        }
        for point in &outline {
            assert_abs_diff_eq!(distance_to_ring(point, &corners), 0.0, epsilon = 1e-12);
        }
        assert!(super::super::doubled_signed_area(&outline) > 0.0);
        // The perimeter splits into 15 + 5 + 15 + 5 intervals, so the points asked for are handed
        // out exactly: a long side is sampled as densely as a short one, not as coarsely.
        assert_eq!(outline.len(), 40);
        Ok(())
    }
    #[test]
    fn outline_points_of_a_polygon_keep_its_corners() -> OpmResult<()> {
        // An L, so that a shape which is not convex is covered as well.
        let corners = l_shape_corners();
        let outline = OutlineShape::from(PolygonShape::new(corners.clone())?).outline_points(30)?;
        for corner in &corners {
            assert!(
                outline.contains(corner),
                "corner {corner:?} is missing from the outline"
            );
        }
        for point in &outline {
            assert_abs_diff_eq!(distance_to_ring(point, &corners), 0.0, epsilon = 1e-12);
        }
        assert!(super::super::doubled_signed_area(&outline) > 0.0);
        assert!(outline.len() >= corners.len());
        Ok(())
    }
    /// The outline is promised counter-clockwise, so a polygon given the other way round has to be
    /// turned around rather than handed on as it is.
    #[test]
    fn a_clockwise_polygon_is_turned_around() -> OpmResult<()> {
        let counter_clockwise = vec![
            millimeter!(-10.0, -10.0),
            millimeter!(10.0, -10.0),
            millimeter!(10.0, 10.0),
            millimeter!(-10.0, 10.0),
        ];
        let mut clockwise = counter_clockwise.clone();
        clockwise.reverse();
        for corners in [counter_clockwise, clockwise] {
            let outline = OutlineShape::from(PolygonShape::new(corners)?).outline_points(8)?;
            assert!(super::super::doubled_signed_area(&outline) > 0.0);
        }
        Ok(())
    }
    #[test]
    fn a_shifted_bore_moves_its_outline() -> OpmResult<()> {
        let (shift, radius) = (millimeter!(5.0, -3.0), millimeter!(12.5));
        let outline =
            Bore::new(CircleShape::new(radius)?, shift, degree!(0.0)).outline_points(32)?;
        assert_eq!(outline.len(), 32);
        for point in &outline {
            assert_abs_diff_eq!(
                (point.x - shift.x).value.hypot((point.y - shift.y).value),
                radius.value,
                epsilon = 1e-12
            );
        }
        assert!(super::super::doubled_signed_area(&outline) > 0.0);
        Ok(())
    }
    #[test]
    fn a_turned_bore_turns_its_outline() -> OpmResult<()> {
        let (width, height) = (millimeter!(30.0), millimeter!(10.0));
        let outline = Bore::new(
            RectangleShape::new(width, height)?,
            millimeter!(0.0, 0.0),
            degree!(90.0),
        )
        .outline_points(40)?;
        // A quarter turn swaps the sides, whichever way it goes: the outline now reaches half the
        // height along x and half the width along y.
        let reach = |coordinate: fn(&Point2<Length>) -> Length| {
            outline
                .iter()
                .map(|point| coordinate(point).value.abs())
                .fold(0.0, f64::max)
        };
        assert_abs_diff_eq!(reach(|p| p.x), (height / 2.).value, epsilon = 1e-12);
        assert_abs_diff_eq!(reach(|p| p.y), (width / 2.).value, epsilon = 1e-12);
        assert!(super::super::doubled_signed_area(&outline) > 0.0);
        Ok(())
    }
    /// An upright bore places a point of its cross-section shifted by its center and turned by its
    /// rotation, and nothing more.
    #[test]
    fn an_upright_bore_shifts_and_turns_a_point() -> OpmResult<()> {
        let placed = bore(circle(1.0)?, 5.0, -3.0, 90.0).placed(millimeter!(2.0, 0.0));
        assert_abs_diff_eq!(placed.x.value, millimeter!(5.0).value, epsilon = 1e-12);
        assert_abs_diff_eq!(placed.y.value, millimeter!(-1.0).value, epsilon = 1e-12);
        Ok(())
    }
    /// A tilted bore places a point of its cross-section along its axis onto the plane of the
    /// outline: the point keeps its distance from the axis, while its ring is stretched across the
    /// plane.
    #[test]
    fn a_tilted_bore_places_a_point_along_its_axis() -> OpmResult<()> {
        let radius = 2.0;
        let tilted =
            bore(circle(radius)?, 1.0, -1.0, 30.0).with_tilt(degree!(20.0), degree!(-10.0));
        let axis = tilted.axis();
        let center = Vector3::new(millimeter!(1.0).value, millimeter!(-1.0).value, 0.0);
        let mut reach = 0.0_f64;
        for step in 0..8 {
            let angle = f64::from(step) * PI / 4.0;
            let placed = tilted.placed(millimeter!(radius * angle.cos(), radius * angle.sin()));
            let from_center = Vector3::new(placed.x.value, placed.y.value, 0.0) - center;
            let off_axis = from_center - axis * from_center.dot(&axis);
            assert_abs_diff_eq!(off_axis.norm(), millimeter!(radius).value, epsilon = 1e-12);
            reach = reach.max(from_center.norm());
        }
        assert!(
            reach > millimeter!(radius).value * 1.01,
            "the tilt did not stretch the ring across the plane"
        );
        Ok(())
    }
    #[test]
    fn an_outline_needs_at_least_three_points() -> OpmResult<()> {
        let shape = OutlineShape::from(CircleShape::new(millimeter!(1.0))?);
        assert!(shape.outline_points(0).is_err());
        assert!(shape.outline_points(2).is_err());
        assert!(shape.outline_points(3).is_ok());
        Ok(())
    }
    #[test]
    fn a_ring_without_area_has_no_outline() -> OpmResult<()> {
        let collinear = [
            millimeter!(-1.0, 0.0),
            millimeter!(0.0, 0.0),
            millimeter!(1.0, 0.0),
        ];
        assert!(super::super::resample_ring(&collinear, 16).is_err());
        assert!(super::super::resample_ring(&collinear[0..2], 16).is_err());
        let point_circle = OutlineShape::from(CircleShape::new(millimeter!(0.0))?);
        assert!(point_circle.outline_points(16).is_err());
        Ok(())
    }
}
