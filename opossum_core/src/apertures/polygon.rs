use super::{Bounded, Shape, resample_ring};
use crate::{
    error::{OpmResult, OpossumError},
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
///
/// Only the points are written to a file; the triangles are worked out from them whenever they
/// change, also when a polygon is read.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated, ToSchema)]
#[serde(try_from = "PolygonFields")]
pub struct PolygonShape {
    #[schema(value_type = Object)]
    points: ValidatedPolygonPoints2D,
    #[validate(skip)]
    #[serde(skip)]
    triangle_indices: Vec<Vec<usize>>,
}

/// The fields of a [`PolygonShape`] as a file states them.
#[derive(Deserialize)]
struct PolygonFields {
    points: ValidatedPolygonPoints2D,
}

impl TryFrom<PolygonFields> for PolygonShape {
    type Error = OpossumError;

    fn try_from(fields: PolygonFields) -> OpmResult<Self> {
        Self::new(fields.points.get().clone())
    }
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

    /// checks, if a point lies within this [`PolygonShape`]
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
    /// Returns a reference to the points of this [`PolygonShape`].
    #[must_use]
    pub fn points(&self) -> &[Point2<Length>] {
        self.points.get()
    }

    /// Add new points to this [`PolygonShape`]. The new points are added to the end of the existing points.
    /// The order of the points must follow the outline of the polygon. Otherwise intersections may occur.
    /// # Errors
    /// This function will return an error if the new points are invalid, e.g. if they are indefinite.
    /// The polygon is left as it was then.
    pub fn add_points(&mut self, new_points: &[Point2<Length>]) -> OpmResult<()> {
        *self = Self::new([self.points(), new_points].concat())?;
        Ok(())
    }

    /// Delete a point at a given index from this [`PolygonShape`].
    /// # Errors
    /// This function will return an error if the index is out of bounds or fewer than three
    /// points would remain. The polygon is left as it was then.
    pub fn delete_point(&mut self, index: usize) -> OpmResult<()> {
        let mut points = self.points.clone();
        points.remove(index)?;
        *self = Self::new(points.get().clone())?;
        Ok(())
    }
}
impl Bounded for PolygonShape {
    /// Return the edge of this polygon as a counter-clockwise ring of about `segments` points.
    ///
    /// Every corner is part of it, and a polygon given clockwise is turned around; see
    /// [`OutlineShape::outline_points`](crate::apertures::OutlineShape::outline_points).
    ///
    /// # Errors
    ///
    /// This function returns an error if the corners enclose no area, i.e. all lie on one line.
    fn outline_points(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        resample_ring(self.points(), segments)
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
    /// A regular pentagon of 5 mm circumradius, counter-clockwise from its top corner.
    fn pentagon() -> Vec<Point2<Length>> {
        [
            (0.0, 5.0),
            (-4.755_283, 1.545_085),
            (-2.938_926, -4.045_085),
            (2.938_926, -4.045_085),
            (4.755_283, 1.545_085),
        ]
        .map(|(x, y)| millimeter!(x, y))
        .to_vec()
    }
    /// A polygon edited point by point, the way the node editor builds one from the default
    /// square, transmits after every edit where a polygon given the same points at once does.
    #[test]
    fn an_edited_polygon_transmits_where_its_points_enclose() -> OpmResult<()> {
        let transmits_alike = |edited: &PolygonShape| -> OpmResult<()> {
            let given = PolygonShape::new(edited.points().to_vec())?;
            for x in (-28..=28).map(|step| f64::from(step) / 2.0) {
                for y in (-28..=28).map(|step| f64::from(step) / 2.0) {
                    let point = millimeter!(x, y, 0.0);
                    assert_eq!(
                        edited.transmission_factor(&point),
                        given.transmission_factor(&point),
                        "at ({x}, {y}) mm within {:?}",
                        edited.points()
                    );
                }
            }
            Ok(())
        };
        let mut edited = PolygonShape::default();
        edited.delete_point(0)?;
        transmits_alike(&edited)?;
        for corner in pentagon() {
            edited.add_points(&[corner])?;
            transmits_alike(&edited)?;
        }
        for _ in 0..3 {
            edited.delete_point(0)?;
            transmits_alike(&edited)?;
        }
        assert_eq!(edited.points(), pentagon().as_slice());
        Ok(())
    }
    /// A file states the points of a polygon only; its triangles are worked out anew when it is
    /// read, whatever an older file wrote for them.
    #[test]
    fn a_polygon_read_from_a_file_is_triangulated_anew() -> OpmResult<()> {
        let pentagon = PolygonShape::new(pentagon())?;
        let file = ron::to_string(&pentagon).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!file.contains("triangle_indices"), "{file}");
        let stale = format!(
            "{},triangle_indices:[[2,3,0],[0,1,7]])",
            file.strip_suffix(')').unwrap()
        );
        let read = ron::from_str::<PolygonShape>(&stale)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(read, pentagon);
        // Between the corners P7, P9 and P1, which the stale triangles leave out.
        assert_eq!(read.transmission_factor(&millimeter!(3.5, 0.0, 0.0)), 1.0);
        Ok(())
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
        assert!(ps.add_points(&[millimeter!(f64::NAN, 0.0)]).is_err());
        assert!(ps.add_points(&[millimeter!(f64::INFINITY, 0.0)]).is_err());
        assert!(
            ps.add_points(&[millimeter!(f64::NEG_INFINITY, 0.0)])
                .is_err()
        );
        assert!(ps.add_points(&[millimeter!(0.0, 0.0)]).is_ok());
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
