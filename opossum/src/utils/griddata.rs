use approx::{abs_diff_eq, abs_diff_ne, assert_abs_diff_eq};
use kahan::KahanSum;
use log::warn;
use nalgebra::{DMatrix, DVector, DVectorSlice, Matrix3xX, MatrixXx2, MatrixXx3, Point2};
use num::ToPrimitive;
use voronator::{
    delaunator::{Coord, Point as VPoint},
    VoronoiDiagram,
};

use crate::{
    error::{OpmResult, OpossumError},
    plottable::AxLims,
};

use super::filter_data::filter_nan_infinite;

pub struct VoronoiData {
    voronoi_diagram: VoronoiDiagram<VPoint>,
    z_data: DVector<f64>,
}

impl VoronoiData {
    #[must_use]
    pub const fn new(voronoi_diagram: VoronoiDiagram<VPoint>, z_data: DVector<f64>) -> Self {
        Self {
            voronoi_diagram,
            z_data,
        }
    }
    #[must_use]
    pub const fn get_voronoi_diagram(&self) -> &VoronoiDiagram<VPoint> {
        &self.voronoi_diagram
    }
    #[must_use]
    pub const fn get_z_data(&self) -> &DVector<f64> {
        &self.z_data
    }
}

/// Calculate the area of a closed polygon using the shoelace formula
/// # Attributes
/// `poly_coords`: array of x-y coordinates of the polygon vertices
/// # Errors
/// This function errors if the numer of coordinates is below 3
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
            area += poly_coords[i].x * poly_coords[j].y;
            area -= poly_coords[i].y * poly_coords[j].x;
        }
        Ok(area.abs() / 2.)
    }
}

/// Interpolation of scattered 3d data (not on a regular grid), meaning a set of "x" and "y" coordinates and a value for each data point.
/// The interpolation is done via delaunay triangulation of the data points and interpolating on the desired points (`y_interp`, `y_interp`) using barycentric coordinates of the triangles
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
    let x_interp_filtered = filter_nan_infinite(&DVectorSlice::from(x_interp));
    let y_interp_filtered = filter_nan_infinite(&DVectorSlice::from(y_interp));
    if x_interp_filtered.len() < 2 || y_interp_filtered.len() < 2{
        return Err(OpossumError::Other(
            "Length of interpolation ranges must be larger than 1 to define the interpolation bounds".into(),
        ));
    };

    if scattered_data.column(0).len() < 3{
        return Err(OpossumError::Other(
            "Number of scattered data points must be at least 3 to define a triangle, which is necessary to interpolate!".into(),
        ));
    }
    let x_bounds = AxLims::new(x_interp_filtered.min(), x_interp_filtered.max())?;
    let y_bounds = AxLims::new(y_interp_filtered.min(), y_interp_filtered.max())?;
    let voronoi_data = create_valued_voronoi_cells(scattered_data, &x_bounds, &y_bounds)?;

    interpolate_3d_triangulated_scatter_data(&voronoi_data, &x_interp_filtered, &y_interp_filtered)
}

/// Creation of arrays from `x` and `y` coordinates.
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
pub fn linspace(start: f64, end: f64, num: f64) -> OpmResult<DVector<f64>> {
    if !start.is_finite() || !end.is_finite() {
        return Err(OpossumError::Other(
            "start and end values must be finite!".into(),
        ));
    };

    if num < 2. {
        warn!("Using linspace with less than two elements results in an empty Vector for num=0 or a Vector with one entry being num=start")
    }
    num.to_usize().map_or_else(
        || {
            Err(OpossumError::Other(
                "Cannot cast num value to usize!".into(),
            ))
        },
        |num_usize| {
            let mut linspace = DVector::<f64>::zeros(num_usize);
            let mut range = KahanSum::<f64>::new_with_value(end);
            range += -start;

            let mut steps = KahanSum::<f64>::new_with_value(num);
            steps += -1.;

            let bin_size = range.sum() / steps.sum();

            let mut summator: KahanSum<f64> = KahanSum::<f64>::new_with_value(start);
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
    data: DVectorSlice<'_, f64>,
    num_axes_points: f64,
) -> OpmResult<(DVector<f64>, AxLims)> {
    let filtered_data = filter_nan_infinite(&data);
    if filtered_data.len() < 2{
        return Err(OpossumError::Other(
            "Length of input data after filtering out non-finite values is below 2! Creating a linearly-spaced array from 1 value is not possible!".into(),
        ))
    }
    let ax_lim = AxLims::new(filtered_data.min(), filtered_data.max())?;
    if num_axes_points < 1. {
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
/// - `x_bounds`: Boundary x-coordinates of the scattered data points.
/// - `y_bounds`: Boundary y-coordinates of the scattered data points.
#[must_use]
pub fn create_voronoi_cells(
    xy_coord: &MatrixXx2<f64>,
    x_bounds: &AxLims,
    y_bounds: &AxLims,
) -> OpmResult<VoronoiDiagram<VPoint>> {
    //collect data to a vector of Points that can be used to create the triangulation
    let points = xy_coord
        .row_iter()
        .map(|c| {
            if c[0].is_finite() && c[1].is_finite() {
                Ok(VPoint::from_xy(c[0], c[1]))
            } else {
                Err(OpossumError::Other(
                    "Coordinate values for voronoi-diagram generation must be finite!".into(),
                ))
            }
        })
        .collect::<OpmResult<Vec<VPoint>>>()?;

    
    if !x_bounds.check_validity() || !y_bounds.check_validity()
    {
        Err(OpossumError::Other(
            "Boundary values for voronoi-diagram generation must be finite!".into(),
        ))
    } 
    else if all_points_on_same_line(xy_coord){
        Err(OpossumError::Other(
            "The passed points lie on the same line! Triangulation not possible!".into(),
        ))
    }
    else {
        //create the voronoi diagram with the minimum and maximum values of the axes as bounds
        VoronoiDiagram::<VPoint>::new(
            &VPoint::from_xy(x_bounds.min, y_bounds.min),
            &VPoint::from_xy(x_bounds.max, y_bounds.max),
            &points,
        )
        .ok_or_else(|| OpossumError::Other("Could not create voronoi diagram!".into()))
    }
}

/// Creates a set of voronoi cells from scattered 2d-coordinates with associated values
/// # Attributes
/// - `xyz_data`: Matrix of x-y-z coordinates of scattered points with the first column being the x coordinates and the second column being the y coordinates of these points and the third column being the z valus at these points
/// - `x_bounds`: Boundary x-coordinates of the scattered data points.
/// - `y_bounds`: Boundary y-coordinates of the scattered data points.
/// # Errors
/// This function errors if the voronoir-diagram generation fails
pub fn create_valued_voronoi_cells(
    xyz_data: &MatrixXx3<f64>,
    x_bounds: &AxLims,
    y_bounds: &AxLims,
) -> OpmResult<VoronoiData> {
    let voronoi_diagram = create_voronoi_cells(
        &MatrixXx2::from_columns(&[xyz_data.column(0), xyz_data.column(1)]),
        x_bounds,
        y_bounds,
    )?;

    let z_data = xyz_data.column(2);
    let mut z_data_voronoi = DVector::from_element(voronoi_diagram.sites.len(), f64::NAN);
    z_data_voronoi.slice_mut((0,0), (z_data.len(),1)).set_column(0, &z_data);

    Ok(VoronoiData::new(
        voronoi_diagram,
        DVector::from_column_slice(&z_data_voronoi.as_slice()),
    ))
}

/// Interpolation of scattered 3d data (not on a regular grid), meaning a set of "x" and "y" coordinates and a value for each data point.
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
    voronoi: &VoronoiData,
    x_interp: &DVector<f64>,
    y_interp: &DVector<f64>,
) -> OpmResult<(DMatrix<f64>, DMatrix<f64>)> {
    let num_axes_points_x = x_interp.len();
    let num_axes_points_y = y_interp.len();

    if num_axes_points_x < 1 || num_axes_points_y < 1 {
        return Err(OpossumError::Other(
            "Cannot interpolate data, as one of the interpolation vectors have zero length".into(),
        ));
    };
    let mut interp_data =
        DMatrix::<f64>::from_element(num_axes_points_x, num_axes_points_y, f64::NAN);
    let tri_index_mat =
        Matrix3xX::from_vec(voronoi.voronoi_diagram.delaunay.triangles.clone()).transpose();
    let mut mask = DMatrix::from_element(num_axes_points_x, num_axes_points_y, 0.);
    let num_axes_points_x = num_axes_points_x.to_f64().unwrap();
    let num_axes_points_y = num_axes_points_x.to_f64().unwrap();

    let x_binning = x_interp[1] - x_interp[0];
    let y_binning = y_interp[1] - y_interp[0];

    let v_cell_sites = &voronoi.voronoi_diagram.sites;

    for tri_idxs in tri_index_mat.row_iter() {
        let p1_x = &v_cell_sites[tri_idxs[0]].x;
        let p1_y = &v_cell_sites[tri_idxs[0]].y;
        let p2_x = &v_cell_sites[tri_idxs[1]].x;
        let p2_y = &v_cell_sites[tri_idxs[1]].y;
        let p3_x = &v_cell_sites[tri_idxs[2]].x;
        let p3_y = &v_cell_sites[tri_idxs[2]].y;

        let p1p2_x = p2_x - p1_x;
        let p1p2_y = p2_y - p1_y;
        let p2p3_x = p3_x - p2_x;
        let p2p3_y = p3_y - p2_y;
        let p3p1_x = p1_x - p3_x;
        let p3p1_y = p1_y - p3_y;

        let tri_area = (-p1p2_x).mul_add(p3p1_y, p1p2_y * p3p1_x).abs();

        let (x_min, x_max, y_min, y_max) = tri_idxs
            .iter()
            .map(|p_id| (v_cell_sites[*p_id].x, v_cell_sites[*p_id].y))
            .fold(
                (
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ),
                |arg, v: (f64, f64)| {
                    (
                        f64::min(arg.0, v.0),
                        f64::max(arg.1, v.0),
                        f64::min(arg.2, v.1),
                        f64::max(arg.3, v.1),
                    )
                },
            );

        let x_i = (x_min - x_interp[0]) / x_binning;
        let x_f = (x_max - x_interp[0]) / x_binning;
        let y_i = (y_min - y_interp[0]) / y_binning;
        let y_f = (y_max - y_interp[0]) / y_binning;

        if x_i.is_sign_negative()
            || y_i.is_sign_negative()
            || y_f >= num_axes_points_y
            || x_f >= num_axes_points_x
        {
            continue;
        }


        let x_i = x_i.to_usize().unwrap();
        let x_f = x_f.to_usize().unwrap();
        let y_i = y_i.to_usize().unwrap();
        let y_f = y_f.to_usize().unwrap();

        let c11 = p1_x * p1p2_y;
        let c12 = p1_y * p1p2_x;
        let c21 = p2_x * p2p3_y;
        let c22 = p2_y * p2p3_x;
        let c31 = p3_x * p3p1_y;
        let c32 = p3_y * p3p1_x;

        for (x_index, x) in x_interp
            .slice((x_i, 0), (x_f - x_i + 1, 1))
            .iter()
            .enumerate()
        {
            for (y_index, y) in y_interp
                .slice((y_i, 0), (y_f - y_i + 1, 1))
                .iter()
                .enumerate()
            {
                let cross_1 = c11 - x * p1p2_y - (c12 - y * p1p2_x);
                let cross_2 = c21 - x * p2p3_y - (c22 - y * p2p3_x);
                let cross_3 = c31 - x * p3p1_y - (c32 - y * p3p1_x);

                let in_tri = (cross_1.is_sign_negative()
                    && cross_2.is_sign_negative()
                    && cross_3.is_sign_negative())
                    || (cross_1.is_sign_positive()
                        && cross_2.is_sign_positive()
                        && cross_3.is_sign_positive())
                    || abs_diff_eq!(cross_1, 0.)
                    || abs_diff_eq!(cross_2, 0.)
                    || abs_diff_eq!(cross_3, 0.);

                if in_tri {
                    let p1xpx = p1_x - x;
                    let p1ypy = p1_y - y;
                    let p2xpx = p2_x - x;
                    let p2ypy = p2_y - y;
                    let p3xpx = p3_x - x;
                    let p3ypy = p3_y - y;

                    let area1 = p2xpx.mul_add(p3ypy, -(p2ypy * p3xpx)).abs();
                    let area2 = p3xpx.mul_add(p1ypy, -(p3ypy * p1xpx)).abs();
                    let area3 = p1xpx.mul_add(p2ypy, -(p1ypy * p2xpx)).abs();

                    interp_data[(y_index + y_i, x_index + x_i)] =
                        area1 * voronoi.z_data[tri_idxs[0]];
                    interp_data[(y_index + y_i, x_index + x_i)] +=
                        area2 * voronoi.z_data[tri_idxs[1]];
                    interp_data[(y_index + y_i, x_index + x_i)] +=
                        area3 * voronoi.z_data[tri_idxs[2]];
                    interp_data[(y_index + y_i, x_index + x_i)] /= tri_area;

                    mask[(y_index + y_i, x_index + x_i)] = 1.;
                }
            }
        }
    }
    Ok((interp_data, mask))
}

fn all_points_on_same_line(point_coords: &MatrixXx2<f64>) -> bool{
    let num_points = point_coords.column(0).len();
    if num_points < 3 {
        //two points are always on the same line
        true
    } else {
        let line_1 = point_coords.row(1) - point_coords.row(0);
        let mut on_line = true;
        for i in 2..num_points {
            let line_2 = point_coords.row(i) - point_coords.row(0);
            if abs_diff_ne!(line_1[0]*line_2[1] - line_1[1]*line_2[0], 0.){
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
    use nalgebra::Matrix2xX;


    use super::*;
    #[test]
    fn all_points_on_same_line_test(){
        let points = Matrix2xX::from_vec(vec![
            0., 0.,
            1., 1.,
            2., 2.,
            ]).transpose();
        assert!(all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![
            0., 0.,
            1., 1.,
            2., 2.,
            2., 3.,
            ]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![
            0., 0.,
            1., 1.,
            ]).transpose();
        assert!(all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![
            0., f64::NAN,
            1., 1.,
            2., 2.,
            ]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![
            0., f64::INFINITY,
            1., 1.,
            2., 2.,
            ]).transpose();
        assert!(!all_points_on_same_line(&points));

        let points = Matrix2xX::from_vec(vec![
            0., f64::NEG_INFINITY,
            1., 1.,
            2., 2.,
            ]).transpose();
        assert!(!all_points_on_same_line(&points));

    }
    #[test]
    fn new_voronoi_data() {
        todo!()
    }
    #[test]
    fn get_voronoi_diagram_test() {
        todo!()
    }
    #[test]
    fn get_z_data_test() {
        todo!()
    }
    #[test]
    fn calc_closed_poly_area_test() {
        todo!()
    }
    #[test]
    fn interpolate_3d_scatter_data_value_test() {
        let scattered_data = Matrix3xX::from_vec(vec![
            0., 0., 0.,
            1., 0., 0.,
            0., 1., 1.,
            1., 1., 1.
            ]).transpose();

        let x_interp = linspace(-0.5, 1., 4.).unwrap();
        let y_interp = linspace(-0.5, 1., 4.).unwrap();
        let (interp_data, _) = interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).unwrap();

        assert!(interp_data[(0,0)].is_nan());
        assert!(interp_data[(0,1)].is_nan());
        assert!(interp_data[(0,2)].is_nan());
        assert!(interp_data[(0,3)].is_nan());
        assert!(interp_data[(1,0)].is_nan());
        assert_abs_diff_eq!(interp_data[(1,1)], 0.0);
        assert_abs_diff_eq!(interp_data[(1,2)], 0.0);
        assert_abs_diff_eq!(interp_data[(1,3)], 0.0);
        assert!(interp_data[(2,0)].is_nan());
        assert_abs_diff_eq!(interp_data[(2,1)], 0.5);
        assert_abs_diff_eq!(interp_data[(2,2)], 0.5);
        assert_abs_diff_eq!(interp_data[(2,3)], 0.5);
        assert!(interp_data[(3,0)].is_nan());
        assert_abs_diff_eq!(interp_data[(3,1)], 1.0);
        assert_abs_diff_eq!(interp_data[(3,2)], 1.0);
        assert_abs_diff_eq!(interp_data[(3,3)], 1.0);
    }
    #[test]
    fn interpolate_3d_scatter_data_mask_test() {
        let scattered_data = Matrix3xX::from_vec(vec![
            0., 0., 0.,
            1., 0., 0.,
            0., 1., 1.,
            1., 1., 1.
            ]).transpose();

        let x_interp = linspace(-0.5, 1., 4.).unwrap();
        let y_interp = linspace(-0.5, 1., 4.).unwrap();
        let (_, mask) = interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).unwrap();

        assert_abs_diff_eq!(mask[(0,0)],0.);
        assert_abs_diff_eq!(mask[(0,1)],0.);
        assert_abs_diff_eq!(mask[(0,2)],0.);
        assert_abs_diff_eq!(mask[(0,3)],0.);
        assert_abs_diff_eq!(mask[(1,0)],0.);
        assert_abs_diff_eq!(mask[(1,1)], 1.);
        assert_abs_diff_eq!(mask[(1,2)], 1.);
        assert_abs_diff_eq!(mask[(1,3)], 1.);
        assert_abs_diff_eq!(mask[(2,0)],0.);
        assert_abs_diff_eq!(mask[(2,1)], 1.);
        assert_abs_diff_eq!(mask[(2,2)], 1.);
        assert_abs_diff_eq!(mask[(2,3)], 1.);
        assert_abs_diff_eq!(mask[(3,0)],0.);
        assert_abs_diff_eq!(mask[(3,1)], 1.0);
        assert_abs_diff_eq!(mask[(3,2)], 1.0);
        assert_abs_diff_eq!(mask[(3,3)], 1.0);
    }

    #[test]
    fn interpolate_3d_scatter_data_amount_data_test() {
        let scattered_data = Matrix3xX::from_vec(vec![
            0., 0., 0.,
            1., 0., 0.,
            0., 1., 1.,
            1., 1., 1.
            ]).transpose();
        let x_interp = linspace(-0.5, 1., 4.).unwrap();
        let y_interp = linspace(-0.5, 1., 4.).unwrap();
        assert!(interpolate_3d_scatter_data(&scattered_data, &DVector::from_vec(vec![0.]), &y_interp).is_err());
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &DVector::from_vec(vec![0.])).is_err());
        assert!(interpolate_3d_scatter_data(&scattered_data, &DVector::from_vec(vec![0.]), &DVector::from_vec(vec![0.])).is_err());

        let scattered_data = Matrix3xX::from_vec(vec![
            0., 0., 0.,
            1., 0., 0.,
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
    }
    #[test]
    fn interpolate_3d_scatter_data_finite_values_test() {
        let x_interp = linspace(-0.5, 1., 4.).unwrap();
        let y_interp = linspace(-0.5, 1., 4.).unwrap();

        
        let scattered_data = Matrix3xX::from_vec(vec![
            -1., 0., f64::NAN,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data = Matrix3xX::from_vec(vec![
            -1., 0., f64::NEG_INFINITY,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data = Matrix3xX::from_vec(vec![
            -1., 0., f64::INFINITY,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data = Matrix3xX::from_vec(vec![
            -1., 0., -1.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

    }
    #[test]
    fn interpolate_3d_scatter_data_finite_coordinates_test() {
        let x_interp = linspace(-0.5, 1., 4.).unwrap();
        let y_interp = linspace(-0.5, 1., 4.).unwrap();
        let scattered_data = Matrix3xX::from_vec(vec![
            f64::NAN, 0., 0.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data = Matrix3xX::from_vec(vec![
            f64::NEG_INFINITY, 0., 0.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data = Matrix3xX::from_vec(vec![
            f64::INFINITY, 0., 0.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());

        let scattered_data = Matrix3xX::from_vec(vec![
            -1., 0., 0.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();

        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data = Matrix3xX::from_vec(vec![
            0., 0., 0.,
            1., 0., 0.,
            0., 1., 0.
            ]).transpose();
            
        assert!(interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());
    }
    #[test]
    fn interpolate_3d_triangulated_scatter_data_test() {
        todo!()
    }
    #[test]
    fn meshgrid_value_test() {
        let x = linspace(1., 3., 3.).unwrap();
        let y = linspace(4., 5., 2.).unwrap();

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
        let x = linspace(1., 3., 3.).unwrap();
        let y = linspace(4., 5., 2.).unwrap();

        let (xx, yy) = meshgrid(&x, &y).unwrap();
        assert_eq!(xx.shape(), (2, 3));
        assert_eq!(yy.shape(), (2, 3));
        let (yy, xx) = meshgrid(&y, &x).unwrap();
        assert_eq!(xx.shape(), (3, 2));
        assert_eq!(yy.shape(), (3, 2));

    }
    #[test]
    fn linspace_test() {
        let x = linspace(1., 3., 3.).unwrap();
        assert_eq!(x.len(), 3);
        assert_abs_diff_eq!(x[0], 1.);
        assert_abs_diff_eq!(x[1], 2.);
        assert_abs_diff_eq!(x[2], 3.);
        assert!(linspace(1., 3., -3.).is_err());

        assert!(linspace(1., f64::NAN, 3.).is_err());
        assert!(linspace(f64::NAN, 3., 3.).is_err());
        assert!(linspace(f64::INFINITY, 3., 3.).is_err());
        assert!(linspace(f64::NEG_INFINITY, 3., 3.).is_err());
        assert!(linspace(1., f64::NEG_INFINITY, 3.).is_err());
        assert!(linspace(1., f64::INFINITY, 3.).is_err());
        assert!(linspace(1., 10., f64::INFINITY).is_err());
        assert!(linspace(1., 10., f64::NEG_INFINITY).is_err());
        assert!(linspace(1., 10., f64::NAN).is_err());
    }
    #[test]
    fn create_linspace_axes_test() {
        let x_dat = DVector::from_vec(vec![0.,-3.,10.,50.]);
        let num_axes_points = 100.;
        let (x, xlim) = create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).unwrap();
        assert_eq!(x.len(), 100);
        assert_abs_diff_eq!(xlim.min, -3.);
        assert_abs_diff_eq!(xlim.max, 50.);
        assert_abs_diff_eq!(xlim.min, x[0]);
        assert_abs_diff_eq!(xlim.max, x[99]);

        let x_dat = DVector::from_vec(vec![0.,-3.,10.,f64::INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0.,-3.,10.,f64::NEG_INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0.,-3.,10.,f64::NAN]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_ok());
        let x_dat = DVector::from_vec(vec![0.,0.,f64::NAN,f64::INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0.,f64::NAN,f64::INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0.,0.]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0.,f64::NAN]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0.,f64::INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![0.,f64::NEG_INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());
        let x_dat = DVector::from_vec(vec![f64::NAN, f64::NAN, f64::NAN]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), num_axes_points).is_err());

        let x_dat = DVector::from_vec(vec![0.,-3.,10.,f64::INFINITY]);
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), 0.).is_err());
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), f64::NAN).is_err());
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), f64::INFINITY).is_err());
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), f64::NEG_INFINITY).is_err());
        assert!(create_linspace_axes(DVectorSlice::from(&x_dat), -1.).is_err());

    }
    #[test]
    fn create_voronoi_cells_test() {

        todo!();
        //points on same line test!

        let xy_coord = MatrixXx2::<f64>::zeros(0);
        let x_bounds = AxLims { min: 0., max: 0. };
        let y_bounds = AxLims { min: 0., max: 0. };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());

        let xy_coord = Matrix2xX::from_vec(vec![1.0, 1.5]).transpose();
        let x_bounds = AxLims { min: 0., max: 2. };
        let y_bounds = AxLims { min: 0., max: 2. };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_ok());

        let unwrapped_voronoi = voronoi.unwrap();
        assert_relative_eq!(unwrapped_voronoi.sites[0].x, 1.0);
        assert_relative_eq!(unwrapped_voronoi.sites[0].y, 1.5);

        let xy_coord = MatrixXx2::<f64>::zeros(0);
        let x_bounds = AxLims {
            min: f64::NAN,
            max: 0.,
        };
        let y_bounds = AxLims {
            min: f64::NAN,
            max: 0.,
        };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());

        let xy_coord = MatrixXx2::<f64>::zeros(0);
        let x_bounds = AxLims {
            min: f64::INFINITY,
            max: 0.,
        };
        let y_bounds = AxLims { min: 0., max: 0. };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());

        let xy_coord = MatrixXx2::<f64>::zeros(0);
        let x_bounds = AxLims {
            min: f64::NEG_INFINITY,
            max: 0.,
        };
        let y_bounds = AxLims { min: 0., max: 0. };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());

        let xy_coord = Matrix2xX::from_vec(vec![1.0, f64::NAN]).transpose();
        let x_bounds = AxLims { min: 0., max: 2. };
        let y_bounds = AxLims { min: 0., max: 2. };
        let voronoi = create_voronoi_cells(&xy_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());
    }
    #[test]
    fn create_valued_voronoi_cells_test() {
        let xyz_coord = MatrixXx3::<f64>::zeros(0);
        let x_bounds = AxLims { min: 0., max: 0. };
        let y_bounds = AxLims { min: 0., max: 0. };
        let voronoi = create_valued_voronoi_cells(&xyz_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_err());

        let xyz_coord = Matrix3xX::from_vec(vec![1.0, 1.5, 10.]).transpose();
        let x_bounds = AxLims { min: 0., max: 2. };
        let y_bounds = AxLims { min: 0., max: 2. };
        let voronoi = create_valued_voronoi_cells(&xyz_coord, &x_bounds, &y_bounds);
        assert!(voronoi.is_ok());

        let unwrapped_voronoi = voronoi.unwrap();
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[0].x, 1.0);
        assert_relative_eq!(unwrapped_voronoi.voronoi_diagram.sites[0].y, 1.5);
        assert_relative_eq!(unwrapped_voronoi.z_data[0], 10.);
    }

}
