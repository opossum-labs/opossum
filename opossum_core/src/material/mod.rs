#![warn(missing_docs)]
//! The material an optical component with a volume is made of.
//!
//! A [`Material`] is the single place where everything a physics model needs to know about the
//! *substance* inside a component lives. Today it can only be defined the way it always was — by a
//! hand-written refractive index model. Spectroscopic data (emission/absorption cross sections,
//! fluorescence lifetime, dopant density) arrive as further *variants*: a named substance from a
//! material library brings its own data along, instead of every material growing optional fields
//! that are empty for most of them.
//!
//! # Why a material and not a bare refractive index
//!
//! Every model that goes beyond pure ray geometry needs more than one material datum, and it needs
//! to know *up front* whether the material at hand can supply them at all. [`Material::provides`]
//! answers exactly that question: it lists the [`MaterialProperty`] values this material carries,
//! so a model can be rejected with a comprehensible message instead of silently computing with a
//! missing quantity.
//!
//! The concrete substances are meant to come from a separate, generic material library later on.
//! This module therefore only fixes the *interface* ([`Material`] and [`MaterialProperty`]); which
//! substances exist and where their numbers come from is deliberately left open.

use crate::{
    properties::Proptype, refractive_index::RefractiveIndexType,
    utils::default_from_name::DefaultFromName,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Name of the property that carries the [`Material`] of a node with a volume.
///
/// Up to and including OPOSSUM 0.7.2 the same slot held a bare refractive index model under the
/// name `refractive index`; see [`LEGACY_REFRACTIVE_INDEX`].
pub const MATERIAL: &str = "material";

/// Name the [`MATERIAL`] property had before it carried a whole [`Material`].
///
/// Kept so that `.opm` files written by an older OPOSSUM can be migrated on load — without that,
/// [`Properties::update`](crate::properties::Properties::update) would silently drop the old key
/// and the node would fall back to its default material.
pub const LEGACY_REFRACTIVE_INDEX: &str = "refractive index";

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
/// The variants are the different *ways to define* a material, which is what a user picks first:
/// either the substance is described by hand — today only by its refractive index — or, once a
/// material library exists, it is a named substance that brings its own data along. Every variant
/// can supply a refractive index, which is why every component with a material can be ray-traced;
/// everything beyond that has to be queried through [`Material::provides`] before it is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
#[non_exhaustive]
pub enum Material {
    /// A material described by nothing but a hand-written refractive index model n(λ).
    ///
    /// This is what OPOSSUM always had: enough to refract a ray, and nothing else.
    RefractiveIndex(RefractiveIndexType),
}
impl Default for Material {
    /// A material given by the default refractive index model.
    fn default() -> Self {
        Self::RefractiveIndex(RefractiveIndexType::default())
    }
}
impl Material {
    /// Return the refractive index model n(λ) of this [`Material`].
    ///
    /// Every way of defining a material has to be able to answer this, so this is an accessor and
    /// not a [`MaterialProperty`] that could be missing.
    #[must_use]
    pub const fn refractive_index(&self) -> &RefractiveIndexType {
        match self {
            Self::RefractiveIndex(refractive_index) => refractive_index,
        }
    }
    /// Return the [`MaterialProperty`] values this [`Material`] can supply.
    ///
    /// A physics model has to check its own requirements against this list before it computes
    /// anything, so a material that is missing a datum is reported rather than silently treated as
    /// if the datum were zero.
    ///
    /// # Returns
    ///
    /// The properties this material carries. A hand-written index model carries exactly the
    /// refractive index; a substance from a material library will carry more.
    #[must_use]
    pub const fn provides(&self) -> &'static [MaterialProperty] {
        match self {
            Self::RefractiveIndex(_) => &[MaterialProperty::RefractiveIndex],
        }
    }
}
impl Display for Material {
    /// Name the way this material is defined — this is what the selector in the GUI shows.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RefractiveIndex(_) => write!(f, "Refractive index"),
        }
    }
}
impl DefaultFromName for Material {}
impl From<RefractiveIndexType> for Material {
    fn from(refractive_index: RefractiveIndexType) -> Self {
        Self::RefractiveIndex(refractive_index)
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
    use strum::IntoEnumIterator;

    #[test]
    fn default_material_is_the_default_index_model() {
        assert_eq!(
            *Material::default().refractive_index(),
            RefractiveIndexType::default()
        );
    }
    #[test]
    fn refractive_index_variant_keeps_its_index_model() -> OpmResult<()> {
        let material =
            Material::RefractiveIndex(RefractiveIndexType::Const(RefrIndexConst::new(1.5)?));
        assert_relative_eq!(
            material
                .refractive_index()
                .get_refractive_index(nanometer!(1054.0))?,
            1.5
        );
        Ok(())
    }
    #[test]
    fn every_variant_is_reachable_by_name() {
        // The GUI builds its material selector from the variant names, so each name must recreate
        // its variant - otherwise a selectable entry would silently do nothing.
        for variant in Material::iter() {
            assert_eq!(
                Material::default_from_name(&variant.to_string()),
                Some(variant.clone()),
                "variant {variant} cannot be recreated from its display name"
            );
        }
        assert_eq!(Material::default_from_name("does not exist"), None);
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
    fn fmt_names_the_way_the_material_is_defined() {
        assert_eq!(format!("{}", Material::default()), "Refractive index");
    }
    #[test]
    fn from_refractive_index_type() -> OpmResult<()> {
        let index = RefractiveIndexType::Const(RefrIndexConst::new(1.5)?);
        assert_eq!(
            Material::from(index.clone()),
            Material::RefractiveIndex(index)
        );
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
        let material =
            Material::RefractiveIndex(RefractiveIndexType::Const(RefrIndexConst::new(1.5)?));
        let serialized =
            ron::to_string(&material).map_err(|e| OpossumError::Other(e.to_string()))?;
        let deserialized: Material =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(material, deserialized);
        Ok(())
    }
}
