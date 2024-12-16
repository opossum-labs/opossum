use std::time::Instant;

use nalgebra::{DMatrix, DVector, MatrixXx2};
use opossum::{
    error::OpmResult,
    joule, millimeter, nanometer,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType},
    position_distributions::Hexapolar,
    rays::Rays,
    utils::geom_transformation::Isometry,
};
use plotters::style::RGBAColor;
use uom::si::{length::millimeter, radiant_exposure::joule_per_square_centimeter};
fn main() -> OpmResult<()> {
    let rays = Rays::new_uniform_collimated(
        nanometer!(1053.),
        joule!(1.),
        &Hexapolar::new(millimeter!(1.), 20)?,
    )?;

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("equal axis test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::PlotSize((1000, 1000)))
        .unwrap();

    let pltdat_uom = rays.get_xy_rays_pos(true, &Isometry::identity());
    let pltdat = MatrixXx2::from_iterator(
        pltdat_uom.nrows(),
        pltdat_uom
            .iter()
            .map(uom::si::f64::Length::get::<millimeter>),
    );

    let plt_series = PlotSeries::new(
        &PlotData::new_dim2(pltdat).unwrap(),
        RGBAColor(255, 0, 0, 1.),
        None,
    );
    let plt_type = PlotType::Line2D(plt_params);
    let start = Instant::now();
    let _ = plt_type.plot(&vec![plt_series]);
    println!("Time elapsed in plotting: {:?}", start.elapsed());

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("ray_fluence_test_random.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::ExpandBounds(false))
        .unwrap()
        .set(&PlotArgs::PlotSize((800, 800)))
        .unwrap();
    // let start: Instant = Instant::now();
    let fluence_data = rays.calc_fluence_at_position(&Isometry::identity())?;
    // let duration = start.elapsed();
    // println!("{duration:?}");
    println!("{:?}", fluence_data.peak());
    // println!("{:?}", fluence_data.get_average_fluence());
    let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();

    let plt_dat = PlotData::ColorMesh {
        x_dat_n: DVector::from_iterator(
            fl_x.len(),
            fl_x.iter().map(uom::si::f64::Length::get::<millimeter>),
        ),
        y_dat_m: DVector::from_iterator(
            fl_y.len(),
            fl_y.iter().map(uom::si::f64::Length::get::<millimeter>),
        ),
        z_dat_nxm: DMatrix::from_iterator(
            fluence_data.len_y(),
            fluence_data.len_x(),
            fl_d.iter()
                .map(uom::si::f64::RadiantExposure::get::<joule_per_square_centimeter>),
        ),
    };

    let plt_series = PlotSeries::new(&plt_dat, RGBAColor(0, 0, 0, 0.), None);
    let plt_type = PlotType::ColorMesh(plt_params);
    let _ = plt_type.plot(&vec![plt_series]);

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("spot_ray_fluence_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::PlotSize((1000, 850)))
        .unwrap();
    // let plt_dat = PlotData::Dim2(rays.get_xy_rays_pos(true));
    // let plt_type = PlotType::Scatter2D(plt_params);
    // let _ = plt_type.plot(&plt_dat);

    Ok(())
}
