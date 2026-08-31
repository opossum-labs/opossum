#![warn(missing_docs)]
//! What every gain model has to answer, and what it is given to answer it with.
//!
//! [`Extraction`] is the per-model contract. It exists so that the code specific to one gain model
//! lives with that model rather than accumulating as `match` arms in the volume machinery every
//! component shares: a node passing light through a medium asks the operating point for an
//! [`Extraction`] and hands it the prepared body and inversion, without knowing or caring which
//! model it got.
//!
//! **The trait deliberately has no default methods.** The matches it replaces were exhaustive on
//! purpose — a model that cannot be evaluated from "the beam was in here" alone has to *state* what
//! it does instead, and a default would silently answer for it. Every question below is therefore
//! one a new stage of the gain modelling must consciously answer before it compiles.

use super::scenario::PumpConfig;
use crate::{
    error::OpmResult,
    gain::inversion_field::Inversion,
    geometry::body::Body,
    light::{Ray, Spectrum},
};

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
    /// interface that has no component in front of it; [`Extraction::build_inversion`] is where the
    /// field is actually built.
    fn needs_inversion(&self) -> bool;
    /// Prepare the inversion of the given body, ready for this model to extract from.
    ///
    /// A model states here what it needs deposited in the medium and, for a shaped pump, how finely
    /// to resolve it. The result is an [`Inversion`]: a shaped pump yields
    /// [`Inversion::Field`], a uniform one [`Inversion::Uniform`] with no grid at all, and an
    /// unpumped medium `Uniform(0.0)` — a state a model can still read (as transparency, or, later,
    /// as absorption). Answering `None` means the model reads no inversion at all and no grid is
    /// laid out for it.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume of the medium.
    /// * `config` - the operating point of the node, whose
    ///   [`PumpSource`](super::PumpSource) shapes the inversion.
    ///
    /// # Returns
    ///
    /// The prepared inversion, or `None` if this model does not read one.
    ///
    /// # Errors
    ///
    /// This function errors if the grid cannot be laid over the body or the medium cannot be
    /// pumped.
    fn build_inversion(&self, body: &dyn Body, config: &PumpConfig)
    -> OpmResult<Option<Inversion>>;
    /// Gain exponent accumulated along the whole chord a ray travels through the medium.
    ///
    /// [`volumetric::Volumetric::propagate_inside_medium`](crate::core_optics::volumetric::Volumetric)
    /// calls this once per ray that passes through the medium and exponentiates the result to get
    /// the gain factor: `factor = exp(path_exponent(…))`.
    ///
    /// Shaped-pump models implement this by walking the ray through the
    /// [`InversionField`](super::InversionField) cell by cell (Amanatides–Woo exact traversal) and
    /// summing `g_cell · Δs_cell` per cell, where `Δs_cell` is the exact arc length the ray spends
    /// inside that cell; a [`Inversion::Uniform`] one integrates over the exact chord through the
    /// body without a grid. A path-independent model — a constant factor — returns `ln(gain)` and
    /// ignores the geometry.
    ///
    /// `inversion` is `&mut Option<Inversion>` so that saturating models can deplete the inversion
    /// as the ray passes through; the [`Option`] layer stays because models such as
    /// [`ConstGain`](super::ConstGain) never read an inversion at all.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume the ray travels through; its isometry converts global ray coordinates
    ///   into the inversion field's own local frame.
    /// * `ray` - the ray whose chord is being integrated; must be a valid ray (the caller has
    ///   already confirmed it is valid and that its chord through the body is positive).
    /// * `inversion` - the inversion prepared by [`Extraction::build_inversion`], mutable so
    ///   that saturating models can write depletion back.
    ///
    /// # Returns
    ///
    /// The dimensionless exponent `∫ g(s) ds` for the full chord. Returns `0.0` for a ray that
    /// contributes nothing — a degenerate direction, no inversion, or no populated cells on the path.
    fn path_exponent(&self, body: &dyn Body, ray: &Ray, inversion: &mut Option<Inversion>) -> f64;
    /// Amplify the spectral energy passing through the medium.
    ///
    /// An energy flow analysis knows no rays and no path lengths, so a model depending on them has
    /// to decide here what it does instead - state a nominal path, or refuse. It cannot stay silent:
    /// amplifying by nothing would report a pumped chain as passive.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume the light passes through.
    /// * `inversion` - the inversion the node was prepared with, or `None` if this model
    ///   built none.
    /// * `spectrum` - the spectral energy arriving at the node, modified in place.
    ///
    /// # Errors
    ///
    /// This function errors if the spectrum cannot be scaled, or if this model cannot be evaluated
    /// without a beam path at all.
    fn amplify_spectrum(
        &self,
        body: &dyn Body,
        inversion: Option<&Inversion>,
        spectrum: &mut Spectrum,
    ) -> OpmResult<()>;
}
