#![warn(missing_docs)]
//! The value of a component's optic shape: an outline on its axis with bores through it.
//!
//! A component has an edge and, possibly, bores through it, such as a mirror with a hole for the
//! beam to pass. That is all the optic shape states: unlike a general [`Aperture`], it has no
//! soft edges, no obstructions and no stack of arbitrary shapes. The geometry works with the
//! [`Outline`] directly (see [`OpticShape::outline`]); an [`Aperture`] is made of it only to be
//! plotted.

use super::{
    Aperture, ApertureShape, ApertureType, CircleShape, PolygonShape, RectangleShape, Shape,
};
use crate::{
    error::{OpmResult, OpossumError},
    meter,
    properties::Proptype,
    utils::{default_from_name::DefaultFromName, math_utils::to_f64},
};
use nalgebra::{Point2, Point3};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use std::{f64::consts::TAU, fmt::Display};
use strum::EnumIter;
use uom::si::f64::{Angle, Length};

/// How many points the edge of a circle is sampled with when the bores are checked.
const CIRCLE_POINTS: usize = 64;
/// How close two edges may come, in meters, before they count as touching.
const TOUCHING: f64 = 1e-9;

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
            Self::Rectangle(rectangle) => {
                let (x, y) = (rectangle.width() / 2.0, rectangle.height() / 2.0);
                vec![
                    Point2::new(x, y),
                    Point2::new(-x, y),
                    Point2::new(-x, -y),
                    Point2::new(x, -y),
                ]
            }
            Self::Polygon(polygon) => polygon.points().to_vec(),
        }
    }
}

/// A bore through a component: the region of a shape, shifted off the axis and turned about it,
/// that holds no material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bore {
    shape: OutlineShape,
    center: Point2<Length>,
    rotation: Angle,
}

impl Bore {
    /// Create a new [`Bore`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the bore, around its own centre.
    /// * `center` - where the centre of the bore lies, relative to the axis of the component.
    /// * `rotation` - how far the bore is turned about its centre.
    pub fn new(shape: impl Into<OutlineShape>, center: Point2<Length>, rotation: Angle) -> Self {
        Self {
            shape: shape.into(),
            center,
            rotation,
        }
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
    /// The region of this bore as an [`Aperture`] of the given type, placed where the bore is.
    fn aperture(&self, aperture_type: ApertureType) -> OpmResult<Aperture> {
        Aperture::new(
            self.shape.clone().into(),
            aperture_type,
            Some(self.center),
            Some(self.rotation),
        )
    }
    /// Move a point of the bore's own frame to where the bore is: turned about its centre, then
    /// shifted off the axis.
    fn placed(&self, point: Point2<Length>) -> Point2<Length> {
        let (sin, cos) = self.rotation.value.sin_cos();
        let (x, y) = (point.x.value, point.y.value);
        Point2::new(
            meter!(x.mul_add(cos, (-y).mul_add(sin, self.center.x.value))),
            meter!(x.mul_add(sin, y.mul_add(cos, self.center.y.value))),
        )
    }
    /// Move a point of the plane into the bore's own frame; the inverse of [`Self::placed`].
    fn unplaced(&self, point: Point2<Length>) -> Point2<Length> {
        let (sin, cos) = self.rotation.value.sin_cos();
        let (x, y) = (
            point.x.value - self.center.x.value,
            point.y.value - self.center.y.value,
        );
        Point2::new(
            meter!(x.mul_add(cos, y * sin)),
            meter!(y.mul_add(cos, -x * sin)),
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
    /// Return whether a point of the plane lies on or within this bore.
    fn contains(&self, point: Point2<Length>) -> bool {
        self.shape.contains(self.unplaced(point))
    }
}

/// The edge of a component, centred on its axis, and the bores through it.
///
/// The bores lie within the edge and apart from each other; that is checked whenever an
/// [`Outline`] is made, also when one is read from a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OutlineFields")]
pub struct Outline {
    shape: OutlineShape,
    bores: Vec<Bore>,
}

/// The fields of an [`Outline`] as a file states them, before they are checked.
#[derive(Deserialize)]
struct OutlineFields {
    shape: OutlineShape,
    #[serde(default)]
    bores: Vec<Bore>,
}

impl TryFrom<OutlineFields> for Outline {
    type Error = OpossumError;

    fn try_from(fields: OutlineFields) -> OpmResult<Self> {
        Self::new(fields.shape, fields.bores)
    }
}

impl Outline {
    /// Create a new [`Outline`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the edge of the component, centred on its axis.
    /// * `bores` - the bores through the component.
    ///
    /// # Errors
    ///
    /// This function returns an error if a bore reaches beyond the edge or touches it, or if two
    /// bores overlap or touch. The edges are compared as polygons, a circle sampled at 64 points;
    /// edges closer than 1 nm touch.
    pub fn new(shape: impl Into<OutlineShape>, bores: Vec<Bore>) -> OpmResult<Self> {
        let outline = Self {
            shape: shape.into(),
            bores,
        };
        outline.check_bores()?;
        Ok(outline)
    }
    /// Returns the edge of the component.
    #[must_use]
    pub const fn shape(&self) -> &OutlineShape {
        &self.shape
    }
    /// Returns the bores through the component.
    #[must_use]
    pub fn bores(&self) -> &[Bore] {
        &self.bores
    }
    /// Return whether a point of the plane holds material: it lies on or within the edge and in
    /// none of the bores.
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the plane of the outline with the axis at its origin.
    #[must_use]
    pub(crate) fn contains(&self, point: Point2<Length>) -> bool {
        self.shape.contains(point) && !self.bores.iter().any(|bore| bore.contains(point))
    }
    /// Return the points the component reaches farthest in, in the plane of the outline.
    ///
    /// They are the extreme points of the edge (see [`OutlineShape::extreme_points`]): a bore
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
    pub fn transversal_reach(&self) -> Length {
        self.extreme_points()
            .iter()
            .fold(Length::zero(), |reach, point| {
                Length::max(reach, meter!(point.x.value.hypot(point.y.value)))
            })
    }
    /// Check that every bore lies within the edge and apart from every other bore, none touching.
    ///
    /// Two regions apart from each other have edges that do not meet and neither holds a point of
    /// the other; a bore within the edge has edges that do not meet and a corner inside it.
    fn check_bores(&self) -> OpmResult<()> {
        let edge_ring = self.shape.corners();
        let corners = self.bores.iter().map(Bore::corners).collect::<Vec<_>>();
        for (number, (bore, ring)) in self.bores.iter().zip(&corners).enumerate() {
            if rings_meet(&edge_ring, ring)
                || !ring.first().is_some_and(|&c| self.shape.contains(c))
            {
                return Err(OpossumError::Properties(format!(
                    "bore {} does not lie within the edge of the component without touching it",
                    number + 1
                )));
            }
            for (other_number, (other, other_ring)) in
                self.bores.iter().zip(&corners).enumerate().skip(number + 1)
            {
                if rings_meet(ring, other_ring)
                    || other_ring.first().is_some_and(|&c| bore.contains(c))
                    || ring.first().is_some_and(|&c| other.contains(c))
                {
                    return Err(OpossumError::Properties(format!(
                        "bores {} and {} overlap or touch",
                        number + 1,
                        other_number + 1
                    )));
                }
            }
        }
        Ok(())
    }
}

impl From<OutlineShape> for Outline {
    fn from(shape: OutlineShape) -> Self {
        Self {
            shape,
            bores: Vec::new(),
        }
    }
}

/// The region of an outline as an [`Aperture`], to be plotted: its edge as a hole, and each bore
/// as an obstruction in it.
///
/// # Errors
///
/// This function returns an error if a bore cannot be placed as an [`Aperture`].
impl TryFrom<&Outline> for Aperture {
    type Error = OpossumError;

    fn try_from(outline: &Outline) -> OpmResult<Self> {
        let edge = Self::new(outline.shape.clone().into(), ApertureType::Hole, None, None)?;
        if outline.bores.is_empty() {
            return Ok(edge);
        }
        let bores = outline
            .bores
            .iter()
            .map(|bore| bore.aperture(ApertureType::Obstruction));
        Self::new_stack(
            std::iter::once(Ok(edge))
                .chain(bores)
                .collect::<OpmResult<_>>()?,
            ApertureType::Hole,
            None,
            None,
        )
    }
}

/// Whether the edges of two closed polygons cross or touch each other: come closer than
/// [`TOUCHING`].
fn rings_meet(one: &[Point2<Length>], other: &[Point2<Length>]) -> bool {
    let edges = |ring: &[Point2<Length>]| {
        ring.iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(from, to)| (from.map(|c| c.value), to.map(|c| c.value)))
            .collect::<Vec<_>>()
    };
    let other_edges = edges(other);
    edges(one).iter().any(|&edge| {
        other_edges
            .iter()
            .any(|&other_edge| segments_meet(edge, other_edge))
    })
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
    /// Bounded by an edge on the axis, with bores through it.
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
            Self::Bounded(outline) => match outline.bores.len() {
                0 => write!(f, "{}", outline.shape),
                1 => write!(f, "{} with 1 bore", outline.shape),
                bores => write!(f, "{} with {bores} bores", outline.shape),
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
    use crate::{degree, millimeter};
    use approx::assert_relative_eq;
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
        region.contains(millimeter!(x, y))
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
            vec![bore(circle(2.0)?, 0.0, 0.0, 0.0)],
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
    /// A square plate of 20 mm with a round bore and a turned slot beside its axis holds material
    /// on the plate but not in the bores, so it transmits on 400 mm² less the bores.
    #[test]
    fn a_plate_transmits_beside_its_shifted_and_turned_bores() -> OpmResult<()> {
        let plate = region(&OpticShape::from(Outline::new(
            rectangle(20.0, 20.0)?,
            vec![
                bore(circle(2.0)?, 4.0, 4.0, 0.0),
                bore(rectangle(3.0, 1.0)?, -4.0, -3.0, 30.0),
            ],
        )?))?;
        assert!(transmits(&plate, 0.0, 0.0));
        assert!(transmits(&plate, 9.9, -9.9));
        assert!(!transmits(&plate, 10.1, 0.0));
        assert!(!transmits(&plate, 4.0, 4.0));
        assert!(!transmits(&plate, 5.9, 4.0));
        assert!(!transmits(&plate, -4.0, -3.0));
        // 1.3 mm from the centre of the slot along it, which an unturned slot would not reach, and
        // as far turned the other way.
        let (sin, cos) = 30.0_f64.to_radians().sin_cos();
        let x = 1.3_f64.mul_add(cos, -4.0);
        assert!(!transmits(&plate, x, 1.3_f64.mul_add(sin, -3.0)));
        assert!(transmits(&plate, x, 1.3_f64.mul_add(-sin, -3.0)));
        assert_relative_eq!(
            area(&plate, 11),
            PI.mul_add(-4.0, 400.0) - 3.0,
            max_relative = 2e-3
        );
        Ok(())
    }
    /// A triangular plate holds material within its edge but not in its turned square bore.
    #[test]
    fn a_polygon_holds_material_beside_its_bore() -> OpmResult<()> {
        let plate = region(&OpticShape::from(Outline::new(
            triangle([(-10.0, -10.0), (10.0, -10.0), (0.0, 10.0)])?,
            vec![bore(rectangle(2.0, 2.0)?, 0.0, -4.0, 45.0)],
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
    /// An outline reaches as far as its edge, wherever its bores are: a ring as far as its outer
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
        let ring = Outline::new(circle(12.5)?, vec![bore(circle(2.0)?, 3.0, 0.0, 0.0)])?;
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
    /// A bore lies within the edge, and apart from every other bore; none touches another or the
    /// edge. A bore 1 µm from another or from the edge is apart.
    #[test]
    fn bores_lie_apart_within_the_edge() -> OpmResult<()> {
        let plate = |bores: Vec<Bore>| Outline::new(rectangle(20.0, 20.0)?, bores);
        assert!(
            plate(vec![
                bore(circle(2.0)?, 4.0, 4.0, 0.0),
                bore(circle(2.0)?, -4.0, -4.0, 0.0)
            ])
            .is_ok()
        );
        assert!(plate(vec![bore(rectangle(2.0, 2.0)?, 8.999, 0.0, 0.0)]).is_ok());
        assert!(
            plate(vec![
                bore(rectangle(2.0, 2.0)?, 0.0, 0.0, 0.0),
                bore(rectangle(2.0, 2.0)?, 2.001, 0.0, 0.0),
            ])
            .is_ok()
        );
        for (case, bores) in [
            ("outside the edge", vec![bore(circle(1.0)?, 15.0, 0.0, 0.0)]),
            ("across the edge", vec![bore(circle(2.0)?, 9.5, 0.0, 0.0)]),
            (
                "turned across the edge",
                vec![bore(rectangle(1.0, 6.0)?, 8.0, 0.0, 45.0)],
            ),
            (
                "touching the edge",
                vec![bore(rectangle(2.0, 2.0)?, 9.0, 0.0, 0.0)],
            ),
            ("around the plate", vec![bore(circle(20.0)?, 0.0, 0.0, 0.0)]),
            (
                "overlapping",
                vec![
                    bore(circle(2.0)?, 0.0, 0.0, 0.0),
                    bore(circle(2.0)?, 3.0, 0.0, 0.0),
                ],
            ),
            (
                "touching the edge with a side",
                vec![bore(
                    triangle([(8.0, 0.0), (10.0, 1.0), (10.0, -1.0)])?,
                    0.0,
                    0.0,
                    0.0,
                )],
            ),
            (
                "touching",
                vec![
                    bore(rectangle(2.0, 2.0)?, 0.0, 0.0, 0.0),
                    bore(rectangle(2.0, 2.0)?, 2.0, 0.0, 0.0),
                ],
            ),
            (
                "touching with a corner",
                vec![
                    bore(rectangle(2.0, 2.0)?, 0.0, 0.0, 0.0),
                    bore(
                        triangle([(3.0, 1.0), (1.0, 0.0), (3.0, -1.0)])?,
                        0.0,
                        0.0,
                        0.0,
                    ),
                ],
            ),
            (
                "one in another",
                vec![
                    bore(circle(1.0)?, 0.0, 0.0, 0.0),
                    bore(circle(3.0)?, 0.0, 0.0, 0.0),
                ],
            ),
        ] {
            assert!(plate(bores).is_err(), "a bore {case} is accepted");
        }
        Ok(())
    }
    /// A file cannot state bores the type refuses: they are checked when the outline is read.
    #[test]
    fn an_outline_read_from_a_file_is_checked() -> OpmResult<()> {
        let apart = Outline::new(
            rectangle(20.0, 20.0)?,
            vec![
                bore(circle(2.0)?, 4.0, 4.0, 0.0),
                bore(circle(2.0)?, -4.0, -4.0, 0.0),
            ],
        )?;
        let file = ron::to_string(&apart).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(ron::from_str::<Outline>(&file).ok(), Some(apart));
        assert!(file.contains("-0.004"));
        let overlapping = file.replace("-0.004", "0.004");
        assert!(ron::from_str::<Outline>(&overlapping).is_err());
        Ok(())
    }
    #[test]
    fn display() -> OpmResult<()> {
        let plate = |bores| -> OpmResult<OpticShape> {
            Ok(Outline::new(rectangle(20.0, 20.0)?, bores)?.into())
        };
        let hole_at = |x| -> OpmResult<Bore> { Ok(bore(circle(1.0)?, x, 0.0, 0.0)) };
        assert_eq!(OpticShape::Open.to_string(), "Open");
        assert_eq!(OpticShape::from(circle(5.0)?).to_string(), "Circle");
        assert_eq!(
            plate(vec![hole_at(4.0)?])?.to_string(),
            "Rectangle with 1 bore"
        );
        assert_eq!(
            plate(vec![hole_at(4.0)?, hole_at(-4.0)?])?.to_string(),
            "Rectangle with 2 bores"
        );
        Ok(())
    }
}
