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
    gain::inversion_field::InversionField,
    geometry::body::Body,
    light::Spectrum,
};
use nalgebra::Point3;
use uom::si::f64::Length;

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
    /// The pumped inversion field, or `None` if this model does not read one.
    ///
    /// # Errors
    ///
    /// This function errors if the grid cannot be laid over the body or the medium cannot be
    /// pumped.
    fn build_inversion(
        &self,
        body: &dyn Body,
        config: &PumpConfig,
    ) -> OpmResult<Option<InversionField>>;
    /// Number of z-march steps this model uses per chord through the medium.
    ///
    /// [`volumetric::Volumetric::propagate_inside_medium`](crate::core_optics::volumetric::Volumetric) divides
    /// the chord a ray travels inside the medium into this many equal segments and calls
    /// [`gain_exponent_at`](Self::gain_exponent_at) once per segment. A model with no geometric
    /// sensitivity — a constant factor — returns 1, so one evaluation covers the full chord.
    ///
    /// # Returns
    ///
    /// Number of integration steps; must be non-zero.
    fn n_steps(&self) -> usize;

    /// Gain exponent contributed by one z-march segment through the medium.
    ///
    /// [`volumetric::Volumetric::propagate_inside_medium`](crate::core_optics::volumetric::Volumetric)
    /// accumulates the return values of all segments and exponentiates the sum to obtain the gain
    /// factor for the whole chord: `factor = exp(Σ gain_exponent_at(…))`.
    ///
    /// `inversion` is `&mut Option<InversionField>` so that saturating models can deplete the field
    /// between substeps; the [`Option`] layer stays because models such as [`ConstGain`](super::ConstGain)
    /// never build an inversion.
    ///
    /// # Arguments
    ///
    /// * `local` - midpoint of the segment in the body's local coordinate frame.
    /// * `step_width` - arc length of the segment.
    /// * `inversion` - the inversion field the node was prepared with, mutable so that saturating
    ///   models can write depletion back. `None` if this model built no field.
    ///
    /// # Returns
    ///
    /// The dimensionless exponent contribution `g · Δz` for this segment.
    /// Returns `0.0` for a segment that contributes nothing — outside the grid, outside the medium,
    /// or no field available.
    fn gain_exponent_at(
        &self,
        local: &Point3<Length>,
        step_width: Length,
        inversion: &mut Option<InversionField>,
    ) -> f64;
    /// Amplify the spectral energy passing through the medium.
    ///
    /// An energy flow analysis knows no rays and no path lengths, so a model depending on them has
    /// to decide here what it does instead - state a nominal path, or refuse. It cannot stay silent:
    /// amplifying by nothing would report a pumped chain as passive.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume the light passes through.
    /// * `inversion` - the inversion field the node was prepared with, or `None` if this model
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
        inversion: Option<&InversionField>,
        spectrum: &mut Spectrum,
    ) -> OpmResult<()>;
}
