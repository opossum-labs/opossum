use nalgebra::{Point2, Vector3};
use opossum::{error::OpmResult, rays::Ray};
use uom::num_traits::Zero;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let ray = Ray::new(
        Point2::new(Length::zero(), Length::new::<millimeter>(1.0)),
        Vector3::z(),
        Length::new::<nanometer>(1053.0),
        Energy::new::<joule>(1.0),
    )
    .unwrap();
    println!("ray: {:?}", ray);
    let ray=ray.refract_paraxial(Length::new::<millimeter>(-10.0)).unwrap();
    println!("refracted ray: {:?}", ray);
    let ray=ray.propagate_along_z(Length::new::<millimeter>(5.0)).unwrap();
    println!("propagated ray: {:?}", ray);
    Ok(())
}
