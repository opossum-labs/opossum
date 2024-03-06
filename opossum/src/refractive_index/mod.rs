use uom::si::f64::Length;

pub mod refr_index_const;
pub use refr_index_const::RefrIndexConst;

pub enum RefractiveIndexType {
  Const(RefrIndexConst)
}

pub trait RefractiveIndex {
  fn get_refractive_index(&self, wavelength: Length) -> f64;
  fn to_enum(&self) -> RefractiveIndexType;
}