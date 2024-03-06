use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

pub mod refr_index_const;
pub use refr_index_const::RefrIndexConst;

use crate::properties::Proptype;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum RefractiveIndexType {
    Const(RefrIndexConst),
}
impl From<RefractiveIndexType> for Proptype {
    fn from(value: RefractiveIndexType) -> Self {
        Self::RefractiveIndex(value)
    }
}
pub trait RefractiveIndex {
    fn get_refractive_index(&self, wavelength: Length) -> f64;
    fn to_enum(&self) -> RefractiveIndexType;
}
