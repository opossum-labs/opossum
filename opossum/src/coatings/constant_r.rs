use super::{Coating, CoatingType};
use crate::{
    error::{OpmResult, OpossumError},
    ray::Ray,
};
use nalgebra::Vector3;

pub struct ConstantR {
    reflectivity: f64,
}

impl ConstantR {
    pub fn new(reflectivity: f64) -> OpmResult<Self> {
        if !(0.0..=1.0).contains(&reflectivity) || !reflectivity.is_normal() {
            return Err(OpossumError::Other(
                "reflectivity must be within [0.0, 1.0] and finite.".into(),
            ));
        }
        Ok(Self { reflectivity })
    }
}

impl Coating for ConstantR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: &Ray,
        _surface_normal: Vector3<f64>,
        _n2: f64,
    ) -> f64 {
        self.reflectivity
    }

    fn to_enum(&self) -> super::CoatingType {
        CoatingType::ConstantR {
            reflectivity: self.reflectivity,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, nanometer, ray::Ray};
    use core::f64;
    use nalgebra::vector;

    #[test]
    fn new() {
        assert!(ConstantR::new(-0.1).is_err());
        assert!(ConstantR::new(f64::NAN).is_err());
        assert!(ConstantR::new(f64::INFINITY).is_err());
        assert!(ConstantR::new(f64::NEG_INFINITY).is_err());
        assert!(ConstantR::new(1.0).is_ok());
        assert!(ConstantR::new(1.0).is_ok());
        assert!(ConstantR::new(1.1).is_err());
    }
    #[test]
    fn to_enum() {
        let coating = ConstantR::new(0.5).unwrap();
        assert!(matches!(
            coating.to_enum(),
            CoatingType::ConstantR { reflectivity: 0.5 }
        ));
    }
    #[test]
    fn calc_refl() {
        let coating = ConstantR::new(0.5).unwrap();
        let ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        assert_eq!(coating.calc_reflectivity(&ray, surface_normal, 1.5), 0.5);
    }
}
