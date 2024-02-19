use nalgebra::{Point2, Point3, Vector3};
use num::Zero;
use opossum::{
    distribution::DistributionStrategy,
    error::OpmResult, nodes::WaveFrontErrorMap, plottable::{PlotArgs, PlotData, PlotParameters, PlotType}, ray::Ray, rays::Rays
};
use rayon::vec;
use uom::si::{
    energy::{joule, Energy},
    f64::Length,
    length::{centimeter, millimeter, nanometer},
};
use std::time::{Duration, Instant};


fn main() -> OpmResult<()> {
    let mut rays = Rays::new_uniform_collimated(
        Length::new::<millimeter>(10.),
        Length::new::<nanometer>(1053.),
        Energy::new::<joule>(1.),
        &DistributionStrategy::FibonacciSquare(1000),
    )?;

    // let points = vec![Point2::new(0., 0.), Point2::new(1., 0.),Point2::new(1., 1.)];
    // let area = rays.calc_closed_poly_area(points);

    // println!("{area}");
    // return Ok(());

    let mut ray1 = Ray::new(
        Point3::new(Length::new::<centimeter>(0.), Length::zero(), Length::zero()), 
        Vector3::new(0.,0.,1.), 
        Length::new::<nanometer>(1053.), 
        Energy::new::<joule>(1.))?;
    let mut ray2 = Ray::new(
            Point3::new(Length::new::<centimeter>(5.), Length::zero(), Length::zero()), 
            Vector3::new(0.,1.,1.), 
            Length::new::<nanometer>(1053.), 
            Energy::new::<joule>(1.))?;
    // let ray3 = Ray::new(
    //     Point3::new( Length::new::<centimeter>(0.), Length::new::<centimeter>(5.), Length::zero()), 
    //     Vector3::new(0.,0.,1.), 
    //     Length::new::<nanometer>(1053.), 
    //     Energy::new::<joule>(1.))?;
    
    //     let ray4 = Ray::new(
    //         Point3::new( Length::new::<centimeter>(5.), Length::new::<centimeter>(5.), Length::zero()), 
    //         Vector3::new(0.,0.,1.), 
    //         Length::new::<nanometer>(1053.), 
    //         Energy::new::<joule>(1.))?;

    //         let ray5 = Ray::new(
    //             Point3::new( Length::new::<centimeter>(2.5), Length::new::<centimeter>(2.5), Length::zero()), 
    //             Vector3::new(0.,0.,1.), 
    //             Length::new::<nanometer>(1053.), 
    //             Energy::new::<joule>(1.))?;


    let mut rays = Rays::from(vec![ray1, ray2]);
    rays.propagate_along_z(Length::new::<millimeter>(10.));

    // WaveFrontErrorMap::new(rays.wavefront_error_at_pos_in_units_of_wvl(), wavelength)

    // println!("{}", ray1.path_length().get::<millimeter>());
    // println!("{}", ray2.path_length().get::<millimeter>());
    

    // rays.refract_paraxial(Length::new::<millimeter>(10.))?;
    // rays.propagate_along_z(Length::new::<millimeter>(30.))?;
    // rays.refract_paraxial(Length::new::<millimeter>(20.))?;
    // rays.propagate_along_z(Length::new::<millimeter>(10.))?;

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("ray_fluence_test_random.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 850)))
        .unwrap();
    // let start: Instant = Instant::now();
    let (rays_fluence, y, x, peak, average) = rays.calc_transversal_fluence(None, Some(Point3::new(Length::zero(), Length::zero(), Length::zero())))?;
    // let duration = start.elapsed();
    // println!("{duration:?}");
    println!("{}", peak);
    println!("{}", average);

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
