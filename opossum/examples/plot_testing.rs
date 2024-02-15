use nalgebra::Vector3;
use opossum::{
    distribution::DistributionStrategy,
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotType},
    rays::Rays,
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
        &DistributionStrategy::Hexapolar(5),
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
        .set(&PlotArgs::FName("ray_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1500, 1500)))
        .unwrap();
    let ray_pos_hist = rays.get_rays_position_history_in_mm();

    let plt_dat = PlotData::MultiDim2(ray_pos_hist.project_to_plane(Vector3::new(1., 0., 0.))?);
    let plt_type = PlotType::MultiLine2D(plt_params);
    let _ = plt_type.plot(&plt_dat);

    Ok(())
}
