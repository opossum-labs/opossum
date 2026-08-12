#![warn(missing_docs)]
//! The material an optical component with a volume is made of.
//!
//! A [`Material`] is the single place where everything a physics model needs to know about the
//! *substance* inside a component lives. Today that is only the wavelength-dependent refractive
//! index; spectroscopic data (emission/absorption cross sections, fluorescence lifetime, dopant
//! density) are added as optional fields as soon as a model actually reads them.
//!
//! # Why a material and not a bare refractive index
//!
//! Every model that goes beyond pure ray geometry needs more than one material datum, and it needs
//! to know *up front* whether the material at hand can supply them at all. [`Material::provides`]
//! answers exactly that question: it lists the [`MaterialProperty`] values this material carries,
//! so a model can be rejected with a comprehensible message instead of silently computing with a
//! missing quantity.
//!
//! The concrete data are meant to come from a separate, generic material library later on. This
//! module therefore only fixes the *interface* ([`Material`] and [`MaterialProperty`]); which
//! substances exist and where their numbers come from is deliberately left open.

use crate::{properties::Proptype, refractive_index::RefractiveIndexType};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// A physical quantity a [`Material`] can supply to a physics model.
///
/// This is the vocabulary shared by the two sides of the material contract: a material declares
/// what it carries via [`Material::provides`], and a physics model declares what it needs. One
/// variant is added per datum an escalation stage of the gain modelling starts to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
#[non_exhaustive]
pub enum MaterialProperty {
    /// The wavelength-dependent refractive index n(λ).
    RefractiveIndex,
}
impl Display for MaterialProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RefractiveIndex => write!(f, "refractive index"),
        }
    }
}

/// The material an optical component with a volume is made of.
///
/// A [`Material`] always carries a refractive index model, which is why every component that has a
/// material can be ray-traced. Everything beyond that is optional and has to be queried through
/// [`Material::provides`] before it is used.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Material {
    refractive_index: RefractiveIndexType,
}
impl Material {
    /// Create a new [`Material`] with the given refractive index model.
    ///
    /// # Arguments
    ///
    /// * `refractive_index` - the refractive index model n(λ) of the material.
    ///
    /// # Returns
    ///
    /// The new [`Material`].
    #[must_use]
    pub const fn new(refractive_index: RefractiveIndexType) -> Self {
        Self { refractive_index }
    }
    /// Return the refractive index model n(λ) of this [`Material`].
    #[must_use]
    pub const fn refractive_index(&self) -> &RefractiveIndexType {
        &self.refractive_index
    }
    /// Return the [`MaterialProperty`] values this [`Material`] can supply.
    ///
    /// A physics model has to check its own requirements against this list before it computes
    /// anything, so a material that is missing a datum is reported rather than silently treated as
    /// if the datum were zero.
    ///
    /// # Returns
    ///
    /// The properties this material carries. Currently always exactly the refractive index — the
    /// list becomes value-dependent as soon as the first optional datum exists.
    #[must_use]
    #[allow(clippy::unused_self)] // the set depends on which optional data a material carries
    pub const fn provides(&self) -> &'static [MaterialProperty] {
        &[MaterialProperty::RefractiveIndex]
    }
}
impl Display for Material {
    /// Describe the material by its refractive index model, the only datum it carries so far.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.refractive_index)
    }
}
impl From<RefractiveIndexType> for Material {
    fn from(refractive_index: RefractiveIndexType) -> Self {
        Self::new(refractive_index)
    }
}
impl From<Material> for Proptype {
    fn from(material: Material) -> Self {
        Self::Material(material)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        error::{OpmResult, OpossumError},
        nanometer,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_relative_eq;

    #[test]
    fn default_material_is_the_default_index_model() {
        assert_eq!(
            *Material::default().refractive_index(),
            RefractiveIndexType::default()
        );
    }
    #[test]
    fn new_keeps_the_given_index_model() -> OpmResult<()> {
        let material = Material::new(RefractiveIndexType::Const(RefrIndexConst::new(1.5)?));
        assert_relative_eq!(
            material
                .refractive_index()
                .get_refractive_index(nanometer!(1054.0))?,
            1.5
        );
        Ok(())
    }
    #[test]
    fn provides_the_refractive_index() {
        assert_eq!(
            Material::default().provides(),
            &[MaterialProperty::RefractiveIndex]
        );
    }
    #[test]
    fn material_property_fmt() {
        assert_eq!(
            format!("{}", MaterialProperty::RefractiveIndex),
            "refractive index"
        );
    }
    #[test]
    fn fmt_shows_the_index_model() {
        assert_eq!(format!("{}", Material::default()), "Sellmeier equation");
    }
    #[test]
    fn from_refractive_index_type() -> OpmResult<()> {
        let index = RefractiveIndexType::Const(RefrIndexConst::new(1.5)?);
        assert_eq!(Material::from(index.clone()), Material::new(index));
        Ok(())
    }
    #[test]
    fn into_proptype() {
        assert!(matches!(
            Material::default().into(),
            Proptype::Material(material) if material == Material::default()
        ));
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        let material = Material::new(RefractiveIndexType::Const(RefrIndexConst::new(1.5)?));
        let serialized =
            ron::to_string(&material).map_err(|e| OpossumError::Other(e.to_string()))?;
        let deserialized: Material =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(material, deserialized);
        Ok(())
    }
}
