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

use super::{
    extraction::{Extraction, Medium},
    inversion_field::{CellIndex, InversionField},
    pump_source::four_level_gain_from_inversion,
    scenario::PumpConfig,
};
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllNotZero, AllPositive, ValidateTrait},
    geometry::body::Body,
    light::{Ray, Rays, Spectrum},
    square_centimeter,
    utils::math_utils::to_f64,
    validated, validated_type,
};
use nalgebra::Point3;
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

impl SmallSignalGain {
    /// Integrate the gain a ray picks up on its way through a pumped medium.
    ///
    /// The ray is expected to sit **on the entrance surface** with the direction it was refracted
    /// into, which is where
    /// [`pass_through_volume_generic`](crate::core_optics::volumetric::Volumetric::pass_through_volume_generic)
    /// hands it over: the chord from there to the exit is exactly the stretch that amplifies it.
    ///
    /// The integral is evaluated by the **midpoint rule** over [`n_steps`](Self::n_steps) equal
    /// substeps. Midpoints rather than endpoints for two reasons: it is second order accurate for
    /// the same number of samples, and it never samples exactly on a bounding surface, where both
    /// [`Body::contains`] and [`InversionField::cell_at`] are half-open and would answer "outside"
    /// for the very point the ray entered or left through.
    ///
    /// Nothing is written back - the inversion is frozen, see the [module documentation](self).
    ///
    /// # Arguments
    ///
    /// * `medium` - the active medium being crossed.
    /// * `ray` - the ray crossing it.
    ///
    /// # Returns
    ///
    /// The factor this ray's energy is multiplied by. A ray that does not pass through the body at
    /// all - because it missed it, left it sideways, or would have to re-enter it - answers exactly
    /// 1.0 rather than an error: not being amplified is precisely what should happen to it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the medium carries no inversion field, if the body cannot
    /// state the path length, or if the accumulated gain is not a finite factor - an energy that
    /// overflows is a modelling mistake worth stopping for rather than an infinity to propagate.
    pub fn gain_factor(&self, medium: &Medium<'_>, ray: &Ray) -> OpmResult<f64> {
        let body = medium.body()?;
        let Some(chord) = body.path_length_inside(ray)? else {
            return Ok(1.0);
        };
        // A ray grazing the very edge of the medium travels no distance in it and gains nothing.
        // Caught here so that the step width below cannot become zero over zero.
        if chord.value <= 0.0 {
            return Ok(1.0);
        }
        // `Ray::direction` is explicitly not guaranteed to be normalized, and stepping along it
        // would otherwise cover the wrong distance. `path_length_inside` is unaffected - it
        // measures between the two intersections rather than along the vector.
        let direction = ray.direction();
        let norm = direction.norm();
        if !norm.is_normal() {
            return Ok(1.0);
        }
        let direction = direction / norm;
        let field = medium.field()?;
        let steps = self.n_steps();
        let step_width = chord / to_f64(steps);
        let start = ray.position();
        // The frame is taken from the body rather than from the node, because that is the frame
        // `InversionField::from_body` masked its cells in. Asking the same object makes it
        // impossible for the sampling and the masking to disagree about where the medium is.
        let frame = body.isometry();
        let mut exponent = 0.0_f64;
        for step in 0..steps {
            let travelled = step_width * (to_f64(step) + 0.5);
            let sample = Point3::new(
                start.x + direction.x * travelled,
                start.y + direction.y * travelled,
                start.z + direction.z * travelled,
            );
            let local = frame.inverse_transform_point(&sample);
            // Outside the grid, or inside it but not in the medium: there is nothing there to
            // amplify with, so the substep contributes nothing rather than failing.
            let Some(cell) = field.cell_at(&local) else {
                continue;
            };
            if !field.is_inside(cell) {
                continue;
            }
            let Some(inversion) = field.population(cell) else {
                continue;
            };
            exponent += (four_level_gain_from_inversion(inversion, self.emission_cross_section())
                * step_width)
                .value;
        }
        let factor = exponent.exp();
        if factor.is_finite() {
            Ok(factor)
        } else {
            Err(OpossumError::Analysis(format!(
                "node '{}' would amplify by exp({exponent}) over a path of {} mm through its \
                 medium, which is not a finite factor",
                medium.node_name(),
                chord.get::<uom::si::length::millimeter>()
            )))
        }
    }
}

impl Extraction for SmallSignalGain {
    fn name(&self) -> &'static str {
        "SmallSignalGain"
    }
    fn needs_inversion(&self) -> bool {
        // The whole point of the stage: what a beam gains is what the medium holds where the beam
        // went, so how the medium was pumped is an input.
        true
    }
    /// Lay an [`InversionField`] over the body and pump it as the operating point says.
    ///
    /// This is the one place the two halves of a [`PumpConfig`] meet: the
    /// [`PumpSource`](super::PumpSource) writes the inversion, this model supplies the σ_e that its
    /// gain coefficient is stated against, and the field that comes out is what
    /// [`gain_factor`](SmallSignalGain::gain_factor) reads. Neither half knows the other.
    fn pumped_medium(
        &self,
        body: &dyn Body,
        config: &PumpConfig,
    ) -> OpmResult<Option<InversionField>> {
        let mut field = InversionField::from_body(body, self.grid())?;
        config
            .pump()
            .deposit(&mut field, self.emission_cross_section())?;
        Ok(Some(field))
    }
    fn amplify_rays(&self, medium: &Medium<'_>, rays_bundle: &mut [Rays]) -> OpmResult<()> {
        for rays in rays_bundle.iter_mut() {
            for ray in rays.iter_mut() {
                // Invalid rays no longer take part in the propagation - the same rule
                // `Rays::scale_energy` and `Rays::filter_energy` follow.
                if ray.valid() {
                    let factor = self.gain_factor(medium, ray)?;
                    ray.scale_energy(factor)?;
                }
            }
        }
        Ok(())
    }
    fn amplify_spectrum(&self, medium: &Medium<'_>, _spectrum: &mut Spectrum) -> OpmResult<()> {
        Err(OpossumError::Analysis(format!(
            "node '{}' is configured with a small signal gain, which is integrated along the path a \
             beam takes through the medium - an energy flow analysis knows no path. Analyze it as a \
             ray trace, or use a constant gain here.",
            medium.node_name()
        )))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        gain::{
            AnalyticPump, BeerLambertProfile, ConstInversion, GainModel, LongitudinalProfile,
            PumpDirection, PumpSource, TransversalProfile,
        },
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        joule, millimeter, nanometer, reciprocal_centimeter, square_meter,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::{geom_transformation::Isometry, super_gaussian::SuperGaussianShape},
    };
    use approx::assert_relative_eq;
    use nalgebra::{Point2, Vector3};
    use std::sync::{Arc, Mutex};
    use uom::si::f64::{Length, ReciprocalLength};

    /// The thickness of the disk every physics test below amplifies through.
    const THICKNESS: f64 = 10.0;

    /// Create a plane-parallel disk of the given thickness and radius, sitting at the origin.
    ///
    /// Plane faces on purpose: the chord of an on-axis ray is then exactly the thickness, so every
    /// expected value below can be worked out from `exp(g₀·L)` by hand.
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
    /// The disk every physics test below is performed on: 10 mm thick, 5 mm in radius.
    fn test_disk() -> OpmResult<SurfaceBoundedBody> {
        disk(millimeter!(THICKNESS), millimeter!(5.0))
    }
    /// A ray entering the medium at the given transversal offset, travelling along the axis.
    fn ray_at(x: f64, y: f64) -> OpmResult<Ray> {
        Ray::new(
            millimeter!(x, y, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )
    }
    /// An operating point that pumps a medium uniformly to the given gain coefficient.
    fn pumped_at(g_0: ReciprocalLength) -> OpmResult<PumpConfig> {
        Ok(PumpConfig::new(
            GainModel::None,
            PumpSource::Const(ConstInversion::new(g_0)?),
        ))
    }
    /// The factor a ray picks up crossing the given body in the given operating point.
    fn factor_through(
        body: &dyn Body,
        model: &SmallSignalGain,
        config: &PumpConfig,
        ray: &Ray,
    ) -> OpmResult<f64> {
        let field = model.pumped_medium(body, config)?;
        model.gain_factor(&Medium::new(body, field.as_ref(), "test head"), ray)
    }
    /// The factor a ray picks up crossing the standard [`test_disk`].
    fn factor_through_disk(
        model: &SmallSignalGain,
        config: &PumpConfig,
        ray: &Ray,
    ) -> OpmResult<f64> {
        factor_through(&test_disk()?, model, config, ray)
    }

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
    fn a_homogeneous_medium_amplifies_by_exp_g0_l() -> OpmResult<()> {
        // The defining case: a uniformly pumped medium gives `G = exp(g0 * L)`, and for an on-axis
        // ray through a plane-parallel disk `L` is exactly the thickness.
        let model = SmallSignalGain::default();
        let g_0 = reciprocal_centimeter!(0.5);
        let factor = factor_through_disk(&model, &pumped_at(g_0)?, &ray_at(0.0, 0.0)?)?;
        assert_relative_eq!(
            factor,
            f64::exp((g_0 * millimeter!(THICKNESS)).value),
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn a_negative_inversion_absorbs() -> OpmResult<()> {
        // A negative coefficient is the same physics with the inversion turned around, so it has to
        // come out as plain Beer-Lambert absorption over the very same path.
        let model = SmallSignalGain::default();
        let g_0 = reciprocal_centimeter!(-0.5);
        let factor = factor_through_disk(&model, &pumped_at(g_0)?, &ray_at(0.0, 0.0)?)?;
        assert_relative_eq!(
            factor,
            f64::exp((g_0 * millimeter!(THICKNESS)).value),
            max_relative = 1e-12
        );
        assert!(factor < 1.0, "an absorbing medium must not amplify");
        Ok(())
    }
    #[test]
    fn an_unpumped_medium_is_a_plain_pass() -> OpmResult<()> {
        // Picking the model must not change a result on its own.
        let model = SmallSignalGain::default();
        let passive = PumpConfig::new(GainModel::None, PumpSource::None);
        assert_relative_eq!(
            factor_through_disk(&model, &passive, &ray_at(0.0, 0.0)?)?,
            1.0
        );
        Ok(())
    }
    #[test]
    fn an_oblique_ray_gains_over_its_real_path() -> OpmResult<()> {
        // The whole difference to a constant gain: the path counts. At 45 degrees the chord through
        // a plane-parallel disk is longer by sqrt(2), and the gain has to follow.
        //
        // The disk is deliberately made wider than the standard one here: at this angle the ray
        // wanders a full thickness across the cross section on its way through, and one that leaves
        // sideways does not pass through the body at all - see
        // `a_ray_that_does_not_cross_the_medium_is_not_amplified` for that case.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let model = SmallSignalGain::default();
        let g_0 = reciprocal_centimeter!(0.5);
        let oblique = Ray::new(
            millimeter!(0.0, -5.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let factor = factor_through(&body, &model, &pumped_at(g_0)?, &oblique)?;
        assert_relative_eq!(
            factor,
            f64::exp((g_0 * millimeter!(THICKNESS) * f64::sqrt(2.0)).value),
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn a_ray_leaving_sideways_is_not_amplified() -> OpmResult<()> {
        // The same 45 degree ray through the standard, narrow disk: it enters the medium but leaves
        // through the barrel rather than the exit face. There is no chord to integrate over, so it
        // is left alone rather than amplified over a path it never completed.
        let model = SmallSignalGain::default();
        let escaping = Ray::new(
            millimeter!(0.0, -3.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(
            factor_through_disk(&model, &pumped_at(reciprocal_centimeter!(0.5))?, &escaping)?,
            1.0
        );
        Ok(())
    }
    #[test]
    fn the_emission_cross_section_cancels_out() -> OpmResult<()> {
        // sigma_e divides when the pump's coefficient becomes a density and multiplies when that
        // density becomes a gain again. As long as both sides use the same number - which is what
        // putting it on the model guarantees - it cannot influence the result at all.
        let g_0 = reciprocal_centimeter!(0.5);
        let config = pumped_at(g_0)?;
        let ray = ray_at(0.0, 0.0)?;
        let lean = SmallSignalGain::new(square_meter!(2.0e-24), 8, (8, 8, 8))?;
        let fat = SmallSignalGain::new(square_meter!(2.0e-23), 8, (8, 8, 8))?;
        assert_relative_eq!(
            factor_through_disk(&lean, &config, &ray)?,
            factor_through_disk(&fat, &config, &ray)?,
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn the_march_converges_when_it_is_refined() -> OpmResult<()> {
        // A flat profile is integrated exactly by a single midpoint step, so the convergence has to
        // be shown on an inversion that actually varies along the ray. Beer-Lambert has a closed
        // form: the integral of g0*exp(-alpha*s) from 0 to L is g0/alpha * (1 - exp(-alpha*L)).
        let g_0 = reciprocal_centimeter!(0.5);
        let alpha = reciprocal_centimeter!(2.0);
        let length = millimeter!(THICKNESS);
        let config = PumpConfig::new(
            GainModel::None,
            PumpSource::Analytic(AnalyticPump::new(
                g_0,
                TransversalProfile::Flat,
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    alpha,
                    PumpDirection::Forward,
                )?),
            )?),
        );
        let exact = f64::exp(((g_0 / alpha) * (1.0 - f64::exp(-(alpha * length).value))).value);
        let ray = ray_at(0.0, 0.0)?;
        // The grid is kept far finer than the march so that what is being refined here is the
        // integration, not the sampling of the profile.
        let error = |steps: usize| -> OpmResult<f64> {
            let model = SmallSignalGain::new(square_meter!(2.0e-24), steps, (4, 4, 512))?;
            Ok((factor_through_disk(&model, &config, &ray)? - exact).abs() / exact)
        };
        let (coarse, medium, fine) = (error(2)?, error(8)?, error(64)?);
        assert!(
            medium < coarse,
            "refining 2 -> 8 did not help: {coarse} -> {medium}"
        );
        assert!(
            fine < medium,
            "refining 8 -> 64 did not help: {medium} -> {fine}"
        );
        assert!(fine < 1e-3, "the fine march is still off by {fine}");
        Ok(())
    }
    #[test]
    fn a_transversal_profile_makes_an_edge_ray_gain_less() -> OpmResult<()> {
        // The inversion varies across the cross section, so two parallel rays through the same
        // medium leave with different factors - something a bundle-wide factor could not express.
        let sigma = millimeter!(2.0);
        let config = PumpConfig::new(
            GainModel::None,
            PumpSource::Analytic(AnalyticPump::new(
                reciprocal_centimeter!(0.5),
                TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                    Point2::new(millimeter!(0.0), millimeter!(0.0)),
                    Point2::new(sigma, sigma),
                    1.0,
                    degree!(0.0),
                    false,
                )?),
                LongitudinalProfile::Flat,
            )?),
        );
        // An odd cell count puts one column of cells exactly on the axis, so the axial ray really
        // samples the peak of the profile rather than a neighbour of it.
        let model = SmallSignalGain::new(square_meter!(2.0e-24), 8, (65, 65, 8))?;
        let axial = factor_through_disk(&model, &config, &ray_at(0.0, 0.0)?)?;
        let outer = factor_through_disk(&model, &config, &ray_at(0.0, 4.0)?)?;
        assert!(
            axial > outer,
            "the axial ray should gain more: {axial} vs {outer}"
        );
        // ... and the axial one follows the peak of the profile, which is the plain exp(g0*L)
        assert_relative_eq!(
            axial,
            f64::exp((reciprocal_centimeter!(0.5) * millimeter!(THICKNESS)).value),
            max_relative = 1e-2
        );
        Ok(())
    }
    #[test]
    fn the_inversion_is_frozen() -> OpmResult<()> {
        // What makes this the *small signal* stage: extracting energy does not draw the medium
        // down, so a second pass sees exactly what the first one saw.
        let body = test_disk()?;
        let model = SmallSignalGain::default();
        let field = model
            .pumped_medium(&body, &pumped_at(reciprocal_centimeter!(0.5))?)?
            .ok_or_else(|| OpossumError::Other("this model must pump the medium".into()))?;
        let untouched = field.clone();
        let medium = Medium::new(&body, Some(&field), "test head");
        let ray = ray_at(0.0, 0.0)?;
        let first = model.gain_factor(&medium, &ray)?;
        let second = model.gain_factor(&medium, &ray)?;
        assert_eq!(field, untouched);
        assert_relative_eq!(first, second);
        Ok(())
    }
    #[test]
    fn a_ray_that_does_not_cross_the_medium_is_not_amplified() -> OpmResult<()> {
        let model = SmallSignalGain::default();
        let config = pumped_at(reciprocal_centimeter!(0.5))?;
        // beside the disk, running past it ...
        let beside = Ray::new(
            millimeter!(20.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(factor_through_disk(&model, &config, &beside)?, 1.0);
        // ... and one pointing away from it, which never reaches it either
        let away = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, -1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(factor_through_disk(&model, &config, &away)?, 1.0);
        Ok(())
    }
    #[test]
    fn a_gain_that_would_overflow_is_refused() -> OpmResult<()> {
        // An absurd operating point has to stop the analysis rather than hand an infinite energy
        // down the graph, where it would only surface much later as a meaningless report.
        let model = SmallSignalGain::default();
        let config = pumped_at(reciprocal_centimeter!(1.0e6))?;
        assert!(factor_through_disk(&model, &config, &ray_at(0.0, 0.0)?).is_err());
        Ok(())
    }
    #[test]
    fn a_model_reading_an_unpumped_medium_says_so() -> OpmResult<()> {
        // The one inconsistency `Medium::field` guards against: a model that claims to read the
        // inversion but was handed no field. Not reachable through the volume machinery, which
        // always asks the very same model for the field it is about to read - but a clear error
        // beats a silent 1.0 if a future caller gets it wrong.
        let body = test_disk()?;
        let model = SmallSignalGain::default();
        let medium = Medium::new(&body, None, "test head");
        assert!(model.gain_factor(&medium, &ray_at(0.0, 0.0)?).is_err());
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
