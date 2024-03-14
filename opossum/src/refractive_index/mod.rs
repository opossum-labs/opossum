use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

pub mod refr_index_const;
pub mod refr_index_schott;
pub mod refr_index_sellmeier1;

pub use refr_index_const::refr_index_vaccuum;
pub use refr_index_const::RefrIndexConst;
pub use refr_index_sellmeier1::RefrIndexSellmeier1;

use self::refr_index_schott::RefrIndexSchott;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum RefractiveIndexType {
    Const(RefrIndexConst),
    Sellmeier1(RefrIndexSellmeier1),
    Schott(RefrIndexSchott),
}

impl RefractiveIndexType {
    #[must_use]
    pub fn get_refractive_index(&self, wavelength: Length) -> f64 {
        match self {
            Self::Const(refr_index_const) => refr_index_const.get_refractive_index(wavelength),
            Self::Sellmeier1(refr_index_sellmeier1) => {
                refr_index_sellmeier1.get_refractive_index(wavelength)
            }
            Self::Schott(refr_index_schott) => refr_index_schott.get_refractive_index(wavelength),
        }
    }
}

pub trait RefractiveIndex {
    fn get_refractive_index(&self, wavelength: Length) -> f64;
    fn to_enum(&self) -> RefractiveIndexType;
}
