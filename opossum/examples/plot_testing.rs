use opossum::{
    error::OpmResult,
    joule, millimeter, nanometer,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType},
    position_distributions::Hexapolar,
    rays::Rays,
};
use plotters::style::RGBAColor;

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

    let pltdat = rays.get_xy_rays_pos(true);
    let plt_series = PlotSeries::new(
        &PlotData::new_dim2(pltdat).unwrap(),
        RGBAColor(255, 0, 0, 1.),
        None,
    );
    let plt_type = PlotType::Line2D(plt_params);
    let _ = plt_type.plot(&vec![plt_series]);

    // // rays.set_dist_to_next_surface(millimeter!(10.));
    // // rays.propagate_along_z()?;
    // // rays.refract_paraxial(millimeter!(10.))?;
    // // rays.set_dist_to_next_surface(millimeter!(30.));
    // // rays.propagate_along_z()?;
    // // rays.refract_paraxial(millimeter!(20.))?;
    // // rays.set_dist_to_next_surface(millimeter!(10.));
    // // rays.propagate_along_z()?;p

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
    let fluence_data = rays.calc_fluence_at_position()?;
    // let duration = start.elapsed();
    // println!("{duration:?}");
    println!("{}", fluence_data.get_peak_fluence());
    println!("{}", fluence_data.get_average_fluence());
    let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();
    let plt_dat = PlotData::ColorMesh {
        x_dat_n: fl_x,
        y_dat_m: fl_y,
        z_dat_nxm: fl_d,
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
