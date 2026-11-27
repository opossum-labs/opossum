//! Module for gridding data

#![warn(missing_docs)]
use super::AxLims;
use super::filter_data::filter_nan_infinite;
use crate::error::{OpmResult, OpossumError};
use itertools::Itertools;
use log::warn;
use nalgebra::{DMatrix, DVector, DVectorView, MatrixXx3, Scalar};
use num_traits::{Float, NumCast};
use spade::{DelaunayTriangulation, HasPosition, Point2 as SpadeP, Triangulation};

/// A point of scattered data in a triangulation: where it lies, its value, and its row in the
/// data.
struct ScatteredPoint {
    position: SpadeP<f64>,
    height: f64,
    row: usize,
}

impl HasPosition for ScatteredPoint {
    type Scalar = f64;

    fn position(&self) -> SpadeP<f64> {
        self.position
    }
}

/// Triangulates scattered data: x, y and a value in each row. The rows whose value is not finite
/// are left out.
///
/// # Errors
/// This function returns an error if a coordinate is not finite or if the points span no
/// triangle, being fewer than three or all on one line.
fn triangulate(
    scattered_data: &MatrixXx3<f64>,
) -> OpmResult<DelaunayTriangulation<ScatteredPoint>> {
    let mut triangulation = DelaunayTriangulation::new();
    for (row, point) in scattered_data.row_iter().enumerate() {
        let (position, height) = (SpadeP::new(point[0], point[1]), point[2]);
        if !(position.x.is_finite() && position.y.is_finite()) {
            return Err(OpossumError::Other(
                "the coordinates of scattered data must be finite".into(),
            ));
        }
        if height.is_finite() {
            triangulation
                .insert(ScatteredPoint {
                    position,
                    height,
                    row,
                })
                .map_err(|e| {
                    OpossumError::Other(format!(
                        "Inserting Point into Spade triangulation failed: {e}"
                    ))
                })?;
        }
    }
    if triangulation.num_inner_faces() == 0 {
        return Err(OpossumError::Other(
            "the scattered data span no triangle: they are fewer than three or lie on one line"
                .into(),
        ));
    }
    Ok(triangulation)
}

/// Interpolation of scattered 3d data
///
/// Interpolation of scattered 3d data (not on a regular grid), meaning a set of "x" and "y" coordinates and a value for each data point.
/// The interpolation runs over the Delaunay triangulation of the data points, by natural
/// neighbors, onto the desired points (`x_interp`, `y_interp`); points outside the hull of the data
/// stay NaN. Data points whose value is not finite are left out.
/// # Attributes
/// `scattered_data`: Scattered data to be interpolated. The data should be structures as x-column, y-column, data-column
/// `x_interp`: x-coordinates of the points on which this function should interpolate
/// `y_interp`: y-coordinates of the points on which this function should interpolate
/// # Returns
/// The interpolated values, a row per y and a column per x, and a mask that is 1 where a value
/// was interpolated and 0 elsewhere.
/// # Errors
/// This function errors if
/// - fewer than two finite interpolation points are given along an axis
/// - a coordinate of the data is not finite, or the data span no triangle
pub fn grid_interpolate_3d_scatter_data(
    scattered_data: &MatrixXx3<f64>,
    x_interp: &DVector<f64>,
    y_interp: &DVector<f64>,
) -> OpmResult<(DMatrix<f64>, DMatrix<f64>)> {
    let x_interp = filter_nan_infinite(x_interp.as_slice());
    let y_interp = filter_nan_infinite(y_interp.as_slice());
    if x_interp.len() < 2 || y_interp.len() < 2 {
        return Err(OpossumError::Other(
            "Length of interpolation ranges must be larger than 1 to define the interpolation bounds".into(),
        ));
    }
    let triangulation = triangulate(scattered_data)?;
    let interpolation = triangulation.natural_neighbor();
    let values = DMatrix::from_fn(y_interp.len(), x_interp.len(), |row, column| {
        match (x_interp.get(column), y_interp.get(row)) {
            (Some(x), Some(y)) => interpolation
                .interpolate(|v| v.data().height, SpadeP::new(*x, *y))
                .unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    });
    let mask = values.map(|value| if value.is_nan() { 0. } else { 1. });
    Ok((values, mask))
}

/// Returns the Delaunay triangles of scattered data: a row per triangle, holding the rows of the
/// data at its three corners, counterclockwise. Data points whose value is not finite are left
/// out.
///
/// # Errors
/// This function errors if a coordinate of the data is not finite, or if the data span no
/// triangle.
pub fn triangulate_scatter_data(scattered_data: &MatrixXx3<f64>) -> OpmResult<MatrixXx3<usize>> {
    let corners = triangulate(scattered_data)?
        .inner_faces()
        .flat_map(|face| face.vertices().map(|vertex| vertex.data().row))
        .collect_vec();
    Ok(MatrixXx3::from_row_slice(&corners))
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
/// This function will return an error if
///  - `start` or `end` are not finite
///  - `num` cannot be casted to float.
///
/// # Panics
/// This function panics if step cannot be casted from usize to `T`.
pub fn linspace<T: Float + Scalar>(start: T, end: T, num: usize) -> OpmResult<DVector<T>> {
    if !start.is_finite() || !end.is_finite() {
        return Err(OpossumError::Other(
            "start and end values must be finite!".into(),
        ));
    }
    let mut linspace = DVector::<T>::from_element(num, start);
    if num < 2 {
        warn!(
            "Using linspace with less than two elements results in an empty Vector for num=0 or a Vector with one entry being num=start"
        );
        return Ok(linspace);
    }
    let bin_size = (end - start)
        / NumCast::from(num - 1)
            .ok_or_else(|| OpossumError::Other("Cannot cast `usize` to float type!".into()))?;
    for (step, val) in linspace.iter_mut().enumerate() {
        *val = *val
            + <T as NumCast>::from(step)
                .ok_or_else(|| OpossumError::Other("Error in num cast".to_string()))?
                * bin_size;
    }
    Ok(linspace)
}
/// Creates a linearly spaced Vector (Matrix with1 column and `num` rows) from `start` to `end` and an [`AxLims`] struct from data.
/// # Attributes
/// - `data`: data that defines the start- and end-points of the linearly spaced vector
/// - `num_axes_points`: number of points
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

#[cfg(test)]
mod test {
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::Matrix3xX;

    use super::*;
    #[test]
    fn interpolate_3d_scatter_data_value_test() -> OpmResult<()> {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();

        let x_interp = linspace(-0.5, 1., 4)?;
        let y_interp = linspace(-0.5, 1., 4)?;
        let (interp_data, _) =
            grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)?;

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
        Ok(())
    }
    /// Inside the hull of its data, natural neighbor interpolation shows a linear function as it
    /// is, however the data are scattered.
    #[test]
    fn interpolation_shows_a_linear_function_as_it_is() -> OpmResult<()> {
        let linear = |x: f64, y: f64| 2.0f64.mul_add(x, (-3.0f64).mul_add(y, 1.0));
        let points = [
            (0.0, 0.0),
            (3.0, 0.5),
            (1.0, 2.5),
            (-1.0, 1.5),
            (2.0, 3.0),
            (0.5, 1.0),
            (-0.5, 3.5),
            (2.5, 2.0),
            (1.5, 0.2),
            (0.2, 2.0),
        ];
        let scattered_data = MatrixXx3::from_row_iterator(
            points.len(),
            points.iter().flat_map(|(x, y)| [*x, *y, linear(*x, *y)]),
        );
        let (x_interp, y_interp) = (linspace(-1., 3., 9)?, linspace(0., 3.5, 8)?);
        let (values, mask) =
            grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)?;
        let mut inside = 0;
        for (column, x) in x_interp.iter().enumerate() {
            for (row, y) in y_interp.iter().enumerate() {
                if mask[(row, column)] > 0.0 {
                    assert_abs_diff_eq!(values[(row, column)], linear(*x, *y), epsilon = 1e-12);
                    inside += 1;
                }
            }
        }
        assert!(inside > 20, "only {inside} points inside");
        Ok(())
    }
    #[test]
    fn interpolate_3d_scatter_data_mask_test() -> OpmResult<()> {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();
        println!("{}", scattered_data);
        let x_interp = linspace(-0.5, 1., 4)?;
        let y_interp = linspace(-0.5, 1., 4)?;
        let (_data, mask) =
            grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)?;

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
        Ok(())
    }

    #[test]
    fn interpolate_3d_scatter_data_amount_data_test() -> OpmResult<()> {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.]).transpose();
        let x_interp = linspace(-0.5, 1., 4)?;
        let y_interp = linspace(-0.5, 1., 4)?;
        assert!(
            grid_interpolate_3d_scatter_data(
                &scattered_data,
                &DVector::from_vec(vec![0.]),
                &y_interp
            )
            .is_err()
        );
        assert!(
            grid_interpolate_3d_scatter_data(
                &scattered_data,
                &x_interp,
                &DVector::from_vec(vec![0.])
            )
            .is_err()
        );
        assert!(
            grid_interpolate_3d_scatter_data(
                &scattered_data,
                &DVector::from_vec(vec![0.]),
                &DVector::from_vec(vec![0.])
            )
            .is_err()
        );

        let scattered_data = Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0.]).transpose();
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        Ok(())
    }
    /// A data point without a finite value is left out; the others must still span a triangle.
    #[test]
    fn interpolate_3d_scatter_data_finite_values_test() -> OpmResult<()> {
        let x_interp = linspace(-0.5, 1., 4)?;
        let y_interp = linspace(-0.5, 1., 4)?;
        for value in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
            let scattered_data =
                Matrix3xX::from_vec(vec![-1., 0., value, 1., 0., 0., 0., 1., 0., 0., -1., 0.])
                    .transpose();
            let (values, _) =
                grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)?;
            assert!(values.iter().filter(|v| !v.is_nan()).all(|v| *v == 0.0));
            let scattered_data =
                Matrix3xX::from_vec(vec![-1., 0., value, 1., 0., 0., 0., 1., 0.]).transpose();
            assert!(
                grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err()
            );
        }
        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., -1., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());
        Ok(())
    }
    #[test]
    fn interpolate_3d_scatter_data_finite_coordinates_test() -> OpmResult<()> {
        let x_interp = linspace(-0.5, 1., 4)?;
        let y_interp = linspace(-0.5, 1., 4)?;
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::NAN, 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::NEG_INFINITY, 0., 0., 1., 0., 0., 0., 1., 0.])
                .transpose();
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());
        let scattered_data =
            Matrix3xX::from_vec(vec![f64::INFINITY, 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_err());

        let scattered_data =
            Matrix3xX::from_vec(vec![-1., 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();

        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());

        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 0., 0., 0., 1., 0.]).transpose();

        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp).is_ok());
        Ok(())
    }
    #[test]
    fn meshgrid_value_test() -> OpmResult<()> {
        let x = linspace(1., 3., 3)?;
        let y = linspace(4., 5., 2)?;

        let (yy, xx) = meshgrid(&y, &x)?;

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
        Ok(())
    }
    #[test]
    fn meshgrid_shape_test() -> OpmResult<()> {
        let x = linspace(1., 3., 3)?;
        let y = linspace(4., 5., 2)?;

        let (xx, yy) = meshgrid(&x, &y)?;
        assert_eq!(xx.shape(), (2, 3));
        assert_eq!(yy.shape(), (2, 3));
        let (yy, xx) = meshgrid(&y, &x)?;
        assert_eq!(xx.shape(), (3, 2));
        assert_eq!(yy.shape(), (3, 2));
        Ok(())
    }
    #[test]
    fn linspace_test() -> OpmResult<()> {
        let x = linspace(1., 3., 3)?;
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
        Ok(())
    }
    #[test]
    fn create_linspace_axes_test() -> OpmResult<()> {
        let x_dat = DVector::from_vec(vec![0., -3., 10., 50.]);
        let num_axes_points = 100;
        let (x, xlim) = create_linspace_axes(DVectorView::from(&x_dat), num_axes_points)?;
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
        Ok(())
    }
    /// The triangles of scattered data hold the rows of their corners, counterclockwise; a row
    /// without a finite value is no corner.
    #[test]
    fn triangles_hold_the_rows_of_their_corners() -> OpmResult<()> {
        let scattered_data = Matrix3xX::from_vec(vec![
            0.,
            0.,
            1., //
            5.,
            5.,
            f64::NAN, //
            1.,
            0.,
            2., //
            0.,
            1.,
            3.,
        ])
        .transpose();
        let triangles = triangulate_scatter_data(&scattered_data)?;
        assert_eq!(triangles.nrows(), 1);
        // rotated to start at its lowest row, the triangle runs from row 0 over 2 to 3
        let corners = triangles.row(0).iter().copied().collect_vec();
        let start = corners.iter().position_min().unwrap_or(0);
        let rotated = [0, 1, 2].map(|i| corners[(start + i) % 3]);
        assert_eq!(rotated, [0, 2, 3]);
        Ok(())
    }
    /// Data on one line span no triangle.
    #[test]
    fn data_on_one_line_span_no_triangle() {
        let scattered_data =
            Matrix3xX::from_vec(vec![0., 0., 0., 1., 1., 0., 2., 2., 0., 1000., 1000., 0.])
                .transpose();
        assert!(triangulate_scatter_data(&scattered_data).is_err());
        let x_interp = DVector::from_vec(vec![0., 1.]);
        assert!(grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &x_interp).is_err());
    }
}
