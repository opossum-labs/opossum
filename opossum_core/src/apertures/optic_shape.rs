#![warn(missing_docs)]
//! The value of a component's optic shape: an outline on its axis with bores through it.
//!
//! A component has an edge and, possibly, bores through it, such as a mirror with a hole for the
//! beam to pass. That is all the optic shape states: unlike a general [`Aperture`], it has no
//! soft edges, no obstructions and no stack of arbitrary shapes. The geometry works with it as the
//! [`Aperture`] it amounts to (see [`OpticShape::cross_section`]).

use super::{Aperture, ApertureShape, ApertureType, CircleShape, PolygonShape, RectangleShape};
use crate::{
    error::{OpmResult, OpossumError},
    properties::Proptype,
    types::validated_type_definitions::ValidatedCrossSection,
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
impl TryFrom<&ApertureShape> for OutlineShape {
    type Error = OpossumError;

    fn try_from(shape: &ApertureShape) -> OpmResult<Self> {
        match shape {
            ApertureShape::BinaryCircle(shape) => Ok(Self::Circle(*shape)),
            ApertureShape::BinaryRectangle(shape) => Ok(Self::Rectangle(*shape)),
            ApertureShape::BinaryPolygon(shape) => Ok(Self::Polygon(shape.clone())),
            shape => Err(OpossumError::Properties(format!(
                "a shape '{shape}' has no edge to outline a component or a bore with"
            ))),
        }
    }
}

impl OutlineShape {
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
    /// Read a bore that an older file stated as an [`Aperture`]: its shape, shifted and turned the
    /// way the aperture is.
    ///
    /// # Arguments
    ///
    /// * `aperture` - the aperture; its type is not looked at.
    ///
    /// # Errors
    ///
    /// This function returns an error if the shape of the aperture has no edge.
    pub(crate) fn from_aperture(aperture: &Aperture) -> OpmResult<Self> {
        let (center, rotation) = aperture.isometry().map_or_else(
            || (Point2::origin(), Angle::zero()),
            |isometry| {
                let shift = isometry.translation();
                (Point2::new(shift.x, shift.y), isometry.rotation().z)
            },
        );
        Ok(Self::new(
            OutlineShape::try_from(aperture.shape())?,
            center,
            rotation,
        ))
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
    /// The corners of the edge of this bore, placed where the bore is.
    fn corners(&self) -> OpmResult<Vec<Point2<Length>>> {
        let placed = self.aperture(ApertureType::Hole)?;
        Ok(self
            .shape
            .corners()
            .into_iter()
            .map(|corner| {
                placed.isometry().map_or(corner, |isometry| {
                    let moved =
                        isometry.transform_point(&Point3::new(corner.x, corner.y, Length::zero()));
                    Point2::new(moved.x, moved.y)
                })
            })
            .collect())
    }
    /// Whether a point lies within this bore.
    fn holds(&self, point: Point2<Length>) -> OpmResult<bool> {
        Ok(self.aperture(ApertureType::Hole)?.apodize(&Point3::new(
            point.x,
            point.y,
            Length::zero(),
        )) > 0.0)
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
    /// The edge of the component, placed on its axis the way a bore is placed off it.
    fn edge(&self) -> Bore {
        Bore::new(self.shape.clone(), Point2::origin(), Angle::zero())
    }
    /// The region of the component as an [`Aperture`]: its edge as a hole, and each bore as an
    /// obstruction in it.
    fn aperture(&self) -> OpmResult<Aperture> {
        let edge = self.edge().aperture(ApertureType::Hole)?;
        if self.bores.is_empty() {
            return Ok(edge);
        }
        let bores = self
            .bores
            .iter()
            .map(|bore| bore.aperture(ApertureType::Obstruction));
        Aperture::new_stack(
            std::iter::once(Ok(edge))
                .chain(bores)
                .collect::<OpmResult<_>>()?,
            ApertureType::Hole,
            None,
            None,
        )
    }
    /// Check that every bore lies within the edge and apart from every other bore, none touching.
    ///
    /// Two regions apart from each other have edges that do not meet and neither holds a point of
    /// the other; a bore within the edge has edges that do not meet and a corner inside it.
    fn check_bores(&self) -> OpmResult<()> {
        let edge = self.edge();
        let edge_ring = edge.corners()?;
        let corners = self
            .bores
            .iter()
            .map(Bore::corners)
            .collect::<OpmResult<Vec<_>>>()?;
        for (number, (bore, ring)) in self.bores.iter().zip(&corners).enumerate() {
            if rings_meet(&edge_ring, ring) || !edge.holds(ring[0])? {
                return Err(OpossumError::Properties(format!(
                    "bore {} does not lie within the edge of the component without touching it",
                    number + 1
                )));
            }
            for (other_number, (other, other_ring)) in
                self.bores.iter().zip(&corners).enumerate().skip(number + 1)
            {
                if rings_meet(ring, other_ring)
                    || bore.holds(other_ring[0])?
                    || other.holds(ring[0])?
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
    /// Return the region of this optic shape as the cross section the geometry works with.
    ///
    /// # Returns
    ///
    /// The cross section, or `None` if the optic shape is open.
    ///
    /// # Errors
    ///
    /// This function returns an error if a shape cannot be placed as an [`Aperture`].
    pub fn cross_section(&self) -> OpmResult<Option<ValidatedCrossSection>> {
        match self {
            Self::Open => Ok(None),
            Self::Bounded(outline) => ValidatedCrossSection::try_new(outline.aperture()?).map(Some),
        }
    }
    /// Read the clear aperture an older file stated as an [`ApertureShape`] as an optic shape.
    ///
    /// A shape with an edge becomes the edge of the component. A stack becomes an edge with bores
    /// if it holds exactly one hole, unshifted and unturned, and obstructions with an edge.
    ///
    /// # Arguments
    ///
    /// * `shape` - the clear aperture as an older file stated it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the shape cannot be stated as an edge with bores: a soft
    /// shape, or a stack that is not one centred hole with obstructions in it.
    pub fn from_aperture_shape(shape: &ApertureShape) -> OpmResult<Self> {
        let ApertureShape::Stack(stack) = shape else {
            if matches!(shape, ApertureShape::Open) {
                return Ok(Self::Open);
            }
            return Ok(Self::Bounded(OutlineShape::try_from(shape)?.into()));
        };
        let (holes, obstructions): (Vec<_>, Vec<_>) = stack
            .apertures()
            .iter()
            .partition(|member| *member.aperture_type() == ApertureType::Hole);
        let [edge] = holes.as_slice() else {
            return Err(OpossumError::Properties(format!(
                "a stack of {} holes cannot be stated as the edge of a component",
                holes.len()
            )));
        };
        if edge.isometry().is_some() {
            return Err(OpossumError::Properties(
                "the hole of the stack is shifted or turned, but the edge of a component is \
                 centred on its axis"
                    .into(),
            ));
        }
        let bores = obstructions
            .iter()
            .map(|obstruction| Bore::from_aperture(obstruction))
            .collect::<OpmResult<_>>()?;
        Ok(Self::Bounded(Outline::new(
            OutlineShape::try_from(edge.shape())?,
            bores,
        )?))
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
    use crate::{apertures::StackShape, degree, millimeter};
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
    /// The cross section of a bounded optic shape.
    fn region(optic_shape: &OpticShape) -> OpmResult<Aperture> {
        let cross_section = optic_shape
            .cross_section()?
            .ok_or_else(|| OpossumError::Other("the optic shape is open".into()))?;
        Ok(cross_section.get().clone())
    }
    /// Whether the region transmits at (x, y) mm.
    fn transmits(region: &Aperture, x: f64, y: f64) -> bool {
        region.apodize(&millimeter!(x, y, 0.0)) > 0.0
    }
    /// The area in mm² the region transmits on, counted on a grid of 0.05 mm over the square of
    /// the given half width in mm.
    fn area(region: &Aperture, half_width: u32) -> f64 {
        let step = 0.05;
        let steps = 40 * half_width;
        let at = |index: u32| (f64::from(index) + 0.5).mul_add(step, -f64::from(half_width));
        let transmitting = (0..steps)
            .flat_map(|i| (0..steps).map(move |j| (i, j)))
            .filter(|&(i, j)| transmits(region, at(i), at(j)))
            .count();
        to_f64(transmitting) * step * step
    }

    /// A ring transmits where the stack of a hole and a central obstruction did, which older files
    /// stated it as, on an area of π (R² - r²).
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
        for x in (-130..=130).map(|tenth| f64::from(tenth) / 10.0) {
            for y in [0.0, 1.0, 1.9, 2.1, 7.0, 12.4] {
                assert_eq!(
                    transmits(&ring, x, y),
                    transmits(&stack, x, y),
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
    /// An optic shape as an older file stated it, as an aperture shape: a shape with an edge is
    /// the edge, a stack of one centred hole and obstructions an edge with bores shifted and turned
    /// as the obstructions were. A hole shifted off the axis, or a second hole, has no such form.
    #[test]
    fn an_old_aperture_shape_reads_as_an_edge_with_bores() -> OpmResult<()> {
        let plate = || {
            Aperture::new_rectangle(
                millimeter!(20.0),
                millimeter!(20.0),
                ApertureType::Hole,
                None,
                None,
            )
        };
        let slot = Aperture::new_rectangle(
            millimeter!(3.0),
            millimeter!(1.0),
            ApertureType::Obstruction,
            Some(millimeter!(-4.0, -3.0)),
            Some(degree!(30.0)),
        )?;
        let stack = |members| -> OpmResult<ApertureShape> {
            Ok(ApertureShape::Stack(StackShape::new(members)?))
        };
        let OpticShape::Bounded(outline) =
            OpticShape::from_aperture_shape(&stack(vec![plate()?, slot.clone()])?)?
        else {
            panic!("a plate with a slot is bounded");
        };
        assert_eq!(outline.shape(), &OutlineShape::from(rectangle(20.0, 20.0)?));
        let [read] = outline.bores() else {
            panic!("the plate has one bore");
        };
        assert_eq!(read.shape(), &OutlineShape::from(rectangle(3.0, 1.0)?));
        assert_relative_eq!(read.center().x.value, -0.004, max_relative = 1e-12);
        assert_relative_eq!(read.center().y.value, -0.003, max_relative = 1e-12);
        assert_relative_eq!(
            read.rotation().value,
            30.0_f64.to_radians(),
            max_relative = 1e-12
        );
        assert_eq!(
            OpticShape::from_aperture_shape(&ApertureShape::BinaryCircle(circle(5.0)?))?,
            circle(5.0)?.into()
        );
        assert_eq!(
            OpticShape::from_aperture_shape(&ApertureShape::Open)?,
            OpticShape::Open
        );
        let shifted = Aperture::new_circle(
            millimeter!(5.0),
            ApertureType::Hole,
            Some(millimeter!(1.0, 0.0)),
        )?;
        for members in [vec![shifted, slot.clone()], vec![plate()?, plate()?, slot]] {
            assert!(
                OpticShape::from_aperture_shape(&stack(members.clone())?).is_err(),
                "{members:?}"
            );
        }
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
