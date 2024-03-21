use opossum::error::OpmResult;
use opossum::{joule, millimeter, nanometer, ray::Ray};

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
    let mut ray = Ray::new_collimated(millimeter!(0., 10., 0.), nanometer!(1053.0), joule!(1.0))?;
    println!("{:?}", ray);
    let length = millimeter!(50.0);
    let _ = ray.refract_paraxial(length)?;
    let _ = ray.propagate_along_z(length)?;
    println!("{:?}", ray);
    let _ = ray.propagate_along_z(length)?;
    let _ = ray.refract_paraxial(length)?;
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
