#![warn(missing_docs)]
//! What puts inversion into an active medium: the pump side of the amplification.
//!
//! A [`PumpSource`] is the exact counterpart of a [`GainModel`](super::GainModel). One writes into
//! an [`InversionField`], the other reads out of it, and the field is the only thing the two share —
//! neither has to know what the other is. That is what lets a pumping scheme and an extraction model
//! be chosen independently of each other, and it is why a solver computing the inversion from a real
//! pump beam will later be able to slot in here without any of the extraction side changing.
//!
//! Like the gain model, a pump source belongs to the **operating point** rather than to the
//! component: which lens is a lens is a property of the model, how hard it is pumped is a property
//! of the run being analyzed. Both therefore live in a
//! [`PumpScenario`](super::PumpScenario).
//!
//! **A pump source is stated as a small-signal gain coefficient g₀**, not as an inversion density.
//! That is the number an amplifier is actually specified in — `G = exp(g₀·L)` for a single pass
//! through a length `L` — and the inversion is implicit in it. The field stores the density, though,
//! because that is what does not depend on a wavelength, so [`PumpSource::deposit`] converts between
//! the two through `g₀ = σ_e · ΔN`. The emission cross section needed for that is an *argument*: see
//! there for why it cannot be read off the medium yet.

use super::inversion_field::{InversionField, cells};
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllPositive, ValidateTrait},
    geometry::body::BoundingBox,
    reciprocal_centimeter,
    utils::{default_from_name::DefaultFromName, super_gaussian::SuperGaussianShape},
    validated, validated_type,
};
use nalgebra::{Point2, Point3};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Range};
use strum::EnumIter;
use uom::si::f64::{Area, Length, ReciprocalLength, VolumetricNumberDensity};
use utoipa::ToSchema;

/// Deserialization shim for [`ConstInversion`].
///
/// It lets a coefficient read from an `.opm` file run through the very same validation as one set
/// through [`ConstInversion::set_gain_coefficient`], so a hand-edited file cannot smuggle in a
/// non-finite value. Same pattern as [`ConstGain`](super::ConstGain).
#[derive(Deserialize)]
struct NonValidatedConstInversion {
    gain_coefficient: ReciprocalLength,
}
impl TryFrom<NonValidatedConstInversion> for ConstInversion {
    type Error = String;
    fn try_from(helper: NonValidatedConstInversion) -> Result<Self, Self::Error> {
        Self::new(helper.gain_coefficient).map_err(|e| e.to_string())
    }
}

/// A small-signal gain coefficient that is guaranteed to be finite.
///
/// Deliberately **not** constrained to be positive. A negative coefficient describes a medium that
/// absorbs where an amplifier would amplify — the same physics with the inversion turned around, and
/// the state an unpumped doped medium is actually in.
type ValidatedGainCoefficient = validated_type!(ReciprocalLength, AllFinite);
impl Default for ValidatedGainCoefficient {
    /// No gain at all, i.e. an unpumped medium.
    fn default() -> Self {
        validated!(reciprocal_centimeter!(0.0), AllFinite).unwrap()
    }
}

/// Parameters of a medium pumped uniformly throughout.
///
/// The simplest conceivable pump: the same inversion everywhere inside the body, with no transversal
/// profile and no decay along the pump axis. It is what one assumes for a first estimate of an
/// amplifier chain, and it is the reference every shaped profile is compared against.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated)]
#[serde(try_from = "NonValidatedConstInversion")]
pub struct ConstInversion {
    #[schema(value_type = f64)]
    gain_coefficient: ValidatedGainCoefficient,
}
impl Default for ConstInversion {
    /// Create an unpumped [`ConstInversion`] with a gain coefficient of zero.
    ///
    /// Choosing this variant must not change a result on its own — the medium starts out as passive
    /// as it was, and only entering a coefficient pumps it.
    fn default() -> Self {
        Self {
            gain_coefficient: ValidatedGainCoefficient::default(),
        }
    }
}
impl ConstInversion {
    /// Create a new [`ConstInversion`] with the given small-signal gain coefficient.
    ///
    /// # Arguments
    ///
    /// * `gain_coefficient` - g₀, the gain per unit length the medium provides. Zero leaves it
    ///   passive, a negative value makes it absorbing. Total small-signal gain relates as G₀ = exp(g₀ x l)
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the given coefficient is not finite.
    pub fn new(gain_coefficient: ReciprocalLength) -> OpmResult<Self> {
        let mut inversion = Self::default();
        inversion.set_gain_coefficient(gain_coefficient)?;
        Ok(inversion)
    }
    /// Return the small-signal gain coefficient.
    #[must_use]
    pub const fn gain_coefficient(&self) -> ReciprocalLength {
        *self.gain_coefficient.get()
    }
    /// Set the small-signal gain coefficient.
    ///
    /// # Arguments
    ///
    /// * `gain_coefficient` - g₀, the gain per unit length the medium provides.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the given coefficient is not finite. The previous value
    /// is kept in that case.
    pub fn set_gain_coefficient(&mut self, gain_coefficient: ReciprocalLength) -> OpmResult<()> {
        self.gain_coefficient.set(gain_coefficient)
    }
}
impl From<ConstInversion> for PumpSource {
    fn from(value: ConstInversion) -> Self {
        Self::Const(value)
    }
}
impl From<ConstInversion> for AnalyticPump {
    /// A uniformly pumped medium *is* the analytic profile with no shape at all.
    ///
    /// Stating it that way is what lets [`PumpSource::deposit`] evaluate both over one and the same
    /// grid walk. It cannot fail: both hold the coefficient in the same validated type, so the value
    /// is known to be good already.
    fn from(value: ConstInversion) -> Self {
        Self {
            peak_gain_coefficient: value.gain_coefficient,
            transversal: TransversalProfile::Flat,
            longitudinal: LongitudinalProfile::Flat,
        }
    }
}

/// Which end of the medium the pump enters through.
///
/// The two ends of the same optical axis the light itself travels along, so this is stated in the
/// optic's own frame rather than as a direction in the laboratory.
#[derive(
    Default, Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq, EnumIter,
)]
pub enum PumpDirection {
    /// The pump enters at the entrance face and is absorbed on its way towards the exit.
    #[default]
    Forward,
    /// The pump enters at the exit face and travels against the optical axis.
    Backward,
}
impl Display for PumpDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Forward => write!(f, "Forward"),
            Self::Backward => write!(f, "Backward"),
        }
    }
}
impl DefaultFromName for PumpDirection {}

/// Deserialization shim for [`BeerLambertProfile`], mirroring [`NonValidatedConstInversion`].
#[derive(Deserialize)]
struct NonValidatedBeerLambertProfile {
    absorption: ReciprocalLength,
    direction: PumpDirection,
}
impl TryFrom<NonValidatedBeerLambertProfile> for BeerLambertProfile {
    type Error = String;
    fn try_from(helper: NonValidatedBeerLambertProfile) -> Result<Self, Self::Error> {
        Self::new(helper.absorption, helper.direction).map_err(|e| e.to_string())
    }
}

/// An absorption coefficient that is guaranteed to be finite and not negative.
///
/// Unlike a gain coefficient this may not be negative: a pump beam growing stronger as it travels
/// through the medium it is being absorbed by is not the same physics turned around, it is no
/// physics at all.
type ValidatedAbsorptionCoefficient = validated_type!(ReciprocalLength, AllFinite && AllPositive);
impl Default for ValidatedAbsorptionCoefficient {
    /// A medium the pump passes through undiminished.
    fn default() -> Self {
        validated!(reciprocal_centimeter!(0.0), AllFinite && AllPositive).unwrap()
    }
}

/// A pump absorbed exponentially along the optical axis of the medium.
///
/// The Beer-Lambert law: a pump entering one face is attenuated by `exp(-α·s)` after a depth `s`,
/// so the inversion it leaves behind decays the same way. This is what makes one end of an
/// end-pumped rod hotter and more strongly inverted than the other.
#[derive(
    Default, Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated,
)]
#[serde(try_from = "NonValidatedBeerLambertProfile")]
pub struct BeerLambertProfile {
    #[schema(value_type = f64)]
    absorption: ValidatedAbsorptionCoefficient,
    #[validate(skip)]
    direction: PumpDirection,
}
impl BeerLambertProfile {
    /// Create a new [`BeerLambertProfile`].
    ///
    /// # Arguments
    ///
    /// * `absorption` - α, the absorption coefficient of the medium at the pump wavelength. Zero
    ///   leaves the pump undiminished, which is the flat profile.
    /// * `direction` - which face the pump enters through.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the coefficient is not finite or is negative.
    pub fn new(absorption: ReciprocalLength, direction: PumpDirection) -> OpmResult<Self> {
        let mut profile = Self::default();
        profile.set_absorption(absorption)?;
        profile.direction = direction;
        Ok(profile)
    }
    /// Set the absorption coefficient.
    ///
    /// # Arguments
    ///
    /// * `absorption` - α, the absorption coefficient at the pump wavelength.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the coefficient is not finite or is negative. The
    /// previous value is kept in that case.
    pub fn set_absorption(&mut self, absorption: ReciprocalLength) -> OpmResult<()> {
        self.absorption.set(absorption)
    }
    /// Return the share of the pump still left at the given position.
    ///
    /// # Arguments
    ///
    /// * `position` - the longitudinal position in the optic's frame.
    /// * `extent` - the longitudinal extent of the medium, whose ends are the two faces the pump can
    ///   enter through.
    ///
    /// # Returns
    ///
    /// The attenuation there, 1 at the face the pump enters through and falling off behind it.
    fn value_at(&self, position: Length, extent: &Range<Length>) -> f64 {
        let depth = match self.direction {
            PumpDirection::Forward => position - extent.start,
            PumpDirection::Backward => extent.end - position,
        };
        f64::exp(-(*self.absorption.get() * depth).value)
    }
}

/// How a pump is distributed across the medium, transversally.
#[derive(
    Default,
    Serialize,
    Deserialize,
    ToSchema,
    Debug,
    Clone,
    Copy,
    PartialEq,
    EnumIter,
    EnsureValidated,
)]
#[non_exhaustive]
pub enum TransversalProfile {
    /// The pump covers the whole cross section evenly.
    #[default]
    Flat,
    /// A super-Gaussian spot, which may be decentred, elliptical, rotated or flat-topped. See
    /// [`SuperGaussianShape`].
    SuperGaussian(SuperGaussianShape),
}
impl Display for TransversalProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flat => write!(f, "Flat"),
            Self::SuperGaussian(_) => write!(f, "SuperGaussian"),
        }
    }
}
impl DefaultFromName for TransversalProfile {}
impl TransversalProfile {
    /// Return the share of the peak this profile reaches at the given transversal position.
    ///
    /// # Arguments
    ///
    /// * `position` - the transversal position in the optic's frame.
    ///
    /// # Returns
    ///
    /// The value of the profile there, at most 1.
    fn value_at(&self, position: &Point2<Length>) -> f64 {
        match self {
            Self::Flat => 1.0,
            Self::SuperGaussian(shape) => shape.value_at(position),
        }
    }
}

/// How a pump is distributed along the optical axis of the medium.
#[derive(
    Default,
    Serialize,
    Deserialize,
    ToSchema,
    Debug,
    Clone,
    Copy,
    PartialEq,
    EnumIter,
    EnsureValidated,
)]
#[non_exhaustive]
pub enum LongitudinalProfile {
    /// The pump reaches the far end of the medium as strongly as the near one.
    #[default]
    Flat,
    /// The pump is absorbed on its way through. See [`BeerLambertProfile`].
    BeerLambert(BeerLambertProfile),
}
impl Display for LongitudinalProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flat => write!(f, "Flat"),
            Self::BeerLambert(_) => write!(f, "BeerLambert"),
        }
    }
}
impl DefaultFromName for LongitudinalProfile {}
impl LongitudinalProfile {
    /// Return the share of the peak this profile reaches at the given longitudinal position.
    ///
    /// # Arguments
    ///
    /// * `position` - the longitudinal position in the optic's frame.
    /// * `extent` - the longitudinal extent of the medium.
    ///
    /// # Returns
    ///
    /// The value of the profile there, at most 1.
    fn value_at(&self, position: Length, extent: &Range<Length>) -> f64 {
        match self {
            Self::Flat => 1.0,
            Self::BeerLambert(profile) => profile.value_at(position, extent),
        }
    }
}

/// Deserialization shim for [`AnalyticPump`], mirroring [`NonValidatedConstInversion`].
#[derive(Deserialize)]
struct NonValidatedAnalyticPump {
    peak_gain_coefficient: ReciprocalLength,
    transversal: TransversalProfile,
    longitudinal: LongitudinalProfile,
}
impl TryFrom<NonValidatedAnalyticPump> for AnalyticPump {
    type Error = String;
    fn try_from(helper: NonValidatedAnalyticPump) -> Result<Self, Self::Error> {
        Self::new(
            helper.peak_gain_coefficient,
            helper.transversal,
            helper.longitudinal,
        )
        .map_err(|e| e.to_string())
    }
}

/// Parameters of a medium pumped into a shape given in closed form.
///
/// The two profiles are **composed**, not chosen between: a real end-pumped rod has a spot profile
/// across its face *and* an absorption decay along its axis at the same time, and stating them
/// separately is what lets either be varied without touching the other.
///
/// Both profiles are peak-normalised, so their product reaches 1 exactly where they both peak — on
/// the axis of the spot, at the face the pump enters through — and the coefficient below is what the
/// medium provides there. If that point happens to lie outside the body, the medium simply never
/// reaches the stated peak; the parameter describes the profile, not the outcome.
#[derive(
    Default, Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated,
)]
#[serde(try_from = "NonValidatedAnalyticPump")]
pub struct AnalyticPump {
    #[schema(value_type = f64)]
    peak_gain_coefficient: ValidatedGainCoefficient,
    #[validate(skip)]
    transversal: TransversalProfile,
    #[validate(skip)]
    longitudinal: LongitudinalProfile,
}
impl AnalyticPump {
    /// Create a new [`AnalyticPump`].
    ///
    /// # Arguments
    ///
    /// * `peak_gain_coefficient` - g₀ at the peak of the combined profile.
    /// * `transversal` - how the pump is distributed across the cross section.
    /// * `longitudinal` - how it is distributed along the optical axis.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the coefficient is not finite.
    pub fn new(
        peak_gain_coefficient: ReciprocalLength,
        transversal: TransversalProfile,
        longitudinal: LongitudinalProfile,
    ) -> OpmResult<Self> {
        let mut pump = Self {
            transversal,
            longitudinal,
            ..Self::default()
        };
        pump.set_peak_gain_coefficient(peak_gain_coefficient)?;
        Ok(pump)
    }
    /// Return the small-signal gain coefficient at the peak of the profile.
    #[must_use]
    pub const fn peak_gain_coefficient(&self) -> ReciprocalLength {
        *self.peak_gain_coefficient.get()
    }
    /// Set the small-signal gain coefficient at the peak of the profile.
    ///
    /// # Arguments
    ///
    /// * `peak_gain_coefficient` - g₀ at the peak of the combined profile.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`] if the coefficient is not finite. The previous value is
    /// kept in that case.
    pub fn set_peak_gain_coefficient(
        &mut self,
        peak_gain_coefficient: ReciprocalLength,
    ) -> OpmResult<()> {
        self.peak_gain_coefficient.set(peak_gain_coefficient)
    }
    /// Return the share of the peak this pump reaches at the given position in the medium.
    ///
    /// # Arguments
    ///
    /// * `position` - the position in the optic's frame.
    /// * `bounds` - the extent of the medium, which the longitudinal profile measures its depth
    ///   from.
    ///
    /// # Returns
    ///
    /// The product of the two profiles there, at most 1.
    fn weight_at(&self, position: &Point3<Length>, bounds: &BoundingBox) -> f64 {
        self.transversal
            .value_at(&Point2::new(position.x, position.y))
            * self.longitudinal.value_at(position.z, &bounds.z_range())
    }
}
impl From<AnalyticPump> for PumpSource {
    fn from(value: AnalyticPump) -> Self {
        Self::Analytic(value)
    }
}

/// How the medium of a node is pumped.
///
/// Each way of describing a pumped medium adds one variant here rather than a node type of its own,
/// exactly as the extraction side does with [`GainModel`](super::GainModel).
#[derive(Default, Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnumIter)]
#[non_exhaustive]
pub enum PumpSource {
    /// Not pumped at all. The medium keeps whatever inversion it already had, which for a fresh
    /// [`InversionField`] is none.
    ///
    /// This is the default, so a component nobody pumped behaves exactly as it always did.
    #[default]
    None,
    /// A uniform inversion throughout the medium. See [`ConstInversion`].
    Const(ConstInversion),
    /// An inversion shaped by profiles given in closed form. See [`AnalyticPump`].
    Analytic(AnalyticPump),
}
impl Display for PumpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Const(_) => write!(f, "Const"),
            Self::Analytic(_) => write!(f, "Analytic"),
        }
    }
}
impl DefaultFromName for PumpSource {}
impl PumpSource {
    /// Return whether this source pumps at all.
    ///
    /// Used to decide whether a node has to be treated as pumped, e.g. when an operating point
    /// stores only the entries that actually do something.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
    /// Add the inversion this source produces to the given [`InversionField`].
    ///
    /// The inversion is **added** to whatever the field already holds rather than replacing it. On a
    /// fresh field the two are the same thing, but adding is what lets several sources pump the very
    /// same medium, and it is the shape the reverse operation needs — an extraction, or a pump
    /// solver depositing energy, is this with the opposite sign.
    ///
    /// Only cells that hold medium are written; the rest of the grid spans the body's bounding box
    /// but not the body itself, and there is nothing out there to excite.
    ///
    /// # Arguments
    ///
    /// * `field` - the field to pump, already laid out over the medium.
    /// * `emission_cross_section` - `σ_e` of the medium at the laser wavelength. It is needed to turn
    ///   the gain coefficient this source is stated in into the inversion density the field stores,
    ///   and it is an argument rather than something read off the medium because
    ///   [`Material`](crate::material::Material) carries no spectroscopic data yet. Once it does,
    ///   this is the one place that has to start asking it.
    ///
    /// # Errors
    ///
    /// This function errors if the cross section is not finite and positive, or if the resulting
    /// inversion density is not finite.
    pub fn deposit(
        &self,
        field: &mut InversionField,
        emission_cross_section: Area,
    ) -> OpmResult<()> {
        // Matched exhaustively on purpose: a source added later has to state here how it is shaped
        // over the grid, rather than falling into a catch-all arm that would silently pump nothing.
        // A uniform source is the shapeless case of a profile, so both leave here as one and the
        // grid is walked once below rather than once per variant.
        let profile = match self {
            Self::None => return Ok(()),
            Self::Const(constant) => AnalyticPump::from(*constant),
            Self::Analytic(analytic) => *analytic,
        };
        let peak = inversion_from_gain(profile.peak_gain_coefficient(), emission_cross_section)?;
        let bounds = field.bounds();
        for cell in cells(field.dimensions()) {
            if !field.is_inside(cell) {
                continue;
            }
            let (Some(center), Some(present)) = (field.cell_center(cell), field.population(cell))
            else {
                continue;
            };
            let deposited = peak * profile.weight_at(&center, &bounds);
            if !deposited.is_finite() {
                return Err(OpossumError::Other(format!(
                    "pumping cell {cell:?} would leave it at an inversion that is not finite"
                )));
            }
            field.set_population(cell, present + deposited)?;
        }
        Ok(())
    }
}

/// Convert a small-signal gain coefficient into the inversion density producing it.
///
/// `g₀ = σ_e · ΔN` is what the gain coefficient *is*: the inversion, measured in how much
/// amplification it yields per unit length. Turning that around is what lets a pump source be stated
/// in the quantity an amplifier is specified in while the field keeps the density, which — unlike
/// the coefficient — does not depend on the wavelength the medium is looked at with.
///
/// # Arguments
///
/// - `gain_coefficient`: g₀, the gain per unit length. May be negative for an absorbing medium.
/// - `emission_cross_section`: `σ_e` of the medium at the laser wavelength.
///
/// # Returns
///
/// The inversion density that produces the given coefficient.
///
/// # Errors
///
/// This function returns an error if the cross section is not finite and positive — a medium that
/// cannot emit has no inversion that would explain a gain — or if the quotient is not finite.
fn inversion_from_gain(
    gain_coefficient: ReciprocalLength,
    emission_cross_section: Area,
) -> OpmResult<VolumetricNumberDensity> {
    let cross_section = emission_cross_section.value;
    if !cross_section.is_finite() || cross_section <= 0.0 {
        return Err(OpossumError::Other(format!(
            "an emission cross section of {cross_section} m^2 cannot relate a gain coefficient to \
             an inversion: it has to be finite and positive"
        )));
    }
    // `uom` keeps number densities in a kind of their own so that they cannot be confused with the
    // other quantities of dimension 1/length^3, which is why the quotient has to be moved into it
    // explicitly rather than simply being one.
    let inversion: VolumetricNumberDensity = (gain_coefficient / emission_cross_section).into();
    if inversion.is_finite() {
        Ok(inversion)
    } else {
        Err(OpossumError::Other(format!(
            "a gain coefficient of {} 1/m over a cross section of {cross_section} m^2 is not a \
             finite inversion density",
            gain_coefficient.value
        )))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        gain::inversion_field::CellIndex,
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        millimeter, num_per_cm3, square_centimeter,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use std::sync::{Arc, Mutex};
    use strum::IntoEnumIterator;
    use uom::si::{f64::Length, volumetric_number_density::per_cubic_centimeter};

    /// The emission cross section of a typical solid state gain medium, of the order Yb:YAG has.
    fn cross_section() -> Area {
        square_centimeter!(2.0e-20)
    }
    /// Create a disk of the given thickness and radius, sitting at the origin.
    fn disk(thickness: Length, radius: Length) -> OpmResult<SurfaceBoundedBody> {
        Ok(SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                thickness,
            )?)))),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                radius,
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        ))
    }
    /// The disk every field below is laid over: 10 mm thick, 5 mm in radius, sitting at the origin.
    ///
    /// Its bounding box therefore spans -5..5 mm transversally and 0..10 mm along the axis, which is
    /// what the expected values are worked out from.
    fn test_disk() -> OpmResult<SurfaceBoundedBody> {
        disk(millimeter!(10.0), millimeter!(5.0))
    }
    /// Create an unpumped field over the disk, coarse enough for its corners to lie outside the body.
    fn field_over_a_disk() -> OpmResult<InversionField> {
        InversionField::from_body(&test_disk()?, (8, 8, 4))
    }
    /// Create an unpumped field over the disk with an odd transversal size, so that one column of
    /// cells sits exactly on the optical axis.
    fn field_centered_on_the_axis() -> OpmResult<InversionField> {
        InversionField::from_body(&test_disk()?, (9, 9, 4))
    }
    /// Return the center of the given cell, or an error if it is not part of the grid.
    fn center_of(field: &InversionField, cell: CellIndex) -> OpmResult<Point3<Length>> {
        field
            .cell_center(cell)
            .ok_or_else(|| OpossumError::Other(format!("cell {cell:?} is not part of the grid")))
    }
    /// Return the inversion of the given cell in 1/cm^3, or an error if it is not part of the grid.
    fn inversion_at(field: &InversionField, cell: (usize, usize, usize)) -> OpmResult<f64> {
        field
            .population(cell)
            .map(|density| density.get::<per_cubic_centimeter>())
            .ok_or_else(|| OpossumError::Other(format!("cell {cell:?} is not part of the grid")))
    }
    #[test]
    fn a_constant_source_pumps_the_whole_medium_evenly() -> OpmResult<()> {
        let mut field = field_over_a_disk()?;
        let source = PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?);
        source.deposit(&mut field, cross_section())?;
        // 0.5 1/cm over 2e-20 cm^2 is an inversion of 2.5e19 1/cm^3, everywhere the medium reaches.
        for cell in cells(field.dimensions()) {
            let expected = if field.is_inside(cell) { 2.5e19 } else { 0.0 };
            assert_relative_eq!(inversion_at(&field, cell)?, expected, max_relative = 1e-12);
        }
        // ... and the mask really did leave something out, otherwise the check above is vacuous
        assert!(cells(field.dimensions()).any(|cell| !field.is_inside(cell)));
        Ok(())
    }
    #[test]
    fn a_larger_cross_section_needs_less_inversion_for_the_same_gain() -> OpmResult<()> {
        // The gain coefficient is what the user states, so doubling the cross section of the medium
        // means half the inversion produces it.
        let source = PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?);
        let mut weak = field_over_a_disk()?;
        let mut strong = field_over_a_disk()?;
        source.deposit(&mut weak, cross_section())?;
        source.deposit(&mut strong, cross_section() * 2.0)?;
        assert_relative_eq!(
            inversion_at(&weak, (3, 3, 0))?,
            2.0 * inversion_at(&strong, (3, 3, 0))?,
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn pumping_twice_adds_up() -> OpmResult<()> {
        // Depositing accumulates rather than overwrites, so a medium pumped from two sides ends up
        // with the sum - and an extraction can later subtract through the very same field.
        let mut field = field_over_a_disk()?;
        let source = PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?);
        source.deposit(&mut field, cross_section())?;
        let once = inversion_at(&field, (3, 3, 0))?;
        source.deposit(&mut field, cross_section())?;
        assert_relative_eq!(inversion_at(&field, (3, 3, 0))?, 2.0 * once);
        Ok(())
    }
    #[test]
    fn an_absent_source_leaves_the_field_alone() -> OpmResult<()> {
        let mut field = field_over_a_disk()?;
        let untouched = field.clone();
        PumpSource::None.deposit(&mut field, cross_section())?;
        assert_eq!(field, untouched);
        Ok(())
    }
    #[test]
    fn a_negative_coefficient_describes_an_absorbing_medium() -> OpmResult<()> {
        // An unpumped doped medium absorbs at the laser wavelength. That is the same physics with
        // the inversion turned around, which is why the coefficient is not constrained to be
        // positive and the field stores a signed density.
        let mut field = field_over_a_disk()?;
        let source = PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(-0.5))?);
        source.deposit(&mut field, cross_section())?;
        assert_relative_eq!(
            inversion_at(&field, (3, 3, 0))?,
            -2.5e19,
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn a_medium_that_cannot_emit_has_no_inversion_to_speak_of() -> OpmResult<()> {
        let mut field = field_over_a_disk()?;
        let source = PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?);
        for impossible in [
            square_centimeter!(0.0),
            square_centimeter!(-1.0e-20),
            square_centimeter!(f64::NAN),
            square_centimeter!(f64::INFINITY),
        ] {
            assert!(source.deposit(&mut field, impossible).is_err());
        }
        // A source that does not pump is not asked to convert anything, so it does not care.
        assert!(
            PumpSource::None
                .deposit(&mut field, square_centimeter!(0.0))
                .is_ok()
        );
        Ok(())
    }
    #[test]
    fn default_is_unpumped() -> OpmResult<()> {
        assert_eq!(PumpSource::default(), PumpSource::None);
        assert!(!PumpSource::default().is_active());
        assert!(PumpSource::Const(ConstInversion::default()).is_active());
        // A freshly selected `Const` must not pump on its own, so its default coefficient is zero.
        assert_relative_eq!(ConstInversion::default().gain_coefficient().value, 0.0);
        let mut field = field_over_a_disk()?;
        PumpSource::Const(ConstInversion::default()).deposit(&mut field, cross_section())?;
        assert_relative_eq!(inversion_at(&field, (3, 3, 0))?, 0.0);
        Ok(())
    }
    #[test]
    fn a_rejected_coefficient_keeps_the_old_value() -> OpmResult<()> {
        let mut inversion = ConstInversion::new(reciprocal_centimeter!(0.5))?;
        // A half-typed value in a user interface must leave the medium as it was.
        assert!(
            inversion
                .set_gain_coefficient(reciprocal_centimeter!(f64::NAN))
                .is_err()
        );
        assert_relative_eq!(
            inversion.gain_coefficient().value,
            reciprocal_centimeter!(0.5).value
        );
        assert!(ConstInversion::new(reciprocal_centimeter!(f64::INFINITY)).is_err());
        assert!(ConstInversion::new(reciprocal_centimeter!(f64::NEG_INFINITY)).is_err());
        Ok(())
    }
    #[test]
    fn fmt() -> OpmResult<()> {
        assert_eq!(format!("{}", PumpSource::None), "None");
        assert_eq!(
            format!("{}", PumpSource::Const(ConstInversion::default())),
            "Const"
        );
        assert_eq!(
            PumpSource::from(ConstInversion::new(reciprocal_centimeter!(0.5))?),
            PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?)
        );
        Ok(())
    }
    #[test]
    fn all_variants_are_reachable_by_name() {
        for variant in PumpSource::iter() {
            assert_eq!(
                PumpSource::default_from_name(&variant.to_string()),
                Some(variant),
                "variant {variant} cannot be recreated from its display name"
            );
        }
        assert_eq!(PumpSource::default_from_name("does not exist"), None);
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        for source in [
            PumpSource::None,
            PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(0.5))?),
            PumpSource::Const(ConstInversion::new(reciprocal_centimeter!(-0.5))?),
            PumpSource::Analytic(AnalyticPump::new(
                reciprocal_centimeter!(0.5),
                TransversalProfile::SuperGaussian(SuperGaussianShape::default()),
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    reciprocal_centimeter!(1.0),
                    PumpDirection::Backward,
                )?),
            )?),
        ] {
            let serialized =
                ron::to_string(&source).map_err(|e| OpossumError::Other(e.to_string()))?;
            let deserialized: PumpSource =
                ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(source, deserialized);
        }
        // A hand-edited file has to run through the same validation as a setter. The accepted case
        // is asserted alongside it so that the rejection is known to come from the value rather
        // than from a shape `ron` could not read in the first place - the two differ in nothing
        // else. The number is in the base unit, so 50 per meter is the 0.5 per centimeter above.
        assert!(ron::from_str::<PumpSource>("Const((gain_coefficient:50))").is_ok());
        assert!(ron::from_str::<PumpSource>("Const((gain_coefficient:NaN))").is_err());
        Ok(())
    }
    #[test]
    fn a_shapeless_analytic_pump_is_a_constant_one() -> OpmResult<()> {
        // Both profiles flat means no shape at all, which is exactly what `Const` describes - and
        // `deposit` really does walk them down the same path, so this has to come out identical.
        let coefficient = reciprocal_centimeter!(0.5);
        let mut shaped = field_over_a_disk()?;
        let mut uniform = field_over_a_disk()?;
        PumpSource::Analytic(AnalyticPump::new(
            coefficient,
            TransversalProfile::Flat,
            LongitudinalProfile::Flat,
        )?)
        .deposit(&mut shaped, cross_section())?;
        PumpSource::Const(ConstInversion::new(coefficient)?)
            .deposit(&mut uniform, cross_section())?;
        assert_eq!(shaped, uniform);
        Ok(())
    }
    #[test]
    fn a_super_gaussian_spot_peaks_on_the_axis() -> OpmResult<()> {
        let sigma = millimeter!(2.0, 2.0);
        let mut field = field_centered_on_the_axis()?;
        PumpSource::Analytic(AnalyticPump::new(
            reciprocal_centimeter!(0.5),
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                millimeter!(0.0, 0.0),
                sigma,
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,
        )?)
        .deposit(&mut field, cross_section())?;
        // The middle column of nine sits on the axis, where the profile is at its peak ...
        assert_relative_eq!(
            inversion_at(&field, (4, 4, 0))?,
            2.5e19,
            max_relative = 1e-12
        );
        // ... and every cell off it follows exp(-r^2 / 2 sigma^2), worked out at the cell's own
        // center rather than at a position the grid was chosen to make round.
        for cell in [(4, 6, 0), (6, 4, 0), (2, 2, 3)] {
            let center = center_of(&field, cell)?;
            let radial = center.x.value.hypot(center.y.value);
            let expected = 2.5e19 * f64::exp(-0.5 * (radial / sigma.x.value).powi(2));
            assert_relative_eq!(inversion_at(&field, cell)?, expected, max_relative = 1e-12);
        }
        Ok(())
    }
    #[test]
    fn a_decentred_spot_moves_the_maximum_with_it() -> OpmResult<()> {
        // The peak follows the center of the spot, so a spot put over one corner of the cross
        // section leaves the axis behind.
        let mut field = field_centered_on_the_axis()?;
        let off_axis = center_of(&field, (6, 6, 0))?;
        PumpSource::Analytic(AnalyticPump::new(
            reciprocal_centimeter!(0.5),
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                Point2::new(off_axis.x, off_axis.y),
                millimeter!(1.0, 1.0),
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,
        )?)
        .deposit(&mut field, cross_section())?;
        assert_relative_eq!(
            inversion_at(&field, (6, 6, 0))?,
            2.5e19,
            max_relative = 1e-12
        );
        assert!(inversion_at(&field, (4, 4, 0))? < inversion_at(&field, (6, 6, 0))?);
        Ok(())
    }
    #[test]
    fn beer_lambert_decays_along_the_axis() -> OpmResult<()> {
        let absorption = reciprocal_centimeter!(1.0);
        let mut field = field_over_a_disk()?;
        PumpSource::Analytic(AnalyticPump::new(
            reciprocal_centimeter!(0.5),
            TransversalProfile::Flat,
            LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                absorption,
                PumpDirection::Forward,
            )?),
        )?)
        .deposit(&mut field, cross_section())?;
        // Between the first and the last slice the pump has travelled the distance between their
        // centers, and it is attenuated by exactly the Beer-Lambert factor over it.
        let (near, far) = ((3, 3, 0), (3, 3, 3));
        let travelled = center_of(&field, far)?.z - center_of(&field, near)?.z;
        assert_relative_eq!(
            inversion_at(&field, far)? / inversion_at(&field, near)?,
            f64::exp(-(absorption * travelled).value),
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn pumping_from_the_other_end_mirrors_the_profile() -> OpmResult<()> {
        let make = |direction| -> OpmResult<InversionField> {
            let mut field = field_over_a_disk()?;
            PumpSource::Analytic(AnalyticPump::new(
                reciprocal_centimeter!(0.5),
                TransversalProfile::Flat,
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    reciprocal_centimeter!(1.0),
                    direction,
                )?),
            )?)
            .deposit(&mut field, cross_section())?;
            Ok(field)
        };
        let forward = make(PumpDirection::Forward)?;
        let backward = make(PumpDirection::Backward)?;
        let (_, _, slices) = forward.dimensions();
        for (i, j, k) in cells(forward.dimensions()) {
            assert_relative_eq!(
                inversion_at(&forward, (i, j, k))?,
                inversion_at(&backward, (i, j, slices - 1 - k))?,
                max_relative = 1e-12
            );
        }
        Ok(())
    }
    #[test]
    fn the_two_profiles_multiply() -> OpmResult<()> {
        // The point of composing them: an end-pumped rod has a spot across its face *and* an
        // absorption decay along its axis, and a cell sees the product of the two.
        let sigma = millimeter!(2.0, 2.0);
        let absorption = reciprocal_centimeter!(1.0);
        let mut field = field_centered_on_the_axis()?;
        PumpSource::Analytic(AnalyticPump::new(
            reciprocal_centimeter!(0.5),
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                millimeter!(0.0, 0.0),
                sigma,
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                absorption,
                PumpDirection::Forward,
            )?),
        )?)
        .deposit(&mut field, cross_section())?;
        let cell = (6, 5, 2);
        let center = center_of(&field, cell)?;
        let radial = center.x.value.hypot(center.y.value);
        // The medium starts at z = 0, so the depth the pump has travelled is the cell's own z.
        let expected = 2.5e19
            * f64::exp(-0.5 * (radial / sigma.x.value).powi(2))
            * f64::exp(-(absorption * center.z).value);
        assert_relative_eq!(inversion_at(&field, cell)?, expected, max_relative = 1e-12);
        Ok(())
    }
    #[test]
    fn a_pump_that_grows_as_it_is_absorbed_is_refused() {
        // A gain coefficient may be negative - that is an absorbing medium. An absorption
        // coefficient may not: a pump getting stronger the deeper it goes is not physics.
        assert!(
            BeerLambertProfile::new(reciprocal_centimeter!(-1.0), PumpDirection::Forward).is_err()
        );
        assert!(
            BeerLambertProfile::new(reciprocal_centimeter!(f64::NAN), PumpDirection::Forward)
                .is_err()
        );
        // No absorption at all is fine though, it is simply the flat profile.
        assert!(
            BeerLambertProfile::new(reciprocal_centimeter!(0.0), PumpDirection::Forward).is_ok()
        );
    }
    /// The conversion a pump source performs, checked against the same numbers by hand.
    #[test]
    fn a_gain_coefficient_becomes_an_inversion_density() -> OpmResult<()> {
        assert_relative_eq!(
            inversion_from_gain(reciprocal_centimeter!(0.5), cross_section())?
                .get::<per_cubic_centimeter>(),
            2.5e19,
            max_relative = 1e-12
        );
        assert_eq!(
            inversion_from_gain(reciprocal_centimeter!(1.0), cross_section())?,
            num_per_cm3!(5.0e19)
        );
        Ok(())
    }
}
