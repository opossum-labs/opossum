use crate::error::{OpmResult, OpossumError};
use earcutr::earcut;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

use super::{ApertureType, Apodize};

/// Configuration of a polygonal aperture defined by a given set of points.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolygonConfig {
    points: Vec<Point2<Length>>,
    aperture_type: ApertureType,
    triangle_indices: Vec<Vec<usize>>,
}
impl PolygonConfig {
    /// Create a new polygonal aperture configuration by a set of given 2D points.
    ///
    /// The order of the points must follow the outline of the polygon. Otherwise intersections may occur.
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the number of points is less than three, so that no polygon can be created.
    pub fn new(points: Vec<Point2<Length>>) -> OpmResult<Self> {
        if points.len() < 3 {
            return Err(OpossumError::Other("less than 3 points given".into()));
        }
        Ok(Self {
            triangle_indices: Self::triangulate(&points)?,
            points,
            aperture_type: ApertureType::default(),
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
    pub fn in_polygon(&self, point: &Point2<Length>) -> bool {
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
        &self.points
    }
}
impl Apodize for PolygonConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let mut transmission = if self.in_polygon(point) { 1.0 } else { 0.0 };
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    #[test]
    fn new() {
        let ok_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0), meter!(1.0, 1.0)];
        assert!(PolygonConfig::new(ok_points).is_ok());
        let too_little_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0)];
        assert!(PolygonConfig::new(too_little_points).is_err());
    }
}