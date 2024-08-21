use crate::error::{OpmResult, OpossumError};
use super::{Coating, CoatingType};

pub struct ConstantR {
    reflectivity: f64,
}

impl ConstantR {
    pub fn new(reflectivity: f64) -> OpmResult<ConstantR> {
        if reflectivity.is_sign_negative() || !reflectivity.is_normal() {
            return Err(OpossumError::Other(
                "reflectivity must be > 0.0 and finite.".into(),
            ));
        }
        Ok(ConstantR { reflectivity })
    }
}

impl Coating for ConstantR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: crate::ray::Ray,
        _surface_normal: nalgebra::Vector3<f64>,
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
