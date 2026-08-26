#![warn(missing_docs)]
//! Amplification models for active (gain) media.
//!
//! Amplification is not modelled as a dedicated node type. Instead, a document-wide
//! [`PumpScenario`] maps the uuid of any node with a physical volume (a lens, a wedge, a cylindric
//! lens, ...) to a [`PumpConfig`] — see
//! [`PropagationStrategy::pump_config`](crate::analyzers::propagation_strategy::PropagationStrategy::pump_config).
//! A node the active scenario does not name, or one mapped to [`GainModel::None`], behaves exactly
//! like the passive component it is.
//!
//! Each escalation stage of the gain modelling adds one further [`GainModel`] variant rather than a
//! parallel set of node types.

pub mod extraction;
pub mod inversion_field;
pub mod pump_source;
pub mod scenario;
pub mod small_signal;
pub use extraction::{Extraction, Medium};
pub use inversion_field::InversionField;
pub use pump_source::{
    AnalyticPump, BeerLambertProfile, ConstInversion, LongitudinalProfile, PumpDirection,
    PumpSource, TransversalProfile,
};
pub use scenario::{ActiveScenario, PumpConfig, PumpScenario};
pub use small_signal::SmallSignalGain;

use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllPositive, ValidateTrait},
    geometry::body::Body,
    light::{Rays, Spectrum},
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;
use utoipa::ToSchema;

/// Deserialization shim for [`ConstGain`].
///
/// It lets a gain factor read from an `.opm` file run through the very same validation as one set
/// through [`ConstGain::set_gain`], so a hand-edited file cannot smuggle in a negative or
/// non-finite factor. Same pattern as [`RefrIndexConst`](crate::refractive_index::RefrIndexConst).
#[derive(Deserialize)]
struct NonValidatedConstGain {
    gain: f64,
}
impl TryFrom<NonValidatedConstGain> for ConstGain {
    type Error = String;
    fn try_from(helper: NonValidatedConstGain) -> Result<Self, Self::Error> {
        Self::new(helper.gain).map_err(|e| e.to_string())
    }
}

/// A gain factor that is guaranteed to be finite and non-negative.
type ValidatedGain = validated_type!(f64, AllFinite && AllPositive);
impl Default for ValidatedGain {
    /// A gain factor of 1.0, i.e. no amplification.
    fn default() -> Self {
        validated!(1.0, AllFinite && AllPositive).unwrap()
    }
}

/// Parameters of a constant, path-length independent energy gain.
///
/// This is the simplest conceivable amplifier: the energy of every ray is multiplied by the same
/// factor, regardless of how far the ray actually travels inside the medium and regardless of how
/// much energy has already been extracted. It is meant for chain layout and system overview, not
/// for a physically faithful description of an amplifier.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated)]
#[serde(try_from = "NonValidatedConstGain")]
pub struct ConstGain {
    gain: ValidatedGain,
}
impl Default for ConstGain {
    /// Create a neutral [`ConstGain`] with a gain factor of 1.0 (no amplification).
    fn default() -> Self {
        Self {
            gain: ValidatedGain::default(),
        }
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
        let mut const_gain = Self::default();
        const_gain.set_gain(gain)?;
        Ok(const_gain)
    }
    /// Return the energy gain factor.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        *self.gain.get()
    }
    /// Set the energy gain factor.
    ///
    /// # Arguments
    ///
    /// * `gain` - energy gain factor. A value of 1.0 leaves the energy unchanged.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the given factor is not finite or negative. The
    /// previous value is kept in that case.
    pub fn set_gain(&mut self, gain: f64) -> OpmResult<()> {
        self.gain.set(gain)
    }
}
impl From<ConstGain> for GainModel {
    fn from(value: ConstGain) -> Self {
        Self::Const(value)
    }
}
impl Extraction for ConstGain {
    fn name(&self) -> &'static str {
        "Const"
    }
    fn needs_inversion(&self) -> bool {
        // A constant factor is constant: how the medium was pumped, or whether it was pumped at
        // all, cannot change it.
        false
    }
    fn pumped_medium(
        &self,
        _body: &dyn Body,
        _config: &PumpConfig,
    ) -> OpmResult<Option<InversionField>> {
        // Reading no inversion, this model must not pay for a grid either.
        Ok(None)
    }
    fn amplify_rays(&self, _medium: &Medium<'_>, rays_bundle: &mut [Rays]) -> OpmResult<()> {
        // Independent of the path through the medium by definition, so every ray of the bundle is
        // multiplied by the same factor, once per pass.
        for rays in rays_bundle.iter_mut() {
            rays.scale_energy(self.gain())?;
        }
        Ok(())
    }
    fn amplify_spectrum(&self, _medium: &Medium<'_>, spectrum: &mut Spectrum) -> OpmResult<()> {
        // Needing no beam path, this model is just as evaluable in an energy flow as in a ray
        // trace - leaving it out would silently report an amplifier chain as passive.
        spectrum.scale_vertical(&self.gain())
    }
}
impl From<SmallSignalGain> for GainModel {
    fn from(value: SmallSignalGain) -> Self {
        Self::SmallSignalGain(value)
    }
}

/// Amplification model of a node with a volume.
///
/// Each escalation stage of the gain modelling adds one variant here. The hosting node does not
/// change, only the value of its `amp config` property.
#[derive(Default, Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnumIter)]
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
    /// Unsaturated gain integrated along the path through the medium, reading the local inversion.
    /// See [`SmallSignalGain`].
    SmallSignalGain(SmallSignalGain),
}
impl Display for GainModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.as_extraction() {
            None => write!(f, "None"),
            Some(model) => write!(f, "{}", model.name()),
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
    /// Return this model as an [`Extraction`], or `None` if it does not amplify.
    ///
    /// **The one place a variant of this enum is named outside its own module.** Everything a model
    /// does — how it is displayed, whether it reads the medium, how it discretises it and how it
    /// amplifies — is asked of the [`Extraction`] and therefore lives with the model, so adding a
    /// stage to the gain modelling does not edit the volume machinery every component shares.
    ///
    /// The match stays exhaustive: a variant added later has to appear here, and the trait it then
    /// has to implement has no default methods, so the compiler still walks it through every
    /// question rather than answering any of them silently.
    ///
    /// # Returns
    ///
    /// The model behind this variant, or `None` for [`GainModel::None`], which has no behaviour to
    /// hand out.
    #[must_use]
    pub const fn as_extraction(&self) -> Option<&dyn Extraction> {
        match self {
            Self::None => None,
            Self::Const(model) => Some(model),
            Self::SmallSignalGain(model) => Some(model),
        }
    }
    /// Return whether this model draws on the inversion stored in the medium.
    ///
    /// See [`Extraction::needs_inversion`], which answers it. Kept on the enum because a caller
    /// deciding whether to *offer* pump settings holds a [`GainModel`], including the passive one.
    ///
    /// # Returns
    ///
    /// `true` if the medium's inversion is an input to this model.
    #[must_use]
    pub fn needs_inversion(&self) -> bool {
        self.as_extraction()
            .is_some_and(Extraction::needs_inversion)
    }
    /// Return this model's display name, or `None` if it does not amplify.
    ///
    /// This is the shape a user interface wants: one value that answers "does it amplify" and
    /// "what is it called" at once, so no caller has to pair [`Self::is_active`] with a separate
    /// `to_string`.
    #[must_use]
    pub fn active_name(&self) -> Option<String> {
        self.is_active().then(|| self.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error::OpossumError;
    use approx::assert_relative_eq;
    use strum::IntoEnumIterator;

    #[test]
    fn default_is_inactive() {
        assert_eq!(GainModel::default(), GainModel::None);
        assert!(!GainModel::default().is_active());
        assert!(GainModel::Const(ConstGain::default()).is_active());
        assert!(GainModel::SmallSignalGain(SmallSignalGain::default()).is_active());
        assert_eq!(GainModel::None.active_name(), None);
        assert_eq!(
            GainModel::Const(ConstGain::default())
                .active_name()
                .as_deref(),
            Some("Const")
        );
        assert_eq!(
            GainModel::SmallSignalGain(SmallSignalGain::default())
                .active_name()
                .as_deref(),
            Some("SmallSignalGain")
        );
    }
    /// Only the models that actually evaluate an inversion say so, and that answer is what makes
    /// the pump settings reachable in the first place - the scenario editor offers them exactly
    /// where this returns `true`. The exhaustive match in `needs_inversion` is what forces every
    /// further stage to make the decision rather than inherit a silent "no".
    #[test]
    fn only_the_models_reading_the_medium_say_so() {
        assert!(!GainModel::None.needs_inversion());
        assert!(!GainModel::Const(ConstGain::default()).needs_inversion());
        assert!(GainModel::SmallSignalGain(SmallSignalGain::default()).needs_inversion());
        // A model that reads the inversion has to amplify at all, or the pump settings would be
        // offered for something that then does nothing with them.
        for variant in GainModel::iter().filter(GainModel::needs_inversion) {
            assert!(
                variant.is_active(),
                "model {variant} reads the inversion but does not amplify"
            );
        }
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
    fn const_gain_set_gain_keeps_old_value_on_rejection() {
        let mut const_gain = ConstGain::default();
        assert!(const_gain.set_gain(2.5).is_ok());
        assert_relative_eq!(const_gain.gain(), 2.5);
        // A rejected edit (e.g. a half-typed value in the GUI) must leave the gain untouched.
        assert!(const_gain.set_gain(-1.0).is_err());
        assert_relative_eq!(const_gain.gain(), 2.5);
    }
    #[test]
    fn fmt() {
        assert_eq!(format!("{}", GainModel::None), "None");
        assert_eq!(
            format!("{}", GainModel::Const(ConstGain::default())),
            "Const"
        );
        assert_eq!(
            format!("{}", GainModel::SmallSignalGain(SmallSignalGain::default())),
            "SmallSignalGain"
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
        // A freshly selected `SmallSignalGain` is neutral for a different reason: it amplifies by
        // whatever the medium holds, and nobody has pumped it yet. Its own parameters must still be
        // usable rather than neutral - see `SmallSignalGain::emission_cross_section`.
        let Some(GainModel::SmallSignalGain(model)) =
            GainModel::default_from_name("SmallSignalGain")
        else {
            panic!("expected a small signal gain model");
        };
        assert!(model.emission_cross_section().value > 0.0);
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
    fn serde_roundtrip() -> OpmResult<()> {
        for model in [
            GainModel::None,
            GainModel::Const(ConstGain::new(3.0)?),
            GainModel::SmallSignalGain(SmallSignalGain::new(
                crate::square_meter!(3.0e-24),
                8,
                (4, 5, 6),
            )?),
        ] {
            let serialized =
                ron::to_string(&model).map_err(|e| OpossumError::Other(e.to_string()))?;
            let deserialized: GainModel =
                ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(model, deserialized);
        }
        Ok(())
    }
}
