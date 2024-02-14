use nalgebra::{Point3, Vector3};
use num::Zero;
use opossum::{
    error::OpmResult, plottable::{PlotArgs, PlotData, PlotParameters, PlotType}, ray::Ray, rays::{DistributionStrategy, Rays}
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
        &DistributionStrategy::Hexapolar(8),
    )?;

    let ray1 = Ray::new(
        Point3::new(Length::new::<millimeter>(-5.), Length::zero(), Length::zero()), 
        Vector3::new(0.,0.,1.), 
        Length::new::<nanometer>(1053.), 
        Energy::new::<joule>(0.5))?;
    let ray2 = Ray::new(
            Point3::new(Length::new::<millimeter>(5.), Length::zero(), Length::zero()), 
            Vector3::new(0.,0.,1.), 
            Length::new::<nanometer>(1053.), 
            Energy::new::<joule>(0.5))?;
    let ray3 = Ray::new(
        Point3::new( Length::zero(), Length::new::<millimeter>(10.), Length::zero()), 
        Vector3::new(0.,0.,1.), 
        Length::new::<nanometer>(1053.), 
        Energy::new::<joule>(1.))?;

    println!("{}", rays.nr_of_rays());

    let rays = Rays::from(vec![ray1, ray2, ray3]);

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
    let (rays_fluence, y, x, peak) = rays.calc_transversal_fluence(None, None)?;
    println!("{}", peak);

    let plt_dat = PlotData::ColorMesh(y, x, rays_fluence);
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
