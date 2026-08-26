#![warn(missing_docs)]
//! What every gain model has to answer, and what it is given to answer it with.
//!
//! [`Extraction`] is the per-model contract. It exists so that the code specific to one gain model
//! lives with that model rather than accumulating as `match` arms in the volume machinery every
//! component shares: a node passing light through a medium asks the operating point for an
//! [`Extraction`] and hands it a [`Medium`], without knowing or caring which model it got.
//!
//! **The trait deliberately has no default methods.** The matches it replaces were exhaustive on
//! purpose — a model that cannot be evaluated from "the beam was in here" alone has to *state* what
//! it does instead, and a default would silently answer for it. Every question below is therefore
//! one a new stage of the gain modelling must consciously answer before it compiles.

use super::scenario::PumpConfig;
use crate::{
    error::{OpmResult, OpossumError},
    gain::inversion_field::InversionField,
    geometry::body::Body,
    light::{Rays, Spectrum},
};

/// The active medium a [`GainModel`](super::GainModel) is evaluated in.
///
/// Everything a model may look at, collected into one object. It is a struct of its own rather than
/// the node itself so that a gain model never reaches into the node API: what a model is allowed to
/// know about the component it sits in is exactly what is listed here, and widening that is a
/// deliberate change to this type rather than an accident of what happened to be in scope.
pub struct Medium<'a> {
    body: Option<&'a dyn Body>,
    field: Option<&'a InversionField>,
    node_name: &'a str,
}

impl<'a> Medium<'a> {
    /// Create a new [`Medium`] over a body.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume the light passes through.
    /// * `field` - its inversion, if the model asked for one via [`Extraction::pumped_medium`].
    /// * `node_name` - the name of the component, for diagnostics.
    #[must_use]
    pub const fn new(
        body: &'a dyn Body,
        field: Option<&'a InversionField>,
        node_name: &'a str,
    ) -> Self {
        Self {
            body: Some(body),
            field,
            node_name,
        }
    }
    /// Create a [`Medium`] a model reads nothing from - neither geometry nor inversion.
    ///
    /// What a model working from its own parameters alone (a constant factor) is handed. Building
    /// the body would resolve the node's ports - a whole [`OpticPorts`](crate::core_optics::OpticPorts)
    /// clone - for a value it never touches, so it is not built at all.
    ///
    /// # Arguments
    ///
    /// * `node_name` - the name of the component, for diagnostics.
    #[must_use]
    pub const fn passive(node_name: &'a str) -> Self {
        Self {
            body: None,
            field: None,
            node_name,
        }
    }
    /// Return the volume the light passes through.
    ///
    /// # Returns
    ///
    /// The body this medium was built over.
    ///
    /// # Errors
    ///
    /// This function errors if the medium was built passive (see [`Medium::passive`]), which means
    /// a model reading the geometry was handed a medium built for one that does not - an
    /// inconsistency within that model, not something a user can cause.
    pub fn body(&self) -> OpmResult<&'a dyn Body> {
        self.body.ok_or_else(|| {
            OpossumError::Analysis(format!(
                "the medium of node '{}' was not built with its geometry, but its gain model reads \
                 it - a model reading the medium has to answer `needs_inversion` with true",
                self.node_name
            ))
        })
    }
    /// Return the name of the component this medium belongs to.
    #[must_use]
    pub const fn node_name(&self) -> &'a str {
        self.node_name
    }
    /// Return the inversion stored in this medium.
    ///
    /// # Returns
    ///
    /// The field the model's own [`Extraction::pumped_medium`] produced.
    ///
    /// # Errors
    ///
    /// This function errors if there is no field, which means the model asked for none and is now
    /// reading one anyway - an inconsistency within that model, not something a user can cause.
    pub fn field(&self) -> OpmResult<&'a InversionField> {
        self.field.ok_or_else(|| {
            OpossumError::Analysis(format!(
                "the medium of node '{}' carries no inversion field, but its gain model reads one - \
                 the model has to build it in `pumped_medium`",
                self.node_name
            ))
        })
    }
}

/// How one gain model draws energy out of an active medium.
///
/// Implemented by the payload of each [`GainModel`](super::GainModel) variant, which is what keeps
/// a model's code in the model's own module. See the [module documentation](self) for why none of
/// these methods has a default.
pub trait Extraction {
    /// The name this model is shown and selected under.
    ///
    /// It names the *variant*, not the parameters - it is what
    /// [`Display`](std::fmt::Display) for [`GainModel`](super::GainModel) prints and what
    /// [`DefaultFromName`](crate::utils::default_from_name::DefaultFromName) recreates the variant
    /// from, so it has to stay stable across releases: an `.opm` file names it.
    fn name(&self) -> &'static str;
    /// Whether this model draws on the inversion stored in the medium.
    ///
    /// A model working from its own parameters alone - a fixed factor, say - does not care how the
    /// medium was pumped, or whether it was pumped at all, so offering pump settings for it would
    /// be offering a setting without an effect. This is the *static* question, asked by a user
    /// interface that has no component in front of it; [`Extraction::pumped_medium`] is where the
    /// field is actually built.
    fn needs_inversion(&self) -> bool;
    /// Discretise and pump the given body, ready for this model to extract from.
    ///
    /// A model states here how finely it wants the medium resolved and what it needs deposited in
    /// it, because both depend on what the model is going to do with it. Answering `None` means the
    /// model reads no inversion at all and no grid is laid out for it.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume of the medium.
    /// * `config` - the operating point of the node, whose
    ///   [`PumpSource`](super::PumpSource) fills the field.
    ///
    /// # Returns
    ///
    /// The pumped medium, or `None` if this model does not read one.
    ///
    /// # Errors
    ///
    /// This function errors if the grid cannot be laid over the body or the medium cannot be
    /// pumped.
    fn pumped_medium(
        &self,
        body: &dyn Body,
        config: &PumpConfig,
    ) -> OpmResult<Option<InversionField>>;
    /// Amplify a ray bundle crossing the medium.
    ///
    /// The rays sit **on the entrance surface** with the direction they were refracted into, and
    /// this must not move them: they are carried to the exit surface afterwards, exactly as through
    /// a passive component.
    ///
    /// # Arguments
    ///
    /// * `medium` - the active medium being crossed.
    /// * `rays_bundle` - the rays inside it, modified in place.
    ///
    /// # Errors
    ///
    /// This function errors if the resulting ray energies would not be finite.
    fn amplify_rays(&self, medium: &Medium<'_>, rays_bundle: &mut [Rays]) -> OpmResult<()>;
    /// Amplify the spectral energy passing through the medium.
    ///
    /// An energy flow analysis knows no rays and no path lengths, so a model depending on them has
    /// to decide here what it does instead - state a nominal path, or refuse. It cannot stay silent:
    /// amplifying by nothing would report a pumped chain as passive.
    ///
    /// # Arguments
    ///
    /// * `medium` - the active medium being crossed.
    /// * `spectrum` - the spectral energy arriving at the node, modified in place.
    ///
    /// # Errors
    ///
    /// This function errors if the spectrum cannot be scaled, or if this model cannot be evaluated
    /// without a beam path at all.
    fn amplify_spectrum(&self, medium: &Medium<'_>, spectrum: &mut Spectrum) -> OpmResult<()>;
}
