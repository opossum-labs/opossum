use delaunator::{triangulate, Point};
use itertools::izip;
use nalgebra::{DMatrix, DVector, Matrix1xX, Matrix3xX, MatrixXx3};
use opossum::{
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotType, PltBackEnd},
};
use std::f64::{self, consts::PI};
use voronator::delaunator::Point as v_point;
use voronator::VoronoiDiagram;

fn linspace(start: f64, end: f64, num: usize) -> Matrix1xX<f64> {
    let mut linspace = Matrix1xX::<f64>::from_element(num, start);
    let bin_size = (end - start) / (num - 1) as f64;
    for (i, val) in linspace.iter_mut().enumerate() {
        *val += bin_size * i as f64
    }
    linspace
}

fn linspace_u8(start: u8, step: u8, num: u8) -> Matrix1xX<u8> {
    let u8_iter = (start..(start + step * num))
        .step_by(step as usize)
        .into_iter();
    Matrix1xX::<u8>::from_iterator(num as usize, u8_iter)
}

fn meshgrid_u8(x: &Matrix1xX<u8>, y: &Matrix1xX<u8>) -> (DMatrix<u8>, DMatrix<u8>) {
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<u8>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<u8>::zeros(y_len, x_len);

    for x_id in 0..x_len {
        for y_id in 0..y_len {
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    }

    (x_mat, y_mat)
}
fn meshgrid(x: &Matrix1xX<f64>, y: &Matrix1xX<f64>) -> (DMatrix<f64>, DMatrix<f64>) {
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<f64>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<f64>::zeros(y_len, x_len);

    for x_id in 0..x_len {
        for y_id in 0..y_len {
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    }

    (x_mat, y_mat)
}

fn main() -> OpmResult<()> {
    //triangulation test
    let dat = Matrix3xX::from_vec(vec![
        0.,
        0.,
        1.,
        f64::cos(2. * PI / 6. * 0.),
        f64::sin(2. * PI / 6. * 0.),
        0.,
        f64::cos(2. * PI / 6. * 1.),
        f64::sin(2. * PI / 6. * 1.),
        0.,
        f64::cos(2. * PI / 6. * 2.),
        f64::sin(2. * PI / 6. * 2.),
        0.,
        f64::cos(2. * PI / 6. * 3.),
        f64::sin(2. * PI / 6. * 3.),
        0.,
        f64::cos(2. * PI / 6. * 4.),
        f64::sin(2. * PI / 6. * 4.),
        0.,
        f64::cos(2. * PI / 6. * 5.),
        f64::sin(2. * PI / 6. * 5.),
        0.,
    ])
    .transpose();

    let mut plt_params = PlotParameters::default();

    plt_params
        .set(&PlotArgs::FName("triangle_test.png".into()))
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .set(&PlotArgs::FigSize((800, 1000)));

    plt_params.set(&PlotArgs::Backend(PltBackEnd::BMP));

    let points: Vec<Point> = dat
        .row_iter()
        .map(|c| Point { x: c[0], y: c[1] })
        .collect::<Vec<Point>>();

    let trianglulation = triangulate(&points);
    let triangles = trianglulation.triangles;

    let tri_index_mat = Matrix3xX::from_vec(triangles).transpose();

    let mut triangle_centroid_z = DVector::<f64>::zeros(tri_index_mat.column(0).len());

    for (c, t) in izip!(tri_index_mat.row_iter(), triangle_centroid_z.iter_mut()) {
        *t = (dat[(c[0], 2)] + dat[(c[1], 2)] + dat[(c[2], 2)]) / 3.;
    }

    let (plt_dat, plt_type) = (
        PlotData::TriangulatedSurface(tri_index_mat, dat.clone()),
        PlotType::TriangulatedSurface(plt_params),
    );
    let _ = plt_type.plot(&plt_dat);

    //colormesh test
    let x = linspace(-50., 50., 101);
    let y = linspace(-50., 50., 101);
    let sigma = 5.;

    let (xx, yy) = meshgrid(&x, &y);
    let gaussian = (-0.5 * (xx.map(|x| x.powi(2)) + yy.map(|y| (y - 10.).powi(2)))
        / f64::powi(sigma, 2))
    .map(|x| x.exp())
        * 2.;

    let flat_x = DVector::from_vec(xx.iter().cloned().map(|x| x).collect::<Vec<f64>>());
    let flat_y = DVector::from_vec(yy.iter().cloned().map(|x| x).collect::<Vec<f64>>());
    let flat_z = DVector::from_vec(gaussian.iter().cloned().map(|x| x).collect::<Vec<f64>>());

    let mat3d = MatrixXx3::from_columns(&[flat_x, flat_y, flat_z]);

    let plt_dat_origin = PlotData::ColorMesh(x.transpose(), y.transpose(), gaussian.clone());
    let plt_dat_binned = bin_2d_scatter_data(&PlotData::Dim3(mat3d)).unwrap();

    let mut p_info_params = PlotParameters::default();
    p_info_params
        .set(&PlotArgs::Backend(PltBackEnd::BMP))
        .set(&PlotArgs::FName("pre_bin.png".into()))
        .set(&PlotArgs::FDir("./opossum/playground/".into()));

    let plt_type = PlotType::ColorMesh(p_info_params.clone());
    plt_type.plot(&plt_dat_origin)?;

    p_info_params.set(&PlotArgs::FName("post_bin.png".into()));
    let plt_type = PlotType::ColorMesh(p_info_params.clone());
    plt_type.plot(&plt_dat_binned)?;

    Ok(())
}

fn bin_2d_scatter_data(plt_dat: &PlotData) -> Option<PlotData> {
    if let PlotData::Dim3(dat) = plt_dat {
        let (x_range, x_min, x_max) = plt_dat.get_min_max_range(&dat.column(0));
        let (y_range, y_min, y_max) = plt_dat.get_min_max_range(&dat.column(1));

        let num_entries = dat.column(0).len();
        let mut num = f64::sqrt(num_entries as f64 / 2.).floor();

        if (num as i32) % 2 == 0 {
            num += 1.;
        }

        let xbin = x_range / (num - 1.0);
        let ybin = y_range / (num - 1.0);

        let x = linspace(x_min - xbin / 2., x_max + xbin / 2., num as usize);
        let y = linspace(y_min - ybin / 2., y_max + ybin / 2., num as usize);

        let xbin = x[1] - x[0];
        let ybin = y[1] - y[0];
        let x_min = x.min();
        let y_min = y.min();

        let (xx, yy) = meshgrid(&x, &y);

        let mut zz = xx.clone() * 0.;
        let mut zz_counter = xx.clone() * 0.;

        for row in dat.row_iter() {
            let x_index = ((row[(0, 0)] - x_min + xbin / 2.) / xbin) as usize;
            let y_index = ((row[(0, 1)] - y_min + ybin / 2.) / ybin) as usize;
            zz[(y_index, x_index)] += row[(0, 2)];
            zz_counter[(y_index, x_index)] += 1.;
        }
        for (z, z_count) in izip!(zz.iter_mut(), zz_counter.iter()) {
            if *z_count > 0.5 {
                *z /= *z_count;
            }
        }

        Some(PlotData::ColorMesh(x.transpose(), y.transpose(), zz))
    } else {
        None
    }
}
