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
    core_optics::NodeAttr,
    error::OpmResult,
    generic_validators::{AllFinite, AllPositive, ValidateTrait},
    properties::Proptype,
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Name of the property that carries the [`GainModel`] of a node with a volume.
///
/// Every node listed in [`AMP_CONFIG_NODE_TYPES`] declares this property unconditionally, so
/// turning a component into an amplifier is an ordinary property patch on this key.
pub const AMP_CONFIG: &str = "amp config";

/// Node types that declare the [`AMP_CONFIG`] property, i.e. the components that have a physical
/// volume in which amplification could take place.
///
/// This is the list a user interface needs in order to decide whether it may offer amplification
/// for a node it only knows by its type name. It is kept in sync with the actual node declarations
/// by the `amp_config_node_types_are_exhaustive` test below.
pub const AMP_CONFIG_NODE_TYPES: &[&str] = &["cylindric lens", "lens", "wedge"];

/// Return the name of the active [`GainModel`] declared by `node_attr`, or `None` if the node does
/// not amplify.
///
/// `None` covers both cases a user interface has to treat alike: a node without a volume, which
/// never declares [`AMP_CONFIG`] at all, and a volume node whose model is [`GainModel::None`].
///
/// # Arguments
///
/// * `node_attr` - attributes of the node to inspect.
///
/// # Returns
///
/// The display name of the active model, or `None`.
#[must_use]
pub fn active_amp_model(node_attr: &NodeAttr) -> Option<String> {
    match node_attr.get_property(AMP_CONFIG) {
        Ok(Proptype::GainModel(model)) => model.active_name(),
        _ => None,
    }
}

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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, EnsureValidated)]
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
    use crate::{
        core_optics::node_attr::HasNodeAttr,
        error::OpossumError,
        geometry::body::CLEAR_APERTURE,
        nodes::{Dummy, Lens, create_node_ref, node_types},
        utils::LockExt,
    };
    use approx::assert_relative_eq;
    use strum::IntoEnumIterator;

    /// "Node with a volume" must mean the same thing wherever it is stated.
    ///
    /// Three places say it independently: this list, the nodes that declare the volume properties,
    /// and the nodes whose analysis goes through `unified_analyze_volume_node`. The first two are
    /// pinned to each other here; the third follows once `Volumetric` replaces the list altogether.
    #[test]
    fn amp_config_node_types_are_exhaustive() -> OpmResult<()> {
        let mut declaring_amp_config = Vec::new();
        let mut declaring_clear_aperture = Vec::new();
        for (node_type, _) in node_types() {
            let optic_ref = create_node_ref(node_type)?;
            let node = optic_ref.optical_ref.lock_opm()?;
            if node.node_attr().get_property(AMP_CONFIG).is_ok() {
                declaring_amp_config.push(node_type);
            }
            if node.node_attr().get_property(CLEAR_APERTURE).is_ok() {
                declaring_clear_aperture.push(node_type);
            }
        }
        declaring_amp_config.sort_unstable();
        declaring_clear_aperture.sort_unstable();
        assert_eq!(
            declaring_amp_config, AMP_CONFIG_NODE_TYPES,
            "AMP_CONFIG_NODE_TYPES does not match the node types actually declaring '{AMP_CONFIG}'"
        );
        assert_eq!(
            declaring_clear_aperture, AMP_CONFIG_NODE_TYPES,
            "a node with a volume declares both '{AMP_CONFIG}' and '{CLEAR_APERTURE}', but these \
             node types declare only one of them"
        );
        Ok(())
    }
    #[test]
    fn active_amp_model_reports_only_active_models() -> OpmResult<()> {
        // A node without a volume never declares the property at all.
        assert_eq!(active_amp_model(Dummy::default().node_attr()), None);

        let mut lens = Lens::default();
        assert_eq!(active_amp_model(lens.node_attr()), None);
        lens.node_attr_mut()
            .set_property(AMP_CONFIG, GainModel::Const(ConstGain::new(3.0)?).into())?;
        assert_eq!(active_amp_model(lens.node_attr()).as_deref(), Some("Const"));
        Ok(())
    }
    #[test]
    fn default_is_inactive() {
        assert_eq!(GainModel::default(), GainModel::None);
        assert!(!GainModel::default().is_active());
        assert!(GainModel::Const(ConstGain::default()).is_active());
        assert_eq!(GainModel::None.active_name(), None);
        assert_eq!(
            GainModel::Const(ConstGain::default())
                .active_name()
                .as_deref(),
            Some("Const")
        );
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
