#![warn(missing_docs)]
//! Amplification models for active (gain) media.
//!
//! Amplification is not modelled as a dedicated node type. Instead, every node that has a physical
//! volume (a lens, a wedge, a cylindric lens, ...) carries an `amp config` property holding a
//! [`GainModel`]. A node whose model is [`GainModel::None`] behaves exactly like the passive
//! component it is — which is why the property can be declared unconditionally without changing
//! any existing simulation result.
//!
//! Turning an existing component into an amplifier is therefore an ordinary property change, and
//! each escalation stage of the gain modelling adds one further [`GainModel`] variant rather than
//! a parallel set of node types.
//!
//! # Note on the current state
//!
//! The models defined here are carriers only: no amplification is applied during ray tracing yet.
//! The evaluation happens in the shared volume propagation of the hosting node and is added in a
//! later step.

use crate::{
    error::{OpmResult, OpossumError},
    utils::default_from_name::DefaultFromName,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Parameters of a constant, path-length independent energy gain.
///
/// This is the simplest conceivable amplifier: the energy of every ray is multiplied by the same
/// factor, regardless of how far the ray actually travels inside the medium and regardless of how
/// much energy has already been extracted. It is meant for chain layout and system overview, not
/// for a physically faithful description of an amplifier.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ConstGain {
    gain: f64,
}
impl Default for ConstGain {
    /// Create a neutral [`ConstGain`] with a gain factor of 1.0 (no amplification).
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}
impl ConstGain {
    /// Create a new [`ConstGain`] with the given energy gain factor.
    ///
    /// # Arguments
    ///
    /// * `gain` - energy gain factor. A value of 1.0 leaves the energy unchanged.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the given factor is not finite or negative.
    pub fn new(gain: f64) -> OpmResult<Self> {
        if !gain.is_finite() || gain.is_sign_negative() {
            return Err(OpossumError::Other(
                "gain factor must be finite and non-negative".into(),
            ));
        }
        Ok(Self { gain })
    }
    /// Return the energy gain factor.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }
}

/// Amplification model of a node with a volume.
///
/// Each escalation stage of the gain modelling adds one variant here. The hosting node does not
/// change, only the value of its `amp config` property.
#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, EnumIter)]
#[non_exhaustive]
pub enum GainModel {
    /// No amplification. The node behaves exactly like the passive component it is.
    ///
    /// This is the default for every node, so declaring the property does not change any result.
    #[default]
    None,
    /// Constant energy gain, independent of the path length through the medium and without
    /// saturation. See [`ConstGain`].
    Const(ConstGain),
}
impl Display for GainModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Const(_) => write!(f, "Const"),
        }
    }
}
impl DefaultFromName for GainModel {}
impl GainModel {
    /// Return whether this model amplifies at all.
    ///
    /// Used to decide whether a node has to be treated as an active medium, e.g. when listing all
    /// amplifiers of a document or when deciding what to show on the node itself.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::properties::Proptype;
    use approx::assert_relative_eq;
    use strum::IntoEnumIterator;

    #[test]
    fn default_is_inactive() {
        assert_eq!(GainModel::default(), GainModel::None);
        assert!(!GainModel::default().is_active());
        assert!(GainModel::Const(ConstGain::default()).is_active());
    }
    #[test]
    fn const_gain_default_is_neutral() {
        assert_relative_eq!(ConstGain::default().gain(), 1.0);
    }
    #[test]
    fn const_gain_new() -> OpmResult<()> {
        assert_relative_eq!(ConstGain::new(2.5)?.gain(), 2.5);
        assert_relative_eq!(ConstGain::new(0.0)?.gain(), 0.0);
        assert!(ConstGain::new(-1.0).is_err());
        assert!(ConstGain::new(f64::NAN).is_err());
        assert!(ConstGain::new(f64::INFINITY).is_err());
        Ok(())
    }
    #[test]
    fn fmt() {
        assert_eq!(format!("{}", GainModel::None), "None");
        assert_eq!(
            format!("{}", GainModel::Const(ConstGain::default())),
            "Const"
        );
    }
    #[test]
    fn default_from_name_yields_neutral_variants() {
        assert_eq!(GainModel::default_from_name("None"), Some(GainModel::None));
        // A freshly selected `Const` must not annihilate the beam, so its default is 1.0.
        let Some(GainModel::Const(const_gain)) = GainModel::default_from_name("Const") else {
            panic!("expected a Const gain model");
        };
        assert_relative_eq!(const_gain.gain(), 1.0);
        assert_eq!(GainModel::default_from_name("does not exist"), None);
    }
    #[test]
    fn all_variants_are_reachable_by_name() {
        for variant in GainModel::iter() {
            assert_eq!(
                GainModel::default_from_name(&variant.to_string()),
                Some(variant),
                "variant {variant} cannot be recreated from its display name"
            );
        }
    }
    #[test]
    fn into_proptype() {
        assert!(matches!(
            GainModel::None.into(),
            Proptype::GainModel(GainModel::None)
        ));
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        for model in [GainModel::None, GainModel::Const(ConstGain::new(3.0)?)] {
            let serialized =
                ron::to_string(&model).map_err(|e| OpossumError::Other(e.to_string()))?;
            let deserialized: GainModel =
                ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(model, deserialized);
        }
        Ok(())
    }
}
