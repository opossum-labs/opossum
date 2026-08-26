#![warn(missing_docs)]
//! Unsaturated amplification that follows the path a ray takes through the medium.
//!
//! The step beyond [`ConstGain`](super::ConstGain): where a constant factor multiplies every ray
//! alike, this one integrates the local gain along the chord the ray actually travels inside the
//! body, `G = exp(∫ σ_e·ΔN ds)`. Two rays crossing the same medium therefore leave with different
//! factors — an oblique one gains over a longer path, one passing the rim of a shaped pump profile
//! gains less than one on the axis.
//!
//! **The inversion is frozen.** Extracting energy here does not draw the medium down, so a second
//! pass sees exactly what the first one saw. That is what makes the model "small signal": it holds
//! as long as the extracted energy is negligible against the stored energy. Saturation is the next
//! stage and is what will start writing back into the
//! [`InversionField`](super::InversionField).
//!
//! **Deliberate non-goals at this stage**, both deferred rather than forgotten:
//!
//! - *No wavelength dependence.* [`SmallSignalGain::emission_cross_section`] is one number, not a
//!   σ_e(λ) curve, so the gain of a ray does not depend on its colour. Gain narrowing and the red
//!   shift of a chirped pulse need the spectral stage.
//! - *No saturation and no extraction warning.* Nothing is drawn out of the medium, so there is
//!   nothing that could be overdrawn.

use super::inversion_field::CellIndex;
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive, ValidateTrait},
    square_centimeter, validated, validated_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Area;
use utoipa::ToSchema;

/// An emission cross section that is guaranteed to be finite and strictly positive.
///
/// Strictly, unlike the gain factor of [`ConstGain`](super::ConstGain): σ_e is a *divisor* when a
/// pump source stated as a gain coefficient is turned into an inversion density (see
/// [`PumpSource::deposit`](super::PumpSource::deposit)), and a medium that cannot emit has no
/// inversion that would explain a gain.
type ValidatedEmissionCrossSection = validated_type!(Area, AllNotZero && AllFinite && AllPositive);
impl Default for ValidatedEmissionCrossSection {
    /// The emission cross section of a typical solid state gain medium, of the order Yb:YAG has.
    ///
    /// A placeholder with a *usable* value rather than a neutral one: zero would be neutral only
    /// for as long as nothing pumps the medium, and would then fail the moment a pump source is
    /// picked. See [`SmallSignalGain::emission_cross_section`] for why this is a parameter at all.
    fn default() -> Self {
        validated!(
            square_centimeter!(2.0e-20),
            AllNotZero && AllFinite && AllPositive
        )
        .unwrap()
    }
}

/// A number of steps that is guaranteed to be non-zero.
type ValidatedStepCount = validated_type!(usize, AllNotZero);

/// How many substeps the inner path is integrated in by default.
///
/// Fine enough to follow a shaped pump profile across a typical head, cheap enough not to be worth
/// tuning for a first look. It is a convergence parameter, not physics: see
/// [`SmallSignalGain::n_steps`].
const DEFAULT_STEPS: usize = 16;

/// How many cells the medium is discretised into along each axis by default.
const DEFAULT_CELLS: usize = 16;

/// Parameters of an unsaturated gain that follows the path through the medium.
///
/// See the [module documentation](self) for what the model does and what it deliberately does not.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated)]
#[serde(try_from = "NonValidatedSmallSignalGain")]
pub struct SmallSignalGain {
    #[schema(value_type = f64)]
    emission_cross_section: ValidatedEmissionCrossSection,
    #[schema(value_type = usize)]
    n_steps: ValidatedStepCount,
    #[schema(value_type = usize)]
    cells_x: ValidatedStepCount,
    #[schema(value_type = usize)]
    cells_y: ValidatedStepCount,
    #[schema(value_type = usize)]
    cells_z: ValidatedStepCount,
}

/// Deserialization shim for [`SmallSignalGain`].
///
/// It lets the values read from an `.opm` file run through the very same validation as ones set
/// through the setters, so a hand-edited file cannot smuggle in a zero step count or a medium that
/// cannot emit. Same pattern as [`ConstGain`](super::ConstGain).
#[derive(Deserialize)]
struct NonValidatedSmallSignalGain {
    emission_cross_section: Area,
    n_steps: usize,
    cells_x: usize,
    cells_y: usize,
    cells_z: usize,
}
impl TryFrom<NonValidatedSmallSignalGain> for SmallSignalGain {
    type Error = String;
    fn try_from(helper: NonValidatedSmallSignalGain) -> Result<Self, Self::Error> {
        Self::new(
            helper.emission_cross_section,
            helper.n_steps,
            (helper.cells_x, helper.cells_y, helper.cells_z),
        )
        .map_err(|e| e.to_string())
    }
}

impl Default for SmallSignalGain {
    /// Create a [`SmallSignalGain`] with a usable cross section and a moderate discretisation.
    ///
    /// Picking this model must not change a result on its own, and it does not: with the medium
    /// unpumped the inversion is zero everywhere, so the integral below is zero and the gain is
    /// exactly one, whatever these parameters say.
    fn default() -> Self {
        Self {
            emission_cross_section: ValidatedEmissionCrossSection::default(),
            n_steps: validated!(DEFAULT_STEPS, AllNotZero).unwrap(),
            cells_x: validated!(DEFAULT_CELLS, AllNotZero).unwrap(),
            cells_y: validated!(DEFAULT_CELLS, AllNotZero).unwrap(),
            cells_z: validated!(DEFAULT_CELLS, AllNotZero).unwrap(),
        }
    }
}

impl SmallSignalGain {
    /// Create a new [`SmallSignalGain`].
    ///
    /// # Arguments
    ///
    /// * `emission_cross_section` - σ_e of the medium, see
    ///   [`SmallSignalGain::emission_cross_section`].
    /// * `n_steps` - how many substeps the inner path is integrated in.
    /// * `grid` - how many cells the medium is discretised into along its x, y and z axis.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the cross section
    /// is not finite, zero or negative, or if any of the four counts is zero.
    pub fn new(emission_cross_section: Area, n_steps: usize, grid: CellIndex) -> OpmResult<Self> {
        let mut model = Self::default();
        model.set_emission_cross_section(emission_cross_section)?;
        model.set_n_steps(n_steps)?;
        model.set_grid(grid)?;
        Ok(model)
    }
    /// Return σ_e, the emission cross section of the medium at the laser wavelength.
    ///
    /// It is a parameter of the *model* rather than something read off the
    /// [`Material`](crate::material::Material), which carries no spectroscopic data yet. Putting it
    /// here is what keeps the two halves of the operating point consistent: the very same number
    /// turns the pump source's gain coefficient into an inversion density and turns that density
    /// back into a gain, so the two cannot be based on different assumptions about the medium. At a
    /// single wavelength it therefore cancels out exactly, and only becomes a physical input once
    /// σ_e(λ) replaces it.
    #[must_use]
    pub const fn emission_cross_section(&self) -> Area {
        *self.emission_cross_section.get()
    }
    /// Return how many substeps the path through the medium is integrated in.
    ///
    /// A convergence parameter, not physics: the exact answer is the limit of refining it. One step
    /// is already exact wherever the inversion does not vary along the ray, and more steps only pay
    /// off where it does.
    #[must_use]
    pub const fn n_steps(&self) -> usize {
        *self.n_steps.get()
    }
    /// Return how many cells the medium is discretised into along its x, y and z axis.
    ///
    /// This is what an [`InversionField`] is laid out with, so it bounds how finely a shaped pump
    /// profile can be resolved. Like [`SmallSignalGain::n_steps`] it is a convergence parameter.
    #[must_use]
    pub const fn grid(&self) -> CellIndex {
        (
            *self.cells_x.get(),
            *self.cells_y.get(),
            *self.cells_z.get(),
        )
    }
    /// Set σ_e, the emission cross section of the medium.
    ///
    /// # Arguments
    ///
    /// * `emission_cross_section` - σ_e, see [`SmallSignalGain::emission_cross_section`].
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the given value is
    /// not finite, zero or negative. The previous value is kept in that case.
    pub fn set_emission_cross_section(&mut self, emission_cross_section: Area) -> OpmResult<()> {
        self.emission_cross_section.set(emission_cross_section)
    }
    /// Set how many substeps the path through the medium is integrated in.
    ///
    /// # Arguments
    ///
    /// * `n_steps` - the number of substeps, at least one.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the given count is
    /// zero. The previous value is kept in that case.
    pub fn set_n_steps(&mut self, n_steps: usize) -> OpmResult<()> {
        self.n_steps.set(n_steps)
    }
    /// Set how many cells the medium is discretised into along its x, y and z axis.
    ///
    /// # Arguments
    ///
    /// * `grid` - the number of cells along each axis, each at least one.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if any of the three
    /// counts is zero. The previous grid is kept in that case, as a whole: a grid that took only
    /// the axes that happened to be valid would be a size nobody asked for.
    pub fn set_grid(&mut self, grid: CellIndex) -> OpmResult<()> {
        let (x, y, z) = grid;
        // All three are validated before any of them is written, so a rejected axis cannot leave
        // the grid half updated.
        let (x, y, z) = (
            validated!(x, AllNotZero)?,
            validated!(y, AllNotZero)?,
            validated!(z, AllNotZero)?,
        );
        self.cells_x = x;
        self.cells_y = y;
        self.cells_z = z;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{error::OpossumError, square_meter};
    use approx::assert_relative_eq;

    #[test]
    fn the_default_is_usable_and_neutral() {
        let model = SmallSignalGain::default();
        // A cross section of zero would pass every test that never pumps the medium and then fail
        // the moment a pump source is picked, so the default has to be a real value.
        assert!(model.emission_cross_section().value > 0.0);
        assert!(model.emission_cross_section().is_finite());
        assert_eq!(model.n_steps(), DEFAULT_STEPS);
        assert_eq!(model.grid(), (DEFAULT_CELLS, DEFAULT_CELLS, DEFAULT_CELLS));
    }
    #[test]
    fn new_keeps_what_it_was_given() -> OpmResult<()> {
        let model = SmallSignalGain::new(square_meter!(3.0e-24), 8, (4, 5, 6))?;
        assert_relative_eq!(model.emission_cross_section().value, 3.0e-24);
        assert_eq!(model.n_steps(), 8);
        assert_eq!(model.grid(), (4, 5, 6));
        Ok(())
    }
    #[test]
    fn a_medium_that_cannot_emit_is_refused() {
        // Not merely non-negative like a gain factor: sigma_e divides when a gain coefficient is
        // turned into an inversion, so zero is as unusable as a negative value.
        for refused in [0.0, -1.0e-24, f64::NAN, f64::INFINITY] {
            assert!(
                SmallSignalGain::new(square_meter!(refused), 8, (4, 4, 4)).is_err(),
                "a cross section of {refused} m^2 should be refused"
            );
        }
    }
    #[test]
    fn a_march_without_steps_is_refused() {
        assert!(SmallSignalGain::new(square_meter!(2.0e-24), 0, (4, 4, 4)).is_err());
        assert!(SmallSignalGain::new(square_meter!(2.0e-24), 1, (4, 4, 4)).is_ok());
    }
    #[test]
    fn a_grid_without_cells_is_refused() {
        for refused in [(0, 4, 4), (4, 0, 4), (4, 4, 0)] {
            assert!(
                SmallSignalGain::new(square_meter!(2.0e-24), 8, refused).is_err(),
                "a grid of {refused:?} should be refused"
            );
        }
        assert!(SmallSignalGain::new(square_meter!(2.0e-24), 8, (1, 1, 1)).is_ok());
    }
    #[test]
    fn a_rejected_value_keeps_the_old_one() -> OpmResult<()> {
        // A half-typed value in the GUI must not damage what is already configured.
        let mut model = SmallSignalGain::new(square_meter!(3.0e-24), 8, (4, 5, 6))?;
        assert!(
            model
                .set_emission_cross_section(square_meter!(0.0))
                .is_err()
        );
        assert!(model.set_n_steps(0).is_err());
        assert_relative_eq!(model.emission_cross_section().value, 3.0e-24);
        assert_eq!(model.n_steps(), 8);
        // ... and a grid is kept as a whole, not per axis: the z axis below is fine, but the y one
        // is not, and a partially applied grid would be a size nobody asked for.
        assert!(model.set_grid((7, 0, 9)).is_err());
        assert_eq!(model.grid(), (4, 5, 6));
        Ok(())
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        let model = SmallSignalGain::new(square_meter!(3.0e-24), 8, (4, 5, 6))?;
        let serialized = ron::to_string(&model).map_err(|e| OpossumError::Other(e.to_string()))?;
        let deserialized: SmallSignalGain =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(model, deserialized);
        Ok(())
    }
    #[test]
    fn a_hand_edited_file_cannot_smuggle_past_the_validation() {
        // The shim is what makes reading a file go through the very same setters as the GUI does.
        for refused in [
            "(emission_cross_section:3.0e-24,n_steps:0,cells_x:4,cells_y:5,cells_z:6)",
            "(emission_cross_section:0.0,n_steps:8,cells_x:4,cells_y:5,cells_z:6)",
            "(emission_cross_section:3.0e-24,n_steps:8,cells_x:0,cells_y:5,cells_z:6)",
        ] {
            assert!(
                ron::from_str::<SmallSignalGain>(refused).is_err(),
                "the file content {refused} should be refused"
            );
        }
    }
}
