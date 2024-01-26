use nalgebra::Point2;
use opossum::ray::Ray;
use uom::num_traits::Zero;
use uom::si::energy::joule;
use uom::si::length::nanometer;
use uom::si::{
    f64::{Energy, Length},
    length::millimeter,
};

use opossum::error::OpmResult;

#[derive(Debug)]
struct SRay {
    pos: f64,
    dir: (f64, f64),
}
impl SRay {
    fn refract(&self, f: f64) -> Self {
        let new_dir = (self.dir.0 - (1.0 / f) * self.pos, 1.0);
        Self {
            pos: self.pos,
            dir: new_dir,
        }
    }
    fn propagate(&self, d: f64) -> Self {
        let length_in_ray_dir = d / self.dir.1;
        let new_pos = self.pos + length_in_ray_dir * self.dir.0;
        Self {
            pos: new_pos,
            dir: self.dir,
        }
    }
}
fn main() -> OpmResult<()> {
    let mut ray = Ray::new_collimated(
        Point2::new(Length::zero(), Length::new::<millimeter>(10.0)),
        Length::new::<nanometer>(1053.0),
        Energy::new::<joule>(1.0),
    )?;
    println!("{:?}", ray);
    let length = Length::new::<millimeter>(50.0);
    ray = ray.refract_paraxial(length)?;
    ray = ray.propagate_along_z(length)?;
    println!("{:?}", ray);
    ray = ray.propagate_along_z(length)?;
    ray = ray.refract_paraxial(length)?;
    println!("{:?}", ray);

    let mut ray = SRay {
        pos: 10.0,
        dir: (0.0, 1.0),
    };
    println!("{:?}", ray);
    let f = 100.0;
    ray = ray.refract(f);
    println!("{:?}", ray);
    ray = ray.propagate(f);
    println!("{:?}", ray);
    ray = ray.propagate(f);
    println!("{:?}", ray);
    ray = ray.refract(f);
    println!("{:?}", ray);
    Ok(())
}
