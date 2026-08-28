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
    extraction::Extraction,
    inversion_field::{CellIndex, InversionField},
    pump_source::four_level_gain_from_inversion,
    scenario::PumpConfig,
};
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllNotZero, AllPositive, ValidateTrait},
    geometry::body::Body,
    light::{Ray, Spectrum},
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

/// A number of cells that is guaranteed to be non-zero.
type ValidatedCellCount = validated_type!(usize, AllNotZero);

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
    cells_x: ValidatedCellCount,
    #[schema(value_type = usize)]
    cells_y: ValidatedCellCount,
    #[schema(value_type = usize)]
    cells_z: ValidatedCellCount,
}

/// Deserialization shim for [`SmallSignalGain`].
///
/// It lets the values read from an `.opm` file run through the very same validation as ones set
/// through the setters, so a hand-edited file cannot smuggle in an unusable cross section or a
/// zero-cell grid. Same pattern as [`ConstGain`](super::ConstGain).
#[derive(Deserialize)]
struct NonValidatedSmallSignalGain {
    emission_cross_section: Area,
    cells_x: usize,
    cells_y: usize,
    cells_z: usize,
}
impl TryFrom<NonValidatedSmallSignalGain> for SmallSignalGain {
    type Error = String;
    fn try_from(helper: NonValidatedSmallSignalGain) -> Result<Self, Self::Error> {
        Self::new(
            helper.emission_cross_section,
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
    /// * `grid` - how many cells the medium is discretised into along its x, y and z axis.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the cross section
    /// is not finite, zero or negative, or if any of the three grid counts is zero.
    pub fn new(emission_cross_section: Area, grid: CellIndex) -> OpmResult<Self> {
        let mut model = Self::default();
        model.set_emission_cross_section(emission_cross_section)?;
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
    /// Return how many cells the medium is discretised into along its x, y and z axis.
    ///
    /// This is what an [`InversionField`] is laid out with, so it bounds how finely a shaped pump
    /// profile can be resolved. It is the sole convergence parameter for the gain integration.
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
    /// [`gain_exponent_at`](Self::gain_exponent_at) reads. Neither half knows the other.
    fn build_inversion(
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
    fn path_exponent(
        &self,
        body: &dyn Body,
        ray: &Ray,
        inversion: &mut Option<InversionField>,
    ) -> f64 {
        let Some(field) = inversion.as_ref() else {
            return 0.0;
        };
        // Normalize the ray direction: the DDA requires a unit vector so that parametric
        // distances equal arc lengths.
        let direction = ray.direction();
        let norm = direction.norm();
        if !norm.is_normal() {
            return 0.0;
        }
        let dir = direction / norm;

        // Transform origin and direction into the field's local frame (the body's own frame).
        let iso = body.isometry();
        let local_origin = iso.inverse_transform_point(&ray.position());
        let local_dir = iso.inverse_transform_vector_f64(&dir);

        let mut exponent = 0.0_f64;
        for (cell, ds) in field.traverse(&local_origin, &local_dir) {
            if !field.is_inside(cell) {
                continue;
            }
            let Some(inv) = field.population(cell) else {
                continue;
            };
            exponent +=
                (four_level_gain_from_inversion(inv, self.emission_cross_section()) * ds).value;
        }
        exponent
    }
    fn amplify_spectrum(
        &self,
        _body: &dyn Body,
        _inversion: Option<&InversionField>,
        _spectrum: &mut Spectrum,
    ) -> OpmResult<()> {
        Err(OpossumError::Analysis(
            "a small signal gain is integrated along the path a beam takes through the medium - \
             an energy flow analysis knows no path. Analyze it as a ray trace, or use a constant \
             gain here."
                .into(),
        ))
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
            PumpDirection, PumpSource, TransversalProfile, inversion_field::cells,
            pump_source::inversion_from_gain,
        },
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        joule,
        light::Ray,
        millimeter, nanometer, reciprocal_centimeter, square_meter,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::{
            geom_transformation::Isometry, math_utils::to_f64, super_gaussian::SuperGaussianShape,
        },
    };
    use approx::assert_relative_eq;
    use nalgebra::{Point2, Vector3};
    use std::sync::{Arc, Mutex};
    use uom::si::f64::{Area, Length, ReciprocalLength};

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
    /// Compute the gain factor a ray picks up traversing the body via the production path_exponent.
    ///
    /// This calls production code rather than reimplementing the integration, so the tests actually
    /// exercise what runs in the analysis. Keeping `inversion` mutable lets
    /// `the_inversion_is_frozen` verify that [`SmallSignalGain`] does not write back into it.
    fn traverse_factor(
        body: &dyn Body,
        model: &SmallSignalGain,
        inversion: &mut Option<InversionField>,
        ray: &Ray,
    ) -> OpmResult<f64> {
        let Some(chord) = body.path_length_inside(ray)? else {
            return Ok(1.0);
        };
        if chord.value <= 0.0 {
            return Ok(1.0);
        }
        let exponent = Extraction::path_exponent(model, body, ray, inversion);
        let factor = exponent.exp();
        if factor.is_finite() {
            Ok(factor)
        } else {
            Err(OpossumError::Analysis(format!(
                "exp({exponent}) is not finite"
            )))
        }
    }
    /// The factor a ray picks up crossing the given body in the given operating point.
    fn factor_through(
        body: &dyn Body,
        model: &SmallSignalGain,
        config: &PumpConfig,
        ray: &Ray,
    ) -> OpmResult<f64> {
        let mut inversion = model.build_inversion(body, config)?;
        traverse_factor(body, model, &mut inversion, ray)
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
        assert_eq!(model.grid(), (DEFAULT_CELLS, DEFAULT_CELLS, DEFAULT_CELLS));
    }
    #[test]
    fn new_keeps_what_it_was_given() -> OpmResult<()> {
        let model = SmallSignalGain::new(square_meter!(3.0e-24), (4, 5, 6))?;
        assert_relative_eq!(model.emission_cross_section().value, 3.0e-24);
        assert_eq!(model.grid(), (4, 5, 6));
        Ok(())
    }
    #[test]
    fn a_medium_that_cannot_emit_is_refused() {
        // Not merely non-negative like a gain factor: sigma_e divides when a gain coefficient is
        // turned into an inversion, so zero is as unusable as a negative value.
        for refused in [0.0, -1.0e-24, f64::NAN, f64::INFINITY] {
            assert!(
                SmallSignalGain::new(square_meter!(refused), (4, 4, 4)).is_err(),
                "a cross section of {refused} m^2 should be refused"
            );
        }
    }
    #[test]
    fn a_grid_without_cells_is_refused() {
        for refused in [(0, 4, 4), (4, 0, 4), (4, 4, 0)] {
            assert!(
                SmallSignalGain::new(square_meter!(2.0e-24), refused).is_err(),
                "a grid of {refused:?} should be refused"
            );
        }
        assert!(SmallSignalGain::new(square_meter!(2.0e-24), (1, 1, 1)).is_ok());
    }
    #[test]
    fn a_rejected_value_keeps_the_old_one() -> OpmResult<()> {
        // A half-typed value in the GUI must not damage what is already configured.
        let mut model = SmallSignalGain::new(square_meter!(3.0e-24), (4, 5, 6))?;
        assert!(
            model
                .set_emission_cross_section(square_meter!(0.0))
                .is_err()
        );
        assert_relative_eq!(model.emission_cross_section().value, 3.0e-24);
        // ... and a grid is kept as a whole, not per axis: the z axis below is fine, but the y one
        // is not, and a partially applied grid would be a size nobody asked for.
        assert!(model.set_grid((7, 0, 9)).is_err());
        assert_eq!(model.grid(), (4, 5, 6));
        Ok(())
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        let model = SmallSignalGain::new(square_meter!(3.0e-24), (4, 5, 6))?;
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
        let lean = SmallSignalGain::new(square_meter!(2.0e-24), (8, 8, 8))?;
        let fat = SmallSignalGain::new(square_meter!(2.0e-23), (8, 8, 8))?;
        assert_relative_eq!(
            factor_through_disk(&lean, &config, &ray)?,
            factor_through_disk(&fat, &config, &ray)?,
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn the_integral_converges_when_the_grid_is_refined() -> OpmResult<()> {
        // With exact voxel traversal the only remaining error is the discretization of the
        // inversion profile onto the grid cells. Beer-Lambert has a closed form:
        // the integral of g0·exp(-α·s) from 0 to L is g0/α · (1 − exp(-α·L)).
        // Finer cells_z → smaller staircase approximation error → factor converges to exact.
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
        let error = |cells_z: usize| -> OpmResult<f64> {
            let model = SmallSignalGain::new(square_meter!(2.0e-24), (4, 4, cells_z))?;
            Ok((factor_through_disk(&model, &config, &ray)? - exact).abs() / exact)
        };
        let (coarse, medium, fine) = (error(2)?, error(8)?, error(64)?);
        assert!(
            medium < coarse,
            "refining cells_z 2 -> 8 did not help: {coarse} -> {medium}"
        );
        assert!(
            fine < medium,
            "refining cells_z 8 -> 64 did not help: {medium} -> {fine}"
        );
        assert!(fine < 1e-3, "the fine grid is still off by {fine}");
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
        let model = SmallSignalGain::new(square_meter!(2.0e-24), (65, 65, 8))?;
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
        let mut inversion =
            model.build_inversion(&body, &pumped_at(reciprocal_centimeter!(0.5))?)?;
        let untouched = inversion.clone();
        let ray = ray_at(0.0, 0.0)?;
        let first = traverse_factor(&body, &model, &mut inversion, &ray)?;
        let second = traverse_factor(&body, &model, &mut inversion, &ray)?;
        // SmallSignalGain must not write back into the field — both passes see identical inversion.
        assert_eq!(inversion, untouched);
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
    fn a_hand_edited_file_cannot_smuggle_past_the_validation() {
        // The shim is what makes reading a file go through the very same setters as the GUI does.
        for refused in [
            "(emission_cross_section:0.0,cells_x:4,cells_y:5,cells_z:6)",
            "(emission_cross_section:3.0e-24,cells_x:0,cells_y:5,cells_z:6)",
            "(emission_cross_section:3.0e-24,cells_x:4,cells_y:0,cells_z:6)",
        ] {
            assert!(
                ron::from_str::<SmallSignalGain>(refused).is_err(),
                "the file content {refused} should be refused"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Inhomogeneous inversion + oblique ray: path-length weighting tests
    //
    // Geometry shared by the two-cell tests: a wide plane-parallel disk (thick
    // enough not to let 45°-class rays escape sideways) with a (2,1,1) grid.
    // The grid splits x at x = 0 (the disk's bounding box is symmetric about
    // the axis), so cell (0,0,0) covers x < 0 and cell (1,0,0) covers x > 0.
    // A ray in the x–z plane crosses that boundary at a predictable depth.
    // -------------------------------------------------------------------------

    /// An `InversionField` over `body` where every in-medium cell carries the gain coefficient
    /// returned by `g(cell_index)`.
    ///
    /// Bypasses the pump/profile path so tests can prescribe an exact piecewise-constant field and
    /// compute analytic expectations without profile-discretisation error.  `sigma_e` is shared
    /// with the marching model so the density→coefficient round-trip cancels exactly.
    fn field_with(
        body: &dyn Body,
        dims: CellIndex,
        sigma_e: Area,
        g: impl Fn(CellIndex) -> OpmResult<ReciprocalLength>,
    ) -> OpmResult<InversionField> {
        let mut field = InversionField::from_body(body, dims)?;
        for cell in cells(dims) {
            if field.is_inside(cell) {
                field.set_population(cell, inversion_from_gain(g(cell)?, sigma_e)?)?;
            }
        }
        Ok(field)
    }

    #[test]
    fn an_oblique_ray_weights_each_cell_by_its_path_length() -> OpmResult<()> {
        // A ray that enters cell A (x < 0) and exits through cell B (x > 0), crossing x = 0
        // exactly at half the chord, spends equal path lengths in each cell.
        //
        // With exact voxel traversal each cell receives its geometric path length, so the
        // accumulated exponent is (g_a + g_b)/2 · chord — exact at any grid resolution.
        //
        // Also checked: the factor lies strictly between exp(g_a·chord) and exp(g_b·chord) — a
        // bundle-wide or endpoint-only approximation could not produce a value in between.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let sigma_e = square_meter!(2.0e-24);
        let g_a = reciprocal_centimeter!(0.3_f64);
        let g_b = reciprocal_centimeter!(0.8_f64);

        let inversion = field_with(&body, (2, 1, 1), sigma_e, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;

        // A ray from (-5, 0, 0) mm toward (5, 0, L) crosses x = 0 exactly at half the chord.
        let ray = Ray::new(
            millimeter!(-5.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, THICKNESS),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let model = SmallSignalGain::new(sigma_e, (2, 1, 1))?;
        let factor = traverse_factor(&body, &model, &mut Some(inversion), &ray)?;

        let chord = body
            .path_length_inside(&ray)?
            .expect("the ray must cross the disk");

        let expected = f64::exp(((g_a + g_b) * 0.5 * chord).value);
        assert_relative_eq!(factor, expected, max_relative = 1e-12);

        let only_a = f64::exp((g_a * chord).value);
        let only_b = f64::exp((g_b * chord).value);
        assert!(
            factor > only_a && factor < only_b,
            "mixed factor {factor} should be between {only_a} (all A) and {only_b} (all B)"
        );
        Ok(())
    }

    #[test]
    fn which_cell_holds_the_longer_path_decides() -> OpmResult<()> {
        // A ray from (-5, 0, 0) mm in direction (15, 0, L): x goes from -5 to -5+15=10 mm
        // over L mm in z, so x(z) = -5 + 15/L · z → x=0 at z = 5·L/15 = L/3.
        // Path fractions: 1/3 in cell A (x < 0), 2/3 in cell B (x ≥ 0).
        //
        // Expected gain: exp(g_a · chord/3 + g_b · 2·chord/3).
        //
        // The crossing at L/3 was NOT a step boundary for any integer n_steps, so the old
        // midpoint rule had a genuine O(1/n) error. Exact voxel traversal eliminates that error
        // entirely — this test now asserts the analytic value at 1e-12 precision.
        //
        // Swapping g_a ↔ g_b must change the factor to exp(g_b·chord/3 + g_a·2·chord/3),
        // proving the result depends on how long the ray spends in each cell.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let sigma_e = square_meter!(2.0e-24);
        let g_a = reciprocal_centimeter!(0.05_f64);
        let g_b = reciprocal_centimeter!(0.10_f64);

        let dims = (2, 1, 1);
        let ray = Ray::new(
            millimeter!(-5.0, 0.0, 0.0),
            Vector3::new(15.0, 0.0, THICKNESS),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let chord = body
            .path_length_inside(&ray)?
            .expect("the ray must cross the disk");

        // -- forward: g_a in cell (0,*,*), g_b in cell (1,*,*) --
        let inv_fwd = field_with(&body, dims, sigma_e, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;
        let model = SmallSignalGain::new(sigma_e, dims)?;
        let factor_fwd = traverse_factor(&body, &model, &mut Some(inv_fwd), &ray)?;
        let expected_fwd = f64::exp((g_a * chord / 3.0 + g_b * chord * 2.0 / 3.0).value);
        assert_relative_eq!(factor_fwd, expected_fwd, max_relative = 1e-12);

        // -- swapped: g_b in cell (0,*,*), g_a in cell (1,*,*) --
        let inv_swp = field_with(&body, dims, sigma_e, |cell| {
            Ok(if cell.0 == 0 { g_b } else { g_a })
        })?;
        let factor_swp = traverse_factor(&body, &model, &mut Some(inv_swp), &ray)?;
        let expected_swp = f64::exp((g_b * chord / 3.0 + g_a * chord * 2.0 / 3.0).value);
        assert_relative_eq!(factor_swp, expected_swp, max_relative = 1e-12);

        assert!(
            (factor_fwd - factor_swp).abs() > 1e-9,
            "swapping the coefficients must change the factor: {factor_fwd} vs {factor_swp}"
        );
        Ok(())
    }

    #[test]
    fn the_x_boundary_crossing_is_exact_not_approximate() -> OpmResult<()> {
        // The asymmetric two-cell setup (1/3 of the chord in A, 2/3 in B): exact voxel traversal
        // gives the analytic result at 1e-12 with no refinement at all, in contrast to the old
        // midpoint march which had O(1/n) error for a crossing not aligned with step boundaries.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let sigma_e = square_meter!(2.0e-24);
        let g_a = reciprocal_centimeter!(0.05_f64);
        let g_b = reciprocal_centimeter!(0.10_f64);
        let dims = (2, 1, 1);
        let ray = Ray::new(
            millimeter!(-5.0, 0.0, 0.0),
            Vector3::new(15.0, 0.0, THICKNESS),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let chord = body
            .path_length_inside(&ray)?
            .expect("the ray must cross the disk");
        let exact = f64::exp((g_a * chord / 3.0 + g_b * chord * 2.0 / 3.0).value);

        let model = SmallSignalGain::new(sigma_e, dims)?;
        let inv = field_with(&body, dims, sigma_e, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;
        let factor = traverse_factor(&body, &model, &mut Some(inv), &ray)?;
        assert_relative_eq!(factor, exact, max_relative = 1e-12);
        Ok(())
    }

    #[test]
    fn an_oblique_ray_through_a_beer_lambert_profile_matches_the_closed_form() -> OpmResult<()> {
        // A Beer-Lambert profile sets the gain density as g(z) = g0 · exp(-α·z) (forward pump).
        // For a ray travelling at angle θ to the optical axis, the path element is ds = dz/cosθ,
        // so the line integral over the disk thickness L is:
        //
        //   ∫₀ᴸ g(z) · dz/cosθ  =  g0/(α·cosθ) · (1 − exp(−α·L))
        //
        // With exact voxel traversal, the path-length weighting is exact; the remaining error
        // comes only from discretizing the profile onto cells_z slices. Finer cells_z → converges
        // toward the closed form. The 1/cosθ factor is naturally produced by the longer physical
        // path, not hard-coded.
        //
        // Ray: 45° in the y-z plane from (0, -5, 0) mm toward (0, 5, L). cosθ = 1/sqrt(2).
        // Disk is made wide so the ray does not escape through the barrel.
        let g_0 = reciprocal_centimeter!(0.5_f64);
        let alpha = reciprocal_centimeter!(2.0_f64);
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
        let body = disk(length, millimeter!(20.0))?;
        let cos_theta = 1.0_f64 / f64::sqrt(2.0);
        let exact =
            f64::exp(((g_0 / alpha / cos_theta) * (1.0 - f64::exp(-(alpha * length).value))).value);
        let oblique = Ray::new(
            millimeter!(0.0, -5.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;

        // Vary cells_z; transversal resolution is flat so 4×4 is exact there.
        let sigma_e = square_meter!(2.0e-24);
        let error = |cells_z: usize| -> OpmResult<f64> {
            let model = SmallSignalGain::new(sigma_e, (4, 4, cells_z))?;
            let factor = factor_through(&body, &model, &config, &oblique)?;
            Ok((factor - exact).abs() / exact)
        };
        let (coarse, fine) = (error(8)?, error(64)?);
        assert!(
            fine < coarse,
            "refining cells_z 8 → 64 did not help for oblique Beer-Lambert: {coarse} → {fine}"
        );
        assert!(
            fine < 1e-3,
            "oblique Beer-Lambert (cells_z=64) still off by {fine} vs \
             exp(g0/(α·cosθ)·(1−exp(−α·L))) = {exact}"
        );
        Ok(())
    }
}
