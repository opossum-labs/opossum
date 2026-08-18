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
    generic_validators::{AllFinite, ValidateTrait},
    reciprocal_centimeter,
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;
use uom::si::f64::{Area, ReciprocalLength, VolumetricNumberDensity};
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
}
impl Display for PumpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Const(_) => write!(f, "Const"),
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
        // Matched exhaustively on purpose: a source added later has to state here how it is
        // evaluated over the grid, rather than falling into a catch-all arm that would silently
        // pump nothing.
        match self {
            Self::None => Ok(()),
            Self::Const(constant) => {
                let inversion =
                    inversion_from_gain(constant.gain_coefficient(), emission_cross_section)?;
                for cell in cells(field.dimensions()) {
                    if !field.is_inside(cell) {
                        continue;
                    }
                    let Some(present) = field.population(cell) else {
                        continue;
                    };
                    field.set_population(cell, present + inversion)?;
                }
                Ok(())
            }
        }
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
    /// Create an unpumped field over a disk, coarse enough for its corners to lie outside the body.
    fn field_over_a_disk() -> OpmResult<InversionField> {
        InversionField::from_body(&disk(millimeter!(10.0), millimeter!(5.0))?, (8, 8, 4))
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
