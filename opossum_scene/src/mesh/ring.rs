//! Walking the edge of an outline as a ring of points.

use nalgebra::Point2;
use opossum_core::{
    apertures::{Bore, CircleShape, OutlineShape},
    error::{OpmResult, OpossumError},
    meter,
    utils::{
        math_distribution_functions::ellipse,
        math_utils::{distance_2d_point, to_f64, try_f64_to_usize},
    },
};
use uom::si::{f64::Length, length::meter};

/// An edge that can be walked as a closed ring of points.
pub trait RingPoints {
    /// Return the edge as a closed polygon, in its own frame.
    ///
    /// Where a shape answers whether a single point lies inside, this states where the inside
    /// *ends*: the edge, walked counter-clockwise. The first point is not repeated at the end — the
    /// ring closes from the last point back to the first.
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
    fn ring_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>>;
}

impl RingPoints for OutlineShape {
    fn ring_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        if segments < 3 {
            return Err(OpossumError::Other(format!(
                "an outline of {segments} points encloses nothing, at least 3 are needed"
            )));
        }
        match self {
            Self::Circle(circle) => circle.ring_points(segments),
            Self::Rectangle(rectangle) => resample_ring(&rectangle.corners(), segments),
            Self::Polygon(polygon) => resample_ring(polygon.points(), segments),
        }
    }
}

/// `segments` points evenly spaced on the circle. A circle of radius zero is a single point and has
/// no outline.
impl RingPoints for CircleShape {
    fn ring_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        let radius = self.radius().get::<meter>();
        if radius == 0.0 {
            return Err(OpossumError::Other(
                "a circle of radius zero has no outline".into(),
            ));
        }
        let num_points = u32::try_from(segments).map_err(|_| {
            OpossumError::Other(format!(
                "a circle cannot be sampled with as many as {segments} outline points"
            ))
        })?;
        Ok(ellipse((0.0, 0.0), (radius, radius), num_points)?
            .into_iter()
            .map(|point| Point2::new(meter!(point.x), meter!(point.y)))
            .collect())
    }
}

/// The edge of a bore, placed where the bore crosses the plane of the outline (see
/// [`Bore::placed`]).
impl RingPoints for Bore {
    fn ring_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        Ok(self
            .shape()
            .ring_points(segments)?
            .into_iter()
            .map(|point| self.placed(point))
            .collect())
    }
}

/// The length of the edge leaving the ring's `from`th point, in meter.
fn ring_edge_length(ring: &[Point2<Length>], from: usize) -> f64 {
    distance_2d_point(&ring[(from + 1) % ring.len()], &ring[from]).value
}

/// The length of the closed ring through the given points, in meter.
pub fn ring_perimeter(ring: &[Point2<Length>]) -> f64 {
    (0..ring.len())
        .map(|from| ring_edge_length(ring, from))
        .sum()
}

/// Twice the area a ring of points encloses, in square meters, signed by its orientation.
///
/// Positive for a counter-clockwise ring, negative for a clockwise one, and zero for one whose
/// points all lie on a line — which is the only reason this is doubled rather than halved: the
/// factor carries no meaning here, only the sign does.
pub fn doubled_signed_area(ring: &[Point2<Length>]) -> f64 {
    (0..ring.len())
        .map(|from| {
            let to = (from + 1) % ring.len();
            ring[from]
                .x
                .value
                .mul_add(ring[to].y.value, -(ring[to].x.value * ring[from].y.value))
        })
        .sum()
}

/// Spread points along a closed ring of corners and return it counter-clockwise.
///
/// Every corner is kept and further points are inserted along the edges, roughly `segments` in
/// total, handed out by edge length so that a long edge is not sampled more coarsely than a short
/// one. This is what lets a rectangle and a polygon share one implementation: each only has to name
/// its corners.
///
/// # Arguments
///
/// - `corners`: the corners of the ring, in either orientation, without repeating the first one
/// - `segments`: the number of points to aim for
///
/// # Returns
///
/// The resampled ring, counter-clockwise, without a repeated first point.
///
/// # Errors
///
/// This function returns an error if fewer than three corners are given, or if the corners enclose
/// no area — a ring whose corners all lie on one line has no inside, and no orientation either.
fn resample_ring(corners: &[Point2<Length>], segments: usize) -> OpmResult<Vec<Point2<Length>>> {
    if corners.len() < 3 {
        return Err(OpossumError::Other(format!(
            "a ring of {} corners encloses nothing",
            corners.len()
        )));
    }
    let area = doubled_signed_area(corners);
    if area == 0.0 {
        return Err(OpossumError::Other(
            "the corners of the ring enclose no area, so there is no outline to walk".into(),
        ));
    }
    // A negative area means the corners run clockwise. Turning the ring around here rather than in
    // each shape keeps the counter-clockwise promise in one place.
    let ring: Vec<Point2<Length>> = if area.is_sign_negative() {
        corners.iter().rev().copied().collect()
    } else {
        corners.to_vec()
    };
    let perimeter = ring_perimeter(&ring);
    let mut outline = Vec::with_capacity(segments.max(ring.len()));
    for from in 0..ring.len() {
        let to = (from + 1) % ring.len();
        // At least one interval per edge, so every corner survives even when fewer segments than
        // corners were asked for.
        let intervals = try_f64_to_usize(
            (to_f64(segments) * ring_edge_length(&ring, from) / perimeter).round(),
        )
        .unwrap_or(1)
        .max(1);
        outline.push(ring[from]);
        let step = ring[to] - ring[from];
        for point_nr in 1..intervals {
            let along = to_f64(point_nr) / to_f64(intervals);
            outline.push(Point2::new(
                ring[from].x + step.x * along,
                ring[from].y + step.y * along,
            ));
        }
    }
    Ok(outline)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::l_shape_corners;
    use approx::assert_abs_diff_eq;
    use opossum_core::{
        apertures::{PolygonShape, RectangleShape},
        degree, millimeter,
    };

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
    fn ring_points_of_a_circle_lie_on_it() -> OpmResult<()> {
        let radius = millimeter!(12.5);
        let outline = OutlineShape::from(CircleShape::new(radius)?).ring_points(64)?;
        assert_eq!(outline.len(), 64);
        for point in &outline {
            assert_abs_diff_eq!(
                point.x.value.hypot(point.y.value),
                radius.value,
                epsilon = 1e-12
            );
        }
        assert!(
            doubled_signed_area(&outline) > 0.0,
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
    fn ring_points_of_a_rectangle_keep_its_corners() -> OpmResult<()> {
        let (width, height) = (millimeter!(30.0), millimeter!(10.0));
        let outline = OutlineShape::from(RectangleShape::new(width, height)?).ring_points(40)?;
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
        assert!(doubled_signed_area(&outline) > 0.0);
        // The perimeter splits into 15 + 5 + 15 + 5 intervals, so the points asked for are handed
        // out exactly: a long side is sampled as densely as a short one, not as coarsely.
        assert_eq!(outline.len(), 40);
        Ok(())
    }
    #[test]
    fn ring_points_of_a_polygon_keep_its_corners() -> OpmResult<()> {
        // An L, so that a shape which is not convex is covered as well.
        let corners = l_shape_corners();
        let outline = OutlineShape::from(PolygonShape::new(corners.clone())?).ring_points(30)?;
        for corner in &corners {
            assert!(
                outline.contains(corner),
                "corner {corner:?} is missing from the outline"
            );
        }
        for point in &outline {
            assert_abs_diff_eq!(distance_to_ring(point, &corners), 0.0, epsilon = 1e-12);
        }
        assert!(doubled_signed_area(&outline) > 0.0);
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
            let outline = OutlineShape::from(PolygonShape::new(corners)?).ring_points(8)?;
            assert!(doubled_signed_area(&outline) > 0.0);
        }
        Ok(())
    }
    #[test]
    fn a_shifted_bore_moves_its_outline() -> OpmResult<()> {
        let (shift, radius) = (millimeter!(5.0, -3.0), millimeter!(12.5));
        let outline = Bore::new(CircleShape::new(radius)?, shift, degree!(0.0)).ring_points(32)?;
        assert_eq!(outline.len(), 32);
        for point in &outline {
            assert_abs_diff_eq!(
                (point.x - shift.x).value.hypot((point.y - shift.y).value),
                radius.value,
                epsilon = 1e-12
            );
        }
        assert!(doubled_signed_area(&outline) > 0.0);
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
        .ring_points(40)?;
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
        assert!(doubled_signed_area(&outline) > 0.0);
        Ok(())
    }
    #[test]
    fn an_outline_needs_at_least_three_points() -> OpmResult<()> {
        let shape = OutlineShape::from(CircleShape::new(millimeter!(1.0))?);
        assert!(shape.ring_points(0).is_err());
        assert!(shape.ring_points(2).is_err());
        assert!(shape.ring_points(3).is_ok());
        Ok(())
    }
    #[test]
    fn a_ring_without_area_has_no_outline() -> OpmResult<()> {
        let collinear = [
            millimeter!(-1.0, 0.0),
            millimeter!(0.0, 0.0),
            millimeter!(1.0, 0.0),
        ];
        assert!(resample_ring(&collinear, 16).is_err());
        assert!(resample_ring(&collinear[0..2], 16).is_err());
        let point_circle = OutlineShape::from(CircleShape::new(millimeter!(0.0))?);
        assert!(point_circle.ring_points(16).is_err());
        Ok(())
    }
}
