use nalgebra::{Point2, Point3, Vector3};
use num::Zero;
use opossum::{
    distributions::Hexapolar,
    error::OpmResult,
    nodes::WaveFrontErrorMap,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotType},
    ray::Ray,
    rays::Rays,
};
use rayon::vec;
use std::time::{Duration, Instant};
use uom::si::{
    energy::{joule, Energy},
    f64::Length,
    length::{centimeter, millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let mut rays = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1053.),
        Energy::new::<joule>(1.),
        &Hexapolar::new(Length::new::<millimeter>(10.), 5)?,
    )?;
    rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
    rays.propagate_along_z()?;
    rays.refract_paraxial(Length::new::<millimeter>(10.))?;
    rays.set_dist_to_next_surface(Length::new::<millimeter>(30.));
    rays.propagate_along_z()?;
    rays.refract_paraxial(Length::new::<millimeter>(20.))?;
    rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
    rays.propagate_along_z()?;

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("ray_fluence_test_random.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    // let start: Instant = Instant::now();
    let fluence_data = rays.calc_transversal_fluence(
        None,
        Some(Point3::new(Length::zero(), Length::zero(), Length::zero())),
    )?;
    // let duration = start.elapsed();
    // println!("{duration:?}");
    println!("{}", fluence_data.get_peak_fluence());
    println!("{}", fluence_data.get_average_fluence());

    let plt_dat = PlotData::ColorMesh(
        fluence_data.interp_x_data,
        fluence_data.interp_y_data,
        fluence_data.interp_distribution,
    );
    let plt_type = PlotType::ColorMesh(plt_params);
    let _ = plt_type.plot(&plt_dat);

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("spot_ray_fluence_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    let plt_dat = PlotData::Dim2(rays.get_xy_rays_pos());
    let plt_type = PlotType::Scatter2D(plt_params);
    let _ = plt_type.plot(&plt_dat);

    Ok(())
}
