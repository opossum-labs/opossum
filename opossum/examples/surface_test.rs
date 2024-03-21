use nalgebra::Vector3;
use opossum::{
    error::OpmResult,
    joule, millimeter, nanometer,
    ray::Ray,
    surface::{Sphere, Surface},
};

fn main() -> OpmResult<()> {
    let surface = Sphere::new(millimeter!(10.0), millimeter!(1.0))?; //Plane::new(10.0);
    let ray = Ray::new(
        millimeter!(15.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        nanometer!(1053.0),
        joule!(1.0),
    )?;
    if let Some((intersection_point, normal_vector)) = surface.calc_intersect_and_normal(&ray) {
        println!("interesection point: {intersection_point:?}");
        println!("normal vector: {normal_vector:?}");
    } else {
        println!("no intersection");
    };
    Ok(())
}
