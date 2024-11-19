//! Module for gridding data

#![warn(missing_docs)]
use super::filter_data::filter_nan_infinite;
use crate::{
    error::{OpmResult, OpossumError},
    plottable::AxLims,
};
use approx::abs_diff_ne;
use itertools::Itertools;
use kahan::KahanSum;
use log::warn;
use nalgebra::{DMatrix, DVector, DVectorView, MatrixXx2, MatrixXx3, Point2, Scalar};
use num::{Float, NumCast, ToPrimitive};
use spade::{DelaunayTriangulation, HasPosition, Point2 as SpadeP, Triangulation};
use std::ops::Add;
// use triangulate::Mappable;
use voronator::{
    delaunator::{Coord, Point as VPoint},
    polygon, VoronoiDiagram,
};

struct PointWithHeight {
    position: SpadeP<f64>,
    height: f64,
    // normal: Vector3<f64>
}

impl HasPosition for PointWithHeight {
    type Scalar = f64;

    fn position(&self) -> SpadeP<f64> {
        self.position
    }
}

/// Storage struct for voronoi diagram cells and associated values of its vertices
#[derive(Clone, Debug)]
pub struct VoronoiedData {
    voronoi_diagram: VoronoiDiagram<VPoint>,
    z_data: Option<DVector<f64>>,
}

impl VoronoiedData {
    /// Creates a new [`VoronoiedData`] struct by voronating thhe coordinates and combining them with data, if provided
    /// # Attributes
    /// `xy_coordinates`: coordinates of the data points
    /// `data`: data to combine with voronoi cells
    /// # Errors
    /// This function errors if the number of voronoi cells and data points are not equal or if the creation of voronoi cells fails
    pub fn new(
        xy_coordinates: &MatrixXx2<f64>,
        z_data_opt: Option<DVector<f64>>,
    ) -> OpmResult<Self> {
        let (voronoi_diagram, _) = create_voronoi_cells(xy_coordinates)?;

        let z_data = if let Some(z_data) = z_data_opt {
            if xy_coordinates.shape().0 != z_data.len() {
                return Err(OpossumError::Other("Number of point coordinates and data value is not the same! Cannot assign values to voronoi cells!".into()));
            };
            let mut z_data_voronoi = DVector::from_element(voronoi_diagram.sites.len(), f64::NAN);
            z_data_voronoi
                .view_mut((0, 0), (z_data.len(), 1))
                .set_column(0, &z_data);

            Some(z_data_voronoi)
        } else {
            None
        };

        Ok(Self {
            voronoi_diagram,
            z_data,
        })
    }

    /// Creates a new [`VoronoiedData`] struct by combining an exisiting voronoi diagram with data
    /// # Attributes
    /// `voronoi`: voronoi diagram created with voronator crate
    /// `data`: data to combine with voronoi cells
    /// # Errors
    /// This function errors if the number of voronoi cells and data points are not equal
    pub fn combine_data_with_voronoi_diagram(
        voronoi: VoronoiDiagram<VPoint>,
        data: DVector<f64>,
    ) -> OpmResult<Self> {
        if voronoi.sites.len() == data.len() {
            Ok(Self {
                voronoi_diagram: voronoi,
                z_data: Some(data),
            })
        } else {
            Err(OpossumError::Other("Number of voronoi-diagram sites and data values is not the same! Cannot combine data and voronoi cells!".into()))
        }
    }
    /// Get the voronoi diagram of the [`VoronoiedData`]
    #[must_use]
    pub const fn get_voronoi_diagram(&self) -> &VoronoiDiagram<VPoint> {
        &self.voronoi_diagram
    }

    /// Get the z dataset of the [`VoronoiedData`]
    #[must_use]
    pub const fn get_z_data(&self) -> &Option<DVector<f64>> {
        &self.z_data
    }
}

/// Calculate the area of a closed polygon using the shoelace formula
/// # Attributes
/// `poly_coords`: array of x-y coordinates of the polygon vertices
/// # Errors
/// This function returns an errror if the numer of coordinates is below 3
pub fn calc_closed_poly_area(poly_coords: &[Point2<f64>]) -> OpmResult<f64> {
    let mut area = 0.;
    let num_points = poly_coords.len();
    if num_points < 3 {
        Err(OpossumError::Other(
            "Not enough points to define a polygon!".into(),
        ))
    } else {
        for i in 0..num_points {
            let j = (i + 1) % num_points;
            if !poly_coords[i].x.is_finite() || !poly_coords[i].y.is_finite() {
                return Err(OpossumError::Other(
                    "Non-finite polygon coordinates!".into(),
                ));
            }
            area += poly_coords[i].x * poly_coords[j].y;
            area -= poly_coords[i].y * poly_coords[j].x;
        }
        Ok(area.abs() / 2.)
    }
}

/// Interpolation of scattered 3d data
///
/// Interpolation of scattered 3d data (not on a regular grid), meaning a set of "x" and "y" coordinates and a value for each data point.
/// The interpolation is done via delaunay triangulation of the data points and interpolating on the desired points (`x_interp`, `y_interp`) using barycentric coordinates of the triangles
/// # Attributes
/// `scattered_data`: Scattered data to be interpolated. The data should be structures as x-column, y-column, data-column
/// `x_interp`: x-coordinates of the points on which this function should interpolate
/// `y_interp`: y-coordinates of the points on which this function should interpolate
/// # Errors
/// This function errors if
/// - The Axlimits can not be created
/// - The triangulation (voronoi diagram) generation fails
pub fn interpolate_3d_scatter_data(
    scattered_data: &MatrixXx3<f64>,
    x_interp: &DVector<f64>,
    y_interp: &DVector<f64>,
) -> OpmResult<(DMatrix<f64>, DMatrix<f64>)> {
    let x_interp_filtered = DVector::from_vec(filter_nan_infinite(x_interp.as_slice()));
    let y_interp_filtered = DVector::from_vec(filter_nan_infinite(y_interp.as_slice()));
    if x_interp_filtered.len() < 2 || y_interp_filtered.len() < 2 {
        return Err(OpossumError::Other(
            "Length of interpolation ranges must be larger than 1 to define the interpolation bounds".into(),
        ));
    };

    if scattered_data.column(0).len() < 3 {
        return Err(OpossumError::Other(
            "Number of scattered data points must be at least 3 to define a triangle, which is necessary to interpolate!".into(),
        ));
    }
    let voronoi_data = create_valued_voronoi_cells(scattered_data)?;

    interpolate_3d_triangulated_scatter_data(&voronoi_data, &x_interp_filtered, &y_interp_filtered)
}

/// Creation of arrays from `x` and `y` coordinates
///
/// The new arrays size have a number of rows according to the length of the y input coordinates and number of columns according to the length of the x input coordinates
/// # Attributes
/// `x`: Vector of x-coordinates
/// `y`: Vector of y-coordinates
/// # Errors
/// This function errors if if the input vectors have a zero length
pub fn meshgrid(x: &DVector<f64>, y: &DVector<f64>) -> OpmResult<(DMatrix<f64>, DMatrix<f64>)> {
    let x_len = x.len();
    let y_len = y.len();

    if x_len == 0 || y_len == 0 {
        Err(OpossumError::Other(
            "Input vectors must have a non-zero length!".into(),
        ))
    } else {
        let mut x_mat = DMatrix::<f64>::zeros(y_len, x_len);
        let mut y_mat = DMatrix::<f64>::zeros(y_len, x_len);

        for x_id in 0..x_len {
            for y_id in 0..y_len {
                x_mat[(y_id, x_id)] = x[x_id];
                y_mat[(y_id, x_id)] = y[y_id];
            }
        }
        Ok((x_mat, y_mat))
    }
}

/// Creates a linearly spaced Vector (Matrix with1 column and `num` rows) from `start` to `end`
/// # Attributes
/// - `start`:  Start value of the array
/// - `end`:    end value of the array
/// - `num`:    number of elements
///
/// # Errors
/// This function will return an error if `num` cannot be casted to usize.
///
/// # Panics
/// This function panics if step cannot be casted from usize to T
pub fn linspace<T: Float + Scalar>(start: T, end: T, num: usize) -> OpmResult<DVector<T>> {
    if !start.is_finite() || !end.is_finite() {
        return Err(OpossumError::Other(
            "start and end values must be finite!".into(),
        ));
    };

    let mut linspace = DVector::<T>::from_element(num, start);
    if num < 2 {
        warn!("Using linspace with less than two elements results in an empty Vector for num=0 or a Vector with one entry being num=start");
        return Ok(linspace);
    }

    let bin_size = (end - start)
        / NumCast::from(num - 1)
            .ok_or_else(|| OpossumError::Other("Cannot Cast usize to float type!".into()))?;

    for (step, val) in linspace.iter_mut().enumerate() {
        *val = *val + <T as NumCast>::from(step).unwrap() * bin_size;
    }
    Ok(linspace)
}

/// Creates a linearly spaced Vector (Matrix with1 column and `num` rows) from `start` to `end`
/// # Attributes
/// - `start`:  Start value of the array
/// - `end`:    end value of the array
/// - `num`:    number of elements
///
/// # Errors
/// This function will return an error if `num` cannot be casted to usize.
pub fn linspace_f32(start: f32, end: f32, num: f32) -> OpmResult<DVector<f32>> {
    if !start.is_finite() || !end.is_finite() {
        return Err(OpossumError::Other(
            "start and end values must be finite!".into(),
        ));
    };

    if num < 2. {
        warn!("Using linspace with less than two elements results in an empty Vector for num=0 or a Vector with one entry being num=start");
    }
    num.to_usize().map_or_else(
        || {
            Err(OpossumError::Other(
                "Cannot cast num value to usize!".into(),
            ))
        },
        |num_usize| {
            let mut linspace = DVector::<f32>::zeros(num_usize);
            let mut range = KahanSum::<f32>::new_with_value(end);
            range += -start;

            let mut steps = KahanSum::<f32>::new_with_value(num);
            steps += -1.;

            let bin_size = range.sum() / steps.sum();

            let mut summator: KahanSum<f32> = KahanSum::<f32>::new_with_value(start);
            for val in linspace.iter_mut() {
                *val = summator.sum();
                summator += bin_size;
            }
            Ok(linspace)
        },
    )
}

/// Creates a linearly spaced Vector (Matrix with1 column and `num` rows) from `start` to `end` and an [`AxLims`] struct from data.
/// # Attributes
/// - `data`: data that defines the start- and end-points of the linearly spaced vector
/// - `num_axes_points`:    number of points
///
/// # Errors
/// This function will return an error if `num_axes_points` is below 1 or if `linspace fails`.
pub fn create_linspace_axes(
    data: DVectorView<'_, f64>,
    num_axes_points: usize,
) -> OpmResult<(DVector<f64>, AxLims)> {
    let ax_lim = AxLims::finite_from_dvector(&data)
        .ok_or_else(|| OpossumError::Other("Cannot create linspace from axlims of None!".into()))?;
    if num_axes_points < 1 {
        Err(OpossumError::Other(
            "The number of points to create linearly-spaced vector must be more than 1!".into(),
        ))
    } else {
        let ax = linspace(ax_lim.min, ax_lim.max, num_axes_points)?;
        Ok((ax, ax_lim))
    }
}

/// Creates a set of voronoi cells from scattered 2d-coordinates
/// # Attributes
/// - `xy_coord`: Matrix of x-y coordinates of scattered points with the first column being the x coordinates and the second column being the y coordinates of these points
/// # Errors
/// This function errors if
/// - coordinate values are non-finite or NAN
/// - all points are on the same line and can therefore not be triangulated properly
/// - the generation of the voronoi diagram fails
/// # Panics
/// This function panics if the number of triangles cannot be converted to f64
pub fn create_voronoi_cells(xy_coord: &MatrixXx2<f64>) -> OpmResult<(VoronoiDiagram<VPoint>, f64)> {
    //collect data to a vector of Points that can be used to create the triangulation
    let points = Iterator::map(xy_coord.row_iter(), |c| {
        if c[0].is_finite() && c[1].is_finite() {
            Ok(VPoint::from_xy(c[0], c[1]))
        } else {
            Err(OpossumError::Other(
                "Coordinate values for voronoi-diagram generation must be finite!".into(),
            ))
        }
    })
    .collect::<OpmResult<Vec<VPoint>>>()?;

    if all_points_on_same_line(xy_coord) {
        Err(OpossumError::Other(
            "All passed points lie on the same line! Triangulation not possible!".into(),
        ))
    } else {
        //create the voronoi diagram with the minimum and maximum values of the axes as bounds
        // let convex_hull_points =
        let triangulation = voronator::delaunator::triangulate(&points)
            .ok_or_else(|| OpossumError::Other("Could not create voronoi diagram!".into()))?;
        let (convex_hull_points, num_triangles) = {
            let convex_hull = triangulation.hull;
            (
                Iterator::map(convex_hull.iter().rev(), |idx| {
                    Point2::new(points[*idx].x, points[*idx].y)
                })
                .collect_vec(),
                triangulation.triangles.len() / 3,
            )
        };

        let area_hull = calc_closed_poly_area(convex_hull_points.as_slice())?;
        let area_triangle = area_hull / num_triangles.to_f64().unwrap();
        let side_length_triangle = (area_triangle * 4. / (3.).sqrt()).sqrt();
        let base_height = (side_length_triangle / 2.)
            .mul_add(-(side_length_triangle / 2.), side_length_triangle.powi(2))
            .sqrt();

        let num_points = convex_hull_points.len();
        let mut shifted_hull = Vec::<VPoint>::with_capacity(num_points * 2);
        //shoose points in between the scaled ones
        for i in 0..num_points {
            let j = (i + 1) % num_points;
            let new_point = VPoint {
                x: convex_hull_points[i].x
                    + (convex_hull_points[j].x - convex_hull_points[i].x) / 2.,
                y: convex_hull_points[i].y
                    + (convex_hull_points[j].y - convex_hull_points[i].y) / 2.,
            };
            shifted_hull.push(VPoint::from_xy(
                convex_hull_points[i].x,
                convex_hull_points[i].y,
            ));
            shifted_hull.push(new_point);
        }

        let num_points_in_hull = shifted_hull.len().to_f64().unwrap();
        let mut hull_centroid = shifted_hull.iter().fold(Point2::new(0., 0.), |arg0, p| {
            let average_val_x = f64::add(arg0.x, p.x);
            let average_val_y = f64::add(arg0.y, p.y);
            Point2::new(average_val_x, average_val_y)
        });
        hull_centroid.x /= num_points_in_hull;
        hull_centroid.y /= num_points_in_hull;

        let scaled_convex_hull = Iterator::map(shifted_hull.iter(), |p| {
            let centroid_vec = Point2::new(p.x, p.y) - hull_centroid;
            let dist_from_centroid = centroid_vec.norm();
            let scale_factor = (dist_from_centroid + base_height / 2.) / dist_from_centroid;
            VPoint::from_xy(
                scale_factor.mul_add(centroid_vec.x, hull_centroid.x),
                scale_factor.mul_add(centroid_vec.y, hull_centroid.y),
            )
        })
        .collect_vec();
        let convex_hull_polygon = polygon::Polygon::from_points(scaled_convex_hull);
        Ok((
            VoronoiDiagram::<VPoint>::with_bounding_polygon(points, &convex_hull_polygon)
                .ok_or_else(|| OpossumError::Other("Could not create voronoi diagram!".into()))?,
            area_hull,
        ))
    }
}

/// Creates a set of voronoi cells from scattered 2d-coordinates with associated values
/// # Attributes
/// - `xyz_data`: Matrix of x-y-z coordinates of scattered points with the first column being the x coordinates and the second column being the y coordinates of these points and the third column being the z valus at these points
/// # Errors
/// This function errors if the voronoir-diagram generation fails
pub fn create_valued_voronoi_cells(xyz_data: &MatrixXx3<f64>) -> OpmResult<VoronoiedData> {
    let (voronoi_diagram, _) = create_voronoi_cells(&MatrixXx2::from_columns(&[
        xyz_data.column(0),
        xyz_data.column(1),
    ]))?;

    let z_data = xyz_data.column(2);
    let mut z_data_voronoi = DVector::from_element(voronoi_diagram.sites.len(), f64::NAN);
    z_data_voronoi
        .view_mut((0, 0), (z_data.len(), 1))
        .set_column(0, &z_data);

    VoronoiedData::combine_data_with_voronoi_diagram(voronoi_diagram, z_data_voronoi)
}

/// Interpolation of scattered 3d data
///
/// Interpolation of scattered 3d data  (not on a regular grid), meaning a set of "x" and "y" coordinates and a value for each data point.
/// The interpolation is done via delaunay triangulation (retrieved from a voronoi diagram, created with voronator) of the data points and interpolating on the desired points (`x_interp`, `y_interp`) using barycentric coordinates of the triangles
/// # Attributes
/// `voronoi`: Reference to a `VoronoiDiagram` struct
/// `z_data`: values referring to the voronoi cell
/// `x_interp`: x-coordinates of the points on which this function should interpolate
/// `y_interp`: y-coordinates of the points on which this function should interpolate
/// # Returns
/// This function returns the interpolated data and a mask that marks the points that have been interpolated
/// # Errors
/// This function errors if any of the interpolation vectors have zero length
/// # Panics
/// This function panics if the conversion from usize to f64 fails. May be the case for extremely large numbers
#[allow(clippy::too_many_lines)]
pub fn interpolate_3d_triangulated_scatter_data(
    voronoi: &VoronoiedData,
    x_interp: &DVector<f64>,
    y_interp: &DVector<f64>,
) -> OpmResult<(DMatrix<f64>, DMatrix<f64>)> {
    let num_axes_points_x = x_interp.len();
    let num_axes_points_y = y_interp.len();
    let Some(z_data) = voronoi.get_z_data() else {
        return Err(OpossumError::Other(
            "No data defined! Cannot interpolate!".into(),
        ));
    };
    if num_axes_points_x < 1 || num_axes_points_y < 1 {
        return Err(OpossumError::Other(
            "Cannot interpolate data, as one of the interpolation vectors have zero length".into(),
        ));
    };

    let mut triangulation: DelaunayTriangulation<PointWithHeight> = DelaunayTriangulation::new();
    //copy points of voronoi diag into spade triangulation
    for (p, z) in voronoi.voronoi_diagram.sites.iter().zip(z_data.iter()) {
        if z.is_finite() {
            // let neighbours = &voronoi.voronoi_diagram.neighbors[idx];

            // let mut normal = Vector3::new(0.,0.,0.);

            // let p1 = Point3::new(p.x, p.y, *z);
            // let p2 = voronoi.voronoi_diagram.sites[*neighbours.last().expect("No edges!")];
            // let mut p2 = Point3::new(p2.x, p2.y, z_data[idx]);
            // for point_idx in neighbours{
            //     let p3 = voronoi.voronoi_diagram.sites[*point_idx];
            //     let p3 = Point3::new(p3.x, p3.y, z_data[*point_idx]);
            //     let vec1 = p2 - p1;
            //     let vec2: nalgebra::Matrix<f64, nalgebra::Const<3>, nalgebra::Const<1>, nalgebra::ArrayStorage<f64, 3, 1>> = p3 - p1;
            //     normal += vec1.cross(&vec2);
            //     p2 = p3;
            // }
            // normal = normal.normalize();

            triangulation
                .insert(PointWithHeight {
                    position: SpadeP::new(p.x, p.y),
                    height: *z,
                    // normal
                })
                .map_err(|_| {
                    OpossumError::Other("Inserting Point into Spade triangulation failed".into())
                })?;
        }
    }

    let mut interp_data =
        DMatrix::<f64>::from_element(num_axes_points_x, num_axes_points_y, f64::NAN);
    let mut mask = DMatrix::from_element(num_axes_points_x, num_axes_points_y, 0.);

    let mm = triangulation.natural_neighbor();
    for (x_index, x) in x_interp.iter().enumerate() {
        for (y_index, y) in y_interp.iter().enumerate() {
            let interp_point = mm.interpolate(|v| v.data().height, SpadeP::new(*x, *y));
            // let interp_point = mm.interpolate_gradient(|v| v.data().height, |v| [v.data().normal[0], v.data().normal[1]], 1., SpadeP::new(*x, *y));
            if let Some(p) = interp_point {
                interp_data[(y_index, x_index)] = p;
                mask[(y_index, x_index)] = 1.;
            }
        }
    }

    Ok((interp_data, mask))
}

fn all_points_on_same_line(point_coords: &MatrixXx2<f64>) -> bool {
    let num_points = point_coords.column(0).len();
    if num_points < 3 {
        //two points are always on the same line
        true
    } else {
        let line_1 = point_coords.row(1) - point_coords.row(0);
        let mut on_line = true;
        for i in 2..num_points {
            let line_2 = point_coords.row(i) - point_coords.row(0);
            if abs_diff_ne!(line_1[0].mul_add(line_2[1], -(line_1[1] * line_2[0])), 0.) {
                on_line = false;
                break;
            }
        }
        on_line
    }
}

#[cfg(test)]
mod test {
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::{Matrix2xX, Matrix3xX};

    use super::*;
    #[test]
    fn all_points_on_same_line_test() {
        let points = Matrix2xX::from_vec(vec![0., 0., 1., 1., 2., 2.]).transpose();
        assert!(all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![0., 0., 1., 1., 2., 2., 2., 3.]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![0., 0., 1., 1.]).transpose();
        assert!(all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![0., f64::NAN, 1., 1., 2., 2.]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![0., f64::INFINITY, 1., 1., 2., 2.]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![0., f64::NEG_INFINITY, 1., 1., 2., 2.]).transpose();
        assert!(!all_points_on_same_line(&points));
    }
    #[test]
    fn new_voronoi_data() {
        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5, 3.0, 15.]).transpose();
        let z_data = DVector::from_vec(vec![-10., 0., 500.]);
        assert!(VoronoiedData::new(&xy_coord, Some(z_data)).is_ok());

        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5]).transpose();
        let z_data = DVector::from_vec(vec![-10., 0., 500.]);
        assert!(VoronoiedData::new(&xy_coord, Some(z_data)).is_err());
    }
    #[test]
    fn get_voronoi_diagram_test() {
        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5, 3.0, 15.]).transpose();
        let (voronoi, _) = create_voronoi_cells(&xy_coord).unwrap();

        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5, 3.0, 15.]).transpose();
        let z_data = DVector::from_vec(vec![-10., 0., 500.]);
        let v_dat = VoronoiedData::new(&xy_coord, Some(z_data)).unwrap();

        let v_diag = v_dat.get_voronoi_diagram();

        assert_eq!(voronoi, v_diag.clone());
    }
    #[test]
    fn get_z_data_test() {
        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5, 3.0, 15.]).transpose();
        let v_dat =
            VoronoiedData::new(&xy_coord, Some(DVector::from_vec(vec![-10., 0., 500.]))).unwrap();

        let z_dat = v_dat.get_z_data();
        assert!(z_dat.is_some());
        let z_dat = z_dat.clone().unwrap();

        assert_relative_eq!(-10., z_dat[0]);
        assert_relative_eq!(0., z_dat[1]);
        assert_relative_eq!(500., z_dat[2]);
        assert!(z_dat[3].is_nan());
        assert!(z_dat[4].is_nan());
        assert!(z_dat[5].is_nan());
        assert!(z_dat[6].is_nan());

        let v_dat = VoronoiedData::new(&xy_coord, None).unwrap();

        let z_dat = v_dat.get_z_data();
        assert!(z_dat.is_none());
    }
    #[test]
    fn calc_closed_poly_area_test() {
        let poly_triangle = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0., 1.),
        ];
        let poly_rect = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(1., 1.),
            Point2::new(0., 1.),
        ];
        let poly_oct = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(2., 1.),
            Point2::new(2., 2.),
            Point2::new(1., 3.),
            Point2::new(0., 3.),
            Point2::new(-1., 2.),
            Point2::new(-1., 1.),
        ];
        let poly_same_line = vec![
            Point2::new(0., 0.),
            Point2::new(1., 1.),
            Point2::new(2., 2.),
        ];

        assert_relative_eq!(calc_closed_poly_area(&poly_triangle).unwrap(), 0.5);
        assert_relative_eq!(calc_closed_poly_area(&poly_rect).unwrap(), 1.);
        assert_relative_eq!(calc_closed_poly_area(&poly_oct).unwrap(), 7.);
        assert_relative_eq!(calc_closed_poly_area(&poly_same_line).unwrap(), 0.);
    }
    #[test]
    fn calc_closed_poly_area_invalid_polygon_test() {
        let poly_triangle = vec![Point2::new(0., 0.), Point2::new(1., 0.)];
        assert!(calc_closed_poly_area(&poly_triangle).is_err());
        let poly_triangle = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0., f64::NAN),
        ];
        assert!(calc_closed_poly_area(&poly_triangle).is_err());
        let poly_triangle = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0., f64::INFINITY),
        ];
        assert!(calc_closed_poly_area(&poly_triangle).is_err());
        let poly_triangle = vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0., f64::NEG_INFINITY),
        ];
        assert!(calc_closed_poly_area(&poly_triangle).is_err());
    }
    #[test]
    fn interpolate_3d_scatter_data_value_test() {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();

        let x_interp = linspace(-0.5, 1., 4).unwrap();
        let y_interp = linspace(-0.5, 1., 4).unwrap();
        let (interp_data, _) =
            interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).unwrap();

        assert!(interp_data[(0, 0)].is_nan());
        assert!(interp_data[(0, 1)].is_nan());
        assert!(interp_data[(0, 2)].is_nan());
        assert!(interp_data[(0, 3)].is_nan());
        assert!(interp_data[(1, 0)].is_nan());
        assert_abs_diff_eq!(interp_data[(1, 1)], 0.0);
        assert_abs_diff_eq!(interp_data[(1, 2)], 0.0);
        assert_abs_diff_eq!(interp_data[(1, 3)], 0.0);
        assert!(interp_data[(2, 0)].is_nan());
        assert_abs_diff_eq!(interp_data[(2, 1)], 0.5);
        assert_abs_diff_eq!(interp_data[(2, 2)], 0.5);
        assert_abs_diff_eq!(interp_data[(2, 3)], 0.5);
        assert!(interp_data[(3, 0)].is_nan());
        assert_abs_diff_eq!(interp_data[(3, 1)], 1.0);
        assert_abs_diff_eq!(interp_data[(3, 2)], 1.0);
        assert_abs_diff_eq!(interp_data[(3, 3)], 1.0);
    }
    #[test]
    fn interpolate_3d_scatter_data_mask_test() {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();
        println!("{}", scattered_data);
        let x_interp = linspace(-0.5, 1., 4).unwrap();
        let y_interp = linspace(-0.5, 1., 4).unwrap();
        let (_data, mask) =
            interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).unwrap();

        assert_abs_diff_eq!(mask[(0, 0)], 0.);
        assert_abs_diff_eq!(mask[(0, 1)], 0.);
        assert_abs_diff_eq!(mask[(0, 2)], 0.);
        assert_abs_diff_eq!(mask[(0, 3)], 0.);
        assert_abs_diff_eq!(mask[(1, 0)], 0.);
        assert_abs_diff_eq!(mask[(1, 1)], 1.);
        assert_abs_diff_eq!(mask[(1, 2)], 1.);
        assert_abs_diff_eq!(mask[(1, 3)], 1.);
        assert_abs_diff_eq!(mask[(2, 0)], 0.);
        assert_abs_diff_eq!(mask[(2, 1)], 1.);
        assert_abs_diff_eq!(mask[(2, 2)], 1.);
        assert_abs_diff_eq!(mask[(2, 3)], 1.);
        assert_abs_diff_eq!(mask[(3, 0)], 0.);
        assert_abs_diff_eq!(mask[(3, 1)], 1.0);
        assert_abs_diff_eq!(mask[(3, 2)], 1.0);
        assert_abs_diff_eq!(mask[(3, 3)], 1.0);
    }

    #[test]
    fn interpolate_3d_scatter_data_amount_data_test() {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();
        let x_interp = linspace(-0.5, 1., 4).unwrap();
        let y_interp = linspace(-0.5, 1., 4).unwrap();
        assert!(interpolate_3d_scatter_data(
            &scattered_data,
            &DVector::from_vec(vec![0.]),
            &y_interp
        )
        .is_err());
        assert!(interpolate_3d_scatter_data(
            &scattered_data,
            &x_interp,
            &DVector::from_vec(vec![0.])
        )
        .is_err());
        assert!(interpolate_3d_scatter_data(
            &scattered_data,
            &DVector::from_vec(vec![0.]),
            &DVector::from_vec(vec![0.])
        )
        .is_err());

        let scattered_data = Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
    }
    #[test]
    fn interpolate_3d_scatter_data_finite_values_test() {
        let x_interp = linspace(-0.5, 1., 4).unwrap();
        let y_interp = linspace(-0.5, 1., 4).unwrap();

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., f64::NAN, 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., f64::NEG_INFINITY, 1., 0., 0., 0., 1., 0.])
                .transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., f64::INFINITY, 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., -1., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());
    }
    #[test]
    fn interpolate_3d_scatter_data_finite_coordinates_test() {
        let x_interp = linspace(-0.5, 1., 4).unwrap();
        let y_interp = linspace(-0.5, 1., 4).unwrap();
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::NAN, 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::NEG_INFINITY, 0., 0., 1., 0., 0., 0., 1., 0.])
                .transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::INFINITY, 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();

        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();

        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());
    }
    #[test]
    fn interpolate_3d_triangulated_scatter_data_test() {
        let xy_coord = Matrix2xX::from_vec(vec![0.0, 0.0, 1.0, 0.0, 0.5, 1.]).transpose();
        let z_data = DVector::from_vec(vec![0., 0., 1.]);
        let v_data = VoronoiedData::new(&xy_coord, Some(z_data)).unwrap();

        let x_interp = linspace(0.5, 1., 1).unwrap();
        let y_interp = linspace(0.5, 1., 1).unwrap();
        let (interp_data, _) =
            interpolate_3d_triangulated_scatter_data(&v_data, &x_interp, &y_interp).unwrap();
        assert_relative_eq!(interp_data[(0, 0)], 0.5);

        let x_interp = linspace(0., 1., 3).unwrap();
        let y_interp = linspace(0., 1., 3).unwrap();
        let (interp_data, _interp_mask) =
            interpolate_3d_triangulated_scatter_data(&v_data, &x_interp, &y_interp).unwrap();

        assert_relative_eq!(interp_data[(0, 0)], 0.);
        assert_relative_eq!(interp_data[(0, 1)], 0.);
        assert_relative_eq!(interp_data[(0, 2)], 0.);
        assert!(interp_data[(1, 0)].is_nan());
        assert_relative_eq!(interp_data[(1, 1)], 0.5);
        assert!(interp_data[(1, 2)].is_nan());
        assert!(interp_data[(2, 0)].is_nan());
        assert_relative_eq!(interp_data[(2, 1)], 1.);
        assert!(interp_data[(2, 2)].is_nan());
    }
    #[test]
    fn meshgrid_value_test() {
        let x = linspace(1., 3., 3).unwrap();
        let y = linspace(4., 5., 2).unwrap();

        let (yy, xx) = meshgrid(&y, &x).unwrap();

        assert_relative_eq!(x[0], xx[(0, 0)]);
        assert_relative_eq!(x[0], xx[(0, 1)]);
        assert_relative_eq!(x[1], xx[(1, 0)]);
        assert_relative_eq!(x[1], xx[(1, 1)]);
        assert_relative_eq!(x[2], xx[(2, 0)]);
        assert_relative_eq!(x[2], xx[(2, 1)]);

        assert_relative_eq!(y[0], yy[(0, 0)]);
        assert_relative_eq!(y[0], yy[(1, 0)]);
        assert_relative_eq!(y[0], yy[(2, 0)]);
        assert_relative_eq!(y[1], yy[(0, 1)]);
        assert_relative_eq!(y[1], yy[(1, 1)]);
        assert_relative_eq!(y[1], yy[(2, 1)]);
    }
    #[test]
    fn meshgrid_shape_test() {
        let x = linspace(1., 3., 3).unwrap();
        let y = linspace(4., 5., 2).unwrap();

        let (xx, yy) = meshgrid(&x, &y).unwrap();
        assert_eq!(xx.shape(), (2, 3));
        assert_eq!(yy.shape(), (2, 3));
        let (yy, xx) = meshgrid(&y, &x).unwrap();
        assert_eq!(xx.shape(), (3, 2));
        assert_eq!(yy.shape(), (3, 2));
    }
    #[test]
    fn linspace_test() {
        let x = linspace(1., 3., 3).unwrap();
        assert_eq!(x.len(), 3);
        assert_abs_diff_eq!(x[0], 1.);
        assert_abs_diff_eq!(x[1], 2.);
        assert_abs_diff_eq!(x[2], 3.);
        // assert!(linspace(1., 3., -3).is_err());

        assert!(linspace(1., f64::NAN, 3).is_err());
        assert!(linspace(f64::NAN, 3., 3).is_err());
        assert!(linspace(f64::INFINITY, 3., 3).is_err());
        assert!(linspace(f64::NEG_INFINITY, 3., 3).is_err());
        assert!(linspace(1., f64::NEG_INFINITY, 3).is_err());
        assert!(linspace(1., f64::INFINITY, 3).is_err());
        // assert!(linspace(1., 10., f64::INFINITY).is_err());
        // assert!(linspace(1., 10., f64::NEG_INFINITY).is_err());
        // assert!(linspace(1., 10., f64::NAN).is_err());
    }
    #[test]
    fn create_linspace_axes_test() {
        let x_dat = DVector::from_vec(vec![0., -3., 10., 50.]);
        let num_axes_points = 100;
        let (x, xlim) = create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).unwrap();
        assert_eq!(x.len(), 100);
        assert_abs_diff_eq!(xlim.min, -3.);
        assert_abs_diff_eq!(xlim.max, 50.);
        assert_abs_diff_eq!(xlim.min, x[0]);
        assert_abs_diff_eq!(xlim.max, x[99]);

        let x_dat = DVector::from_vec(vec![0., -3., 10., f64::INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0., -3., 10., f64::NEG_INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0., -3., 10., f64::NAN]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0., 0., f64::NAN, f64::INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0., f64::NAN, f64::INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0., 0.]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0., f64::NAN]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0., f64::INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0., f64::NEG_INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![f64::NAN, f64::NAN, f64::NAN]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), num_axes_points).is_err());

        let x_dat = DVector::from_vec(vec![0., -3., 10., f64::INFINITY]);
        assert!(create_linspace_axes(DVectorView::from(&x_dat), 0).is_err());
        // assert!(create_linspace_axes(DVectorView::from(&x_dat), f64::NAN).is_err());
        // assert!(create_linspace_axes(DVectorView::from(&x_dat), f64::INFINITY).is_err());
        // assert!(create_linspace_axes(DVectorView::from(&x_dat), f64::NEG_INFINITY).is_err());
        // assert!(create_linspace_axes(DVectorView::from(&x_dat), -1.).is_err());
    }
    #[test]
    fn create_voronoi_cells_same_line_test() {
        let xy_coord = Matrix2xX::from_vec(vec![0., 0., 1., 1., 2., 2., 1000., 1000.]).transpose();
        let voronoi = create_voronoi_cells(&xy_coord);
        assert!(voronoi.is_err());
    }
    #[test]
    fn create_voronoi_cells_site_coordinates_test() {
        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5, 0.0, 2.5, 3.0, 15.]).transpose();
        let voronoi = create_voronoi_cells(&xy_coord);
        assert!(voronoi.is_ok());

        let (unwrapped_voronoi, _) = voronoi.unwrap();
        assert_relative_eq!(unwrapped_voronoi.sites[0].x, 1.0);
        assert_relative_eq!(unwrapped_voronoi.sites[0].y, 1.5);
        assert_relative_eq!(unwrapped_voronoi.sites[1].x, 0.0);
        assert_relative_eq!(unwrapped_voronoi.sites[1].y, 2.5);
        assert_relative_eq!(unwrapped_voronoi.sites[2].x, 3.0);
        assert_relative_eq!(unwrapped_voronoi.sites[2].y, 15.);
    }
    #[test]
    fn create_voronoi_cells_invalid_site_coordinates_test() {
        let xy_coord = Matrix2xX::from_vec(vec![1.0, f64::NAN]).transpose();
        let voronoi = create_voronoi_cells(&xy_coord);
        assert!(voronoi.is_err());
    }
    #[test]
    fn create_valued_voronoi_cells_test() {
        let xyz_coord = MatrixXx3::<f64>::zeros(0);
        let voronoi = create_valued_voronoi_cells(&xyz_coord);
        assert!(voronoi.is_err());

        let xyz_coord = Matrix3xX::from_vec(vec![1.0, 1.5, 10.]).transpose();
        let voronoi = create_valued_voronoi_cells(&xyz_coord);
        assert!(voronoi.is_err());

        let xyz_coord =
            Matrix3xX::from_vec(vec![1.0, 1.5, 10., 2.0, 2.5, 20., -1.0, -3.5, 30.]).transpose();
        let voronoi = create_valued_voronoi_cells(&xyz_coord);
        assert!(voronoi.is_ok());

        let unwrapped_voronoi = voronoi.unwrap();
        let z_data = unwrapped_voronoi.z_data.unwrap();
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[0].x, 1.0);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[0].y, 1.5);
        assert_relative_eq!(z_data[0], 10.);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[1].x, 2.0);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[1].y, 2.5);
        assert_relative_eq!(z_data[1], 20.);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[2].x, -1.0);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[2].y, -3.5);
        assert_relative_eq!(z_data[2], 30.);
    }
}
