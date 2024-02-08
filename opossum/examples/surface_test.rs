use nalgebra::{Point2, Vector3};
use opossum::{
    error::OpmResult,
    ray::Ray,
    surface::{Sphere, Surface},
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let surface = Sphere::new(10.0, 1.0)?; //Plane::new(10.0);
    let ray = Ray::new(
        Point2::new(
            Length::new::<millimeter>(15.0),
            Length::new::<millimeter>(0.0),
        ),
        Vector3::new(0.0, 0.0, 1.0),
        Length::new::<nanometer>(1053.0),
        Energy::new::<joule>(1.0),
    )?;
    if let Some((intersection_point, normal_vector)) = surface.calc_intersect_and_normal(&ray) {
        println!("interesection point: {intersection_point:?}");
        println!("normal vector: {normal_vector:?}");
    } else {
        println!("no intersection");
    };
    Ok(())
}
