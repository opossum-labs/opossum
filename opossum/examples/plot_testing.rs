use nalgebra::Vector3;
use opossum::{
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotType},
    rays::{DistributionStrategy, Rays},
};
use uom::si::{
    energy::{joule, Energy},
    f64::Length,
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let mut rays = Rays::new_uniform_collimated(
        Length::new::<millimeter>(10.),
        Length::new::<nanometer>(1053.),
        Energy::new::<joule>(1.),
        &DistributionStrategy::Fibonacci(100000),
    )?;

    println!("{}", rays.nr_of_rays());

    // rays.propagate_along_z(Length::new::<millimeter>(10.))?;
    // rays.refract_paraxial(Length::new::<millimeter>(10.))?;
    // rays.propagate_along_z(Length::new::<millimeter>(30.))?;
    // rays.refract_paraxial(Length::new::<millimeter>(20.))?;
    // rays.propagate_along_z(Length::new::<millimeter>(10.))?;

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("ray_fluence_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    let (rays_fluence, y, x, _) = rays.calc_transversal_fluence(None, None)?;

    let plt_dat = PlotData::ColorMesh(y, x, rays_fluence);
    let plt_type = PlotType::ColorMesh(plt_params);
    let _ = plt_type.plot(&plt_dat);

    // let plt_dat = PlotData::Dim2(rays.get_xy_rays_pos());
    // let plt_type = PlotType::Scatter2D(plt_params);
    // let _ = plt_type.plot(&plt_dat);


    Ok(())
}
