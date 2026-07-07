use super::Shape;
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::ValidateTrait,
    millimeter,
    prelude::ApertureShape,
    types::validated_type_definitions::ValidatedPolygonPoints2D,
};
use earcutr::earcut;
use nalgebra::{Point2, Point3};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
use utoipa::ToSchema;

/// Configuration of a polygonal aperture defined by a given set of points.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated, ToSchema)]
pub struct PolygonShape {
    #[schema(value_type = Object)]
    points: ValidatedPolygonPoints2D,
    #[validate(skip)]
    triangle_indices: Vec<Vec<usize>>,
}
impl From<PolygonShape> for ApertureShape {
    fn from(value: PolygonShape) -> Self {
        Self::BinaryPolygon(value)
    }
}
impl Default for PolygonShape {
    fn default() -> Self {
        let points = vec![
            millimeter!(-12.5, -12.5),
            millimeter!(12.5, -12.5),
            millimeter!(12.5, 12.5),
            millimeter!(-12.5, 12.5),
        ];
        Self::new(points).unwrap()
    }
}

impl PolygonShape {
    /// Create a new polygonal aperture configuration by a set of given 2D points.
    ///
    /// The order of the points must follow the outline of the polygon. Otherwise intersections may occur.
    ///
    /// # Errors
    ///
    /// This function will return an error if the number of points is less than three, so that no polygon can be created.
    pub fn new(points: Vec<Point2<Length>>) -> OpmResult<Self> {
        let validated_points = ValidatedPolygonPoints2D::try_new(points)?;
        Ok(Self {
            triangle_indices: Self::triangulate(validated_points.get())?,
            points: validated_points,
        })
    }

    fn triangulate(points: &[Point2<Length>]) -> OpmResult<Vec<Vec<usize>>> {
        let polygon_vertices_flat = points
            .iter()
            .flat_map(|p| vec![p.x.get::<meter>(), p.y.get::<meter>()])
            .collect::<Vec<f64>>();

        let triangulated_indices = earcut(polygon_vertices_flat.as_slice(), &[], 2)
            .map_err(|e| OpossumError::Other(format!("Triangulation of polygon failed:{e}")))?;
        let mut chunked_indices = Vec::<Vec<usize>>::with_capacity(triangulated_indices.len() / 3);
        for chunk in triangulated_indices.chunks(3) {
            chunked_indices.push(Vec::<usize>::from(chunk));
        }
        Ok(chunked_indices)
    }

    /// checks, if a point lies within this [`PolygonConfig`]
    /// # Panics
    /// This function panics if the triangulation fails
    #[must_use]
    pub fn in_polygon(&self, point: &Point3<Length>) -> bool {
        let mut in_polygon = false;
        for tri in &self.triangle_indices {
            let p1 = self.points[tri[0]];
            let p2 = self.points[tri[1]];
            let p3 = self.points[tri[2]];

            let denominator =
                (p2[1] - p3[1]).mul_add(p1[0] - p3[0], (p3[0] - p2[0]) * (p1[1] - p3[1]));
            let a = (((p2[1] - p3[1])
                .mul_add(point.x - p3[0], (p3[0] - p2[0]) * (point.y - p3[1])))
                / denominator)
                .value;
            let b = (((p3[1] - p1[1])
                .mul_add(point.x - p3[0], (p1[0] - p3[0]) * (point.y - p3[1])))
                / denominator)
                .value;
            let c = 1. - a - b;

            if (0. ..=1.).contains(&a) && (0. ..=1.).contains(&b) && (0. ..=1.).contains(&c) {
                in_polygon = true;
                break;
            }
        }
        in_polygon
    }
    /// Returns a reference to the points of this [`PolygonConfig`].
    #[must_use]
    pub fn points(&self) -> &[Point2<Length>] {
        self.points.get()
    }

    /// Add new points to this [`PolygonConfig`]. The new points are added to the end of the existing points.
    /// The order of the points must follow the outline of the polygon. Otherwise intersections may occur.
    /// # Errors
    /// This function will return an error if the new points are invalid, e.g. if they are indefinite.
    pub fn add_points(&mut self, new_points: &[Point2<Length>]) -> OpmResult<()> {
        for p in new_points {
            self.points.push(*p)?;
        }
        Ok(())
    }

    /// Delete a point at a given index from this [`PolygonConfig`].
    /// # Errors
    /// This function will return an error if the index is out of bounds.
    pub fn delete_point(&mut self, index: usize) -> OpmResult<()> {
        self.points.remove(index)?;
        Ok(())
    }
}
impl Shape for PolygonShape {
    fn transmission_factor(&self, point: &Point3<Length>) -> f64 {
        if self.in_polygon(point) { 1.0 } else { 0.0 }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    #[test]
    fn new() {
        let ok_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0), meter!(1.0, 1.0)];
        assert!(PolygonShape::new(ok_points).is_ok());
        let too_little_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0)];
        assert!(PolygonShape::new(too_little_points).is_err());
    }
    #[test]
    fn getters() -> OpmResult<()> {
        let points = vec![
            meter!(0.0, 0.0),
            meter!(1.0, 0.5),
            meter!(2.0, 0.0),
            meter!(1.0, 1.0),
        ];
        let poly = PolygonShape::new(points.clone())?;
        assert_eq!(poly.points(), points.as_slice());
        Ok(())
    }
    #[test]
    fn transmission_factor() -> OpmResult<()> {
        let poly = PolygonShape::new(vec![
            meter!(0.0, 0.0),
            meter!(1.0, 0.5),
            meter!(2.0, 0.0),
            meter!(1.0, 1.0),
        ])?;
        assert_eq!(poly.transmission_factor(&meter!(0.0, 0.0, 0.0)), 1.0);
        assert_eq!(poly.transmission_factor(&meter!(2.0, 0.0, 0.0)), 1.0);
        assert_eq!(poly.transmission_factor(&meter!(1.0, 1.0, 0.0)), 1.0);
        assert_eq!(poly.transmission_factor(&meter!(1.0, 0.0, 0.0)), 0.0);
        assert_eq!(poly.transmission_factor(&meter!(2.0, 1.0, 0.0)), 0.0);
        assert_eq!(poly.transmission_factor(&meter!(0.0, 1.0, 0.0)), 0.0);
        Ok(())
    }
    #[test]
    fn test_non_convex_u_shape() -> OpmResult<()> {
        // A U-shaped polygon
        let points = vec![
            meter!(0.0, 0.0),
            meter!(3.0, 0.0),
            meter!(3.0, 3.0),
            meter!(2.0, 3.0),
            meter!(2.0, 1.0),
            meter!(1.0, 1.0),
            meter!(1.0, 3.0),
            meter!(0.0, 3.0),
        ];
        let poly = PolygonShape::new(points)?;

        // Inside the "arms"
        assert_eq!(poly.transmission_factor(&meter!(0.5, 2.0, 0.0)), 1.0);
        assert_eq!(poly.transmission_factor(&meter!(2.5, 2.0, 0.0)), 1.0);

        // In the "gap" of the U (should be outside/0.0)
        assert_eq!(poly.transmission_factor(&meter!(1.5, 2.0, 0.0)), 0.0);

        // Bottom bar
        assert_eq!(poly.transmission_factor(&meter!(1.5, 0.5, 0.0)), 1.0);
        Ok(())
    }
    #[test]
    fn from() {
        let ps = PolygonShape::default();
        let aps: ApertureShape = ps.into();
        assert!(matches!(aps, ApertureShape::BinaryPolygon(_)))
    }
    #[test]
    fn default() {
        let ps = PolygonShape::default();
        let expected = vec![
            millimeter!(-12.5, -12.5),
            millimeter!(12.5, -12.5),
            millimeter!(12.5, 12.5),
            millimeter!(-12.5, 12.5),
        ];
        for (point, expected) in ps.points().iter().zip(expected) {
            assert_eq!(point, &expected);
        }
    }
    #[test]
    fn remove_add_point() {
        let mut ps = PolygonShape::default();
        assert!(ps.delete_point(4).is_err());
        assert!(ps.delete_point(1).is_ok());
        let expected = vec![
            millimeter!(-12.5, -12.5),
            millimeter!(12.5, 12.5),
            millimeter!(-12.5, 12.5),
        ];
        for (point, expected) in ps.points().iter().zip(expected) {
            assert_eq!(point, &expected);
        }
        assert!(ps.add_points(&vec![millimeter!(f64::NAN, 0.0)]).is_err());
        assert!(
            ps.add_points(&vec![millimeter!(f64::INFINITY, 0.0)])
                .is_err()
        );
        assert!(
            ps.add_points(&vec![millimeter!(f64::NEG_INFINITY, 0.0)])
                .is_err()
        );
        assert!(ps.add_points(&vec![millimeter!(0.0, 0.0)]).is_ok());
        let expected = vec![
            millimeter!(-12.5, -12.5),
            millimeter!(12.5, 12.5),
            millimeter!(-12.5, 12.5),
            millimeter!(0.0, 0.0),
        ];
        for (point, expected) in ps.points().iter().zip(expected) {
            assert_eq!(point, &expected);
        }
    }
}
