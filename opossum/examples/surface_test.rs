use nalgebra::{Point2, Vector3};
use opossum::{
    error::OpmResult,
    ray::Ray,
    surface::{Plane, Surface},
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let plane = Plane::new(10.0);
    let ray = Ray::new(
        Point2::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
        ),
        Vector3::new(0.0, 1.0, -1.0),
        Length::new::<nanometer>(1053.0),
        Energy::new::<joule>(1.0),
    )?;
    if let Some((intersection_point, normal_vector)) = plane.calc_intersect_and_normal(&ray) {
        println!("interesection point: {intersection_point:?}");
        println!("normal vector: {normal_vector:?}");
    } else {
        println!("no intersection");
    };
    Ok(())
}
