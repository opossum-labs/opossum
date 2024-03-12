use opossum::{
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType},
    position_distributions::{FibonacciEllipse, FibonacciRectangle},
    rays::Rays,
};
use plotters::style::RGBAColor;
use uom::si::{
    energy::{joule, Energy},
    f64::Length,
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let rays = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1053.),
        Energy::new::<joule>(1.),
        &FibonacciRectangle::new(
            Length::new::<millimeter>(1.),
            Length::new::<millimeter>(1.),
            1000,
        )?,
    )?;
    // rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
    // rays.propagate_along_z()?;
    // rays.refract_paraxial(Length::new::<millimeter>(10.))?;
    // rays.set_dist_to_next_surface(Length::new::<millimeter>(30.));
    // rays.propagate_along_z()?;
    // rays.refract_paraxial(Length::new::<millimeter>(20.))?;
    // rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
    // rays.propagate_along_z()?;

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("ray_fluence_test_random.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    // let start: Instant = Instant::now();
    let fluence_data = rays.calc_fluence_at_position()?;
    // let duration = start.elapsed();
    // println!("{duration:?}");
    println!("{}", fluence_data.get_peak_fluence());
    println!("{}", fluence_data.get_average_fluence());
    let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();
    let plt_dat = PlotData::ColorMesh(fl_x, fl_y, fl_d);
    let plt_series = PlotSeries::new(&plt_dat, RGBAColor(0,0,0,0.), None);
    let plt_type = PlotType::ColorMesh(plt_params);
    let _ = plt_type.plot(&vec![plt_series]);

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("spot_ray_fluence_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    // let plt_dat = PlotData::Dim2(rays.get_xy_rays_pos(true));
    // let plt_type = PlotType::Scatter2D(plt_params);
    // let _ = plt_type.plot(&plt_dat);

    Ok(())
}
