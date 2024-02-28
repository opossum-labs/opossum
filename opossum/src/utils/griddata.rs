use kahan::KahanSum;
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
    let x_bounds = AxLims::new(x_interp.min(), x_interp.max())?;
    let y_bounds = AxLims::new(y_interp.min(), y_interp.max())?;
    let voronoi_data = create_valued_voronoi_cells(scattered_data, &x_bounds, &y_bounds)?;

    interpolate_3d_triangulated_scatter_data(&voronoi_data, x_interp, y_interp)
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
    let ax_lim = AxLims::new(data.min(), data.max())?;
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
) -> Option<VoronoiDiagram<VPoint>> {
    //collect data to a vector of Points that can be used to create the triangulation
    let points: Vec<VPoint> = xy_coord
        .row_iter()
        .map(|c| VPoint::from_xy(c[0], c[1]))
        .collect();

    //create the voronoi diagram with the minimum and maximum values of the axes as bounds
    VoronoiDiagram::<VPoint>::new(
        &VPoint::from_xy(x_bounds.min, y_bounds.min),
        &VPoint::from_xy(x_bounds.max, y_bounds.max),
        &points,
    )
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
    create_voronoi_cells(
        &MatrixXx2::from_columns(&[xyz_data.column(0), xyz_data.column(1)]),
        x_bounds,
        y_bounds,
    )
    .map_or_else(
        || {
            Err(OpossumError::Other(
                "Could not create Voronoi diagram! Interpolation not possible".into(),
            ))
        },
        |voronoi| {
            Ok(VoronoiData::new(
                voronoi,
                DVector::from_column_slice(xyz_data.column(2).as_slice()),
            ))
        },
    )
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
    let mut mask = DMatrix::from_element(num_axes_points_y, num_axes_points_x, 0.);

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
                        && cross_3.is_sign_positive());

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
