use crate::error::OpmResult;
use crate::error::OpossumError;
use super::{RefractiveIndex, RefractiveIndexType};

#[derive(Clone)]
pub struct RefrIndexConst {
  refractive_index: f64
}
impl RefrIndexConst {
  pub fn new(refractive_index: f64) -> OpmResult<Self> {
    if refractive_index<1.0 || !refractive_index.is_finite() {
      return Err(OpossumError::Other("refractive inde must be >=1.0 and finite.".into()));
    }
    Ok(Self{refractive_index})
  }
}

impl RefractiveIndex for RefrIndexConst {
    fn get_refractive_index(&self, _wavelength: uom::si::f64::Length) -> f64 {
        self.refractive_index
    }
    fn to_enum(&self) -> super::RefractiveIndexType {
        RefractiveIndexType::Const(self.clone())
    }
}