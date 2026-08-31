#![warn(missing_docs)]
//! Unsaturated amplification that follows the path a ray takes through the medium.
//!
//! The step beyond [`ConstGain`](super::ConstGain): where a constant factor multiplies every ray
//! alike, this one integrates the local gain along the chord the ray actually travels inside the
//! body, `G = exp(∫ g₀·β ds)`, where `g₀` is the peak small-signal gain coefficient this model
//! carries and `β` the normalized inversion the pump deposited. Two rays crossing the same medium
//! therefore leave with different factors — an oblique one gains over a longer path, one passing the
//! rim of a shaped pump profile gains less than one on the axis.
//!
//! **The inversion is frozen.** Extracting energy here does not draw the medium down, so a second
//! pass sees exactly what the first one saw. That is what makes the model "small signal": it holds
//! as long as the extracted energy is negligible against the stored energy. Saturation is the next
//! stage and is what will start writing back into the
//! [`InversionField`](super::InversionField).
//!
//! **Deliberate non-goals at this stage**, both deferred rather than forgotten:
//!
//! - *No wavelength dependence.* [`MonochromaticSmallSignalGain::peak_gain_coefficient`] is one
//!   number, not a `g₀(λ)` curve, so the gain of a ray does not depend on its colour. Gain narrowing
//!   and the red shift of a chirped pulse need the spectral stage — a sibling model reading the same
//!   `β`.
//! - *No saturation and no extraction warning.* Nothing is drawn out of the medium, so there is
//!   nothing that could be overdrawn.

use super::{
    extraction::Extraction,
    inversion_field::{Inversion, InversionField},
    pump_source::PumpSource,
    scenario::PumpConfig,
};
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::AllFinite,
    geometry::body::Body,
    light::{Ray, Spectrum},
    reciprocal_centimeter, validated, validated_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::ReciprocalLength;
use utoipa::ToSchema;

/// A small-signal gain coefficient that is guaranteed to be finite.
///
/// Deliberately **not** constrained to be positive: a negative coefficient describes a medium that
/// absorbs where an amplifier would amplify — the same physics with the inversion turned around, and
/// the state an unpumped doped medium is actually in.
type ValidatedGainCoefficient = validated_type!(ReciprocalLength, AllFinite);
impl Default for ValidatedGainCoefficient {
    /// No gain at all: a model that changes no result until a coefficient is dialled in.
    fn default() -> Self {
        validated!(reciprocal_centimeter!(0.0), AllFinite).unwrap()
    }
}

/// Parameters of an unsaturated gain that follows the path through the medium.
///
/// See the [module documentation](self) for what the model does and what it deliberately does not.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated)]
#[serde(try_from = "NonValidatedMonochromaticSmallSignalGain")]
pub struct MonochromaticSmallSignalGain {
    #[schema(value_type = f64)]
    peak_gain_coefficient: ValidatedGainCoefficient,
}

/// Deserialization shim for [`MonochromaticSmallSignalGain`].
///
/// It lets the value read from an `.opm` file run through the very same validation as one set
/// through the setter, so a hand-edited file cannot smuggle in a non-finite coefficient. Same
/// pattern as [`ConstGain`](super::ConstGain).
#[derive(Deserialize)]
struct NonValidatedMonochromaticSmallSignalGain {
    peak_gain_coefficient: ReciprocalLength,
}
impl TryFrom<NonValidatedMonochromaticSmallSignalGain> for MonochromaticSmallSignalGain {
    type Error = String;
    fn try_from(helper: NonValidatedMonochromaticSmallSignalGain) -> Result<Self, Self::Error> {
        Self::new(helper.peak_gain_coefficient).map_err(|e| e.to_string())
    }
}

impl Default for MonochromaticSmallSignalGain {
    /// A neutral [`MonochromaticSmallSignalGain`] whose peak gain coefficient is zero.
    ///
    /// Picking this model must not change a result on its own, and it does not: with a peak
    /// coefficient of zero the integral is zero and the gain is exactly one, whatever the medium is
    /// pumped to.
    fn default() -> Self {
        Self {
            peak_gain_coefficient: ValidatedGainCoefficient::default(),
        }
    }
}

impl MonochromaticSmallSignalGain {
    /// Create a new [`MonochromaticSmallSignalGain`] with the given peak gain coefficient.
    ///
    /// # Arguments
    ///
    /// * `peak_gain_coefficient` - g₀ where the pump reaches its peak, see
    ///   [`MonochromaticSmallSignalGain::peak_gain_coefficient`].
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the coefficient is
    /// not finite.
    pub fn new(peak_gain_coefficient: ReciprocalLength) -> OpmResult<Self> {
        let mut model = Self::default();
        model.set_peak_gain_coefficient(peak_gain_coefficient)?;
        Ok(model)
    }
    /// Return g₀, the small-signal gain coefficient where the pump reaches its peak (`β = 1`).
    ///
    /// This is the whole magnitude of the amplification: the pump supplies only the shape `β`, and
    /// the local gain per unit length is `g₀ · β`. A negative value makes the medium absorbing.
    /// Total single-pass gain relates as `G₀ = exp(g₀ · L)`. It becomes a spectroscopic input —
    /// `σ_e(λ)` times an absolute inversion — only once the spectral stage replaces this single
    /// number with a curve.
    #[must_use]
    pub const fn peak_gain_coefficient(&self) -> ReciprocalLength {
        *self.peak_gain_coefficient.get()
    }
    /// Set g₀, the peak small-signal gain coefficient.
    ///
    /// # Arguments
    ///
    /// * `peak_gain_coefficient` - g₀ where the pump reaches its peak.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if the given value is
    /// not finite. The previous value is kept in that case.
    pub fn set_peak_gain_coefficient(
        &mut self,
        peak_gain_coefficient: ReciprocalLength,
    ) -> OpmResult<()> {
        self.peak_gain_coefficient.set(peak_gain_coefficient)
    }
}

impl Extraction for MonochromaticSmallSignalGain {
    fn name(&self) -> &'static str {
        "Small-Signal-Gain"
    }
    fn needs_inversion(&self) -> bool {
        // The whole point of the stage: what a beam gains is what the medium holds where the beam
        // went, so how the medium was pumped is an input.
        true
    }
    /// Prepare the inversion the operating point describes for this model to read.
    ///
    /// This is the one place the two halves of a [`PumpConfig`] meet, and where the const pump earns
    /// its keep: a [`PumpSource::None`] leaves an unpumped `Uniform(0.0)`, a [`PumpSource::Const`] a
    /// `Uniform(1.0)` — neither needs a grid, both integrate over the exact chord — and only a
    /// [`PumpSource::Analytic`] lays out an [`InversionField`] and has the pump write its shape into
    /// it. Neither half knows the other; the field, or the uniform number, is all they share.
    fn build_inversion(
        &self,
        body: &dyn Body,
        config: &PumpConfig,
    ) -> OpmResult<Option<Inversion>> {
        Ok(Some(match config.pump() {
            PumpSource::None => Inversion::Uniform(0.0),
            PumpSource::Const => Inversion::Uniform(1.0),
            PumpSource::Analytic(pump) => {
                let mut field = InversionField::from_body(body, pump.grid())?;
                pump.deposit_shape(&mut field)?;
                Inversion::Field(field)
            }
        }))
    }
    fn path_exponent(&self, body: &dyn Body, ray: &Ray, inversion: &mut Option<Inversion>) -> f64 {
        match inversion.as_ref() {
            None => 0.0,
            // A uniform inversion needs no grid: the local gain is the same everywhere, so the
            // exponent is the coefficient times β times the exact chord the ray travels through the
            // body. This is what makes the const pump both cheaper and more accurate than voxelising
            // a uniform medium would be.
            Some(Inversion::Uniform(beta)) => match body.path_length_inside(ray) {
                Ok(Some(chord)) if chord.value > 0.0 => {
                    (self.peak_gain_coefficient() * chord).value * *beta
                }
                _ => 0.0,
            },
            Some(Inversion::Field(field)) => {
                use uom::si::{f64::Length, length::meter};

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

                // The exact chord gives the body's extent along the ray: t = 0 at the entrance
                // surface (where the refracted ray starts) and t = t_body_exit at the exit
                // surface. Clipping each cell's ds to [0, t_body_exit] corrects two
                // boundary-voxel errors that arise at curved surfaces:
                //   - exit voxel: center-point mask says "inside", but the full DDA ds extends
                //     past the curved exit surface → clip to t_body_exit.
                //   - entry voxel: center-point mask says "outside" because the curved entrance
                //     surface cuts through the voxel, but the clipped segment already lies
                //     inside the body → the midpoint check below catches it.
                let t_body_exit = match body.path_length_inside(ray) {
                    Ok(Some(chord)) if chord.value > 0.0 => chord.value,
                    _ => return 0.0,
                };

                let cells = field.traverse(&local_origin, &local_dir);
                // A ghost ray exiting at the lower boundary of the grid (traveling outward) produces
                // an empty forward traversal because t_exit == t_enter == 0. For non-saturating
                // small-signal gain the integral is path-symmetric, so falling back to the reverse
                // direction recovers the full chord without affecting the computed gain magnitude.
                let cells = if cells.is_empty() {
                    field.traverse(&local_origin, &(-local_dir))
                } else {
                    cells
                };

                // Accumulate t along the traversal so the [t_start, t_end] interval of each cell
                // is known — traverse() yields only ds, not the absolute position along the ray.
                let mut t_cur = 0.0_f64;
                let mut exponent = 0.0_f64;
                for (cell, ds) in cells {
                    let t_start = t_cur;
                    let t_end = t_cur + ds.value;
                    t_cur = t_end;

                    let t_lo = t_start.max(0.0);
                    let t_hi = t_end.min(t_body_exit);
                    if t_hi <= t_lo {
                        continue;
                    }
                    let effective_ds = Length::new::<meter>(t_hi - t_lo);

                    let beta = if field.is_inside(cell) {
                        // Interior or exit-boundary cell: center-point mask confirms medium;
                        // t-clipping already trims any overshoot past the exit surface.
                        field.population(cell)
                    } else {
                        // Entry-boundary voxel or lateral exterior cell: center-point mask says
                        // "outside", but the clipped segment [t_lo, t_hi] may still lie inside
                        // the body. Check the midpoint of the clipped segment in world coordinates.
                        let t_mid = f64::midpoint(t_lo, t_hi);
                        let displacement = local_dir.map(|v| Length::new::<meter>(v * t_mid));
                        let world_mid = iso.transform_point(&(local_origin + displacement));
                        if body.contains(&world_mid).unwrap_or(false) {
                            field.population(cell)
                        } else {
                            None
                        }
                    };

                    let Some(beta) = beta else {
                        continue;
                    };
                    exponent = (self.peak_gain_coefficient() * effective_ds)
                        .value
                        .mul_add(beta, exponent);
                }
                exponent
            }
        }
    }
    fn amplify_spectrum(
        &self,
        _body: &dyn Body,
        _inversion: Option<&Inversion>,
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
            AnalyticPump, BeerLambertProfile, GainModel, LongitudinalProfile, PumpDirection,
            PumpSource, TransversalProfile,
            inversion_field::{CellIndex, cells},
        },
        geometry::{Plane, Sphere, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        joule,
        light::Ray,
        millimeter, nanometer, reciprocal_centimeter,
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
    /// A uniform (const) operating point. The magnitude lives on the model handed to the factor
    /// helpers, so the pump here carries only "uniformly inverted".
    fn const_pump() -> PumpConfig {
        PumpConfig::new(GainModel::None, PumpSource::Const)
    }
    /// A shaped operating point with the given profiles and grid.
    fn analytic(
        transversal: TransversalProfile,
        longitudinal: LongitudinalProfile,
        grid: CellIndex,
    ) -> OpmResult<PumpConfig> {
        Ok(PumpConfig::new(
            GainModel::None,
            PumpSource::Analytic(AnalyticPump::new(transversal, longitudinal, grid)?),
        ))
    }
    /// Compute the gain factor a ray picks up traversing the body via the production path_exponent.
    ///
    /// This calls production code rather than reimplementing the integration, so the tests actually
    /// exercise what runs in the analysis. Keeping `inversion` mutable lets `the_inversion_is_frozen`
    /// verify that [`MonochromaticSmallSignalGain`] does not write back into it.
    fn traverse_factor(
        body: &dyn Body,
        model: &MonochromaticSmallSignalGain,
        inversion: &mut Option<Inversion>,
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
        model: &MonochromaticSmallSignalGain,
        config: &PumpConfig,
        ray: &Ray,
    ) -> OpmResult<f64> {
        let mut inversion = model.build_inversion(body, config)?;
        traverse_factor(body, model, &mut inversion, ray)
    }
    /// The factor a ray picks up crossing the standard [`test_disk`].
    fn factor_through_disk(
        model: &MonochromaticSmallSignalGain,
        config: &PumpConfig,
        ray: &Ray,
    ) -> OpmResult<f64> {
        factor_through(&test_disk()?, model, config, ray)
    }

    #[test]
    fn the_default_is_neutral() {
        // Picking the model must not change a result on its own, so its peak gain coefficient is
        // zero: whatever the medium is pumped to, `g₀ · β = 0`.
        assert_relative_eq!(
            MonochromaticSmallSignalGain::default()
                .peak_gain_coefficient()
                .value,
            0.0
        );
    }
    #[test]
    fn new_keeps_what_it_was_given() -> OpmResult<()> {
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        assert_relative_eq!(
            model.peak_gain_coefficient().value,
            reciprocal_centimeter!(0.5).value
        );
        Ok(())
    }
    #[test]
    fn a_non_finite_coefficient_is_refused() {
        for refused in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                MonochromaticSmallSignalGain::new(reciprocal_centimeter!(refused)).is_err(),
                "a coefficient of {refused} should be refused"
            );
        }
        // A negative coefficient is fine though: it is an absorbing medium.
        assert!(MonochromaticSmallSignalGain::new(reciprocal_centimeter!(-0.5)).is_ok());
    }
    #[test]
    fn a_rejected_value_keeps_the_old_one() -> OpmResult<()> {
        // A half-typed value in the GUI must not damage what is already configured.
        let mut model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        assert!(
            model
                .set_peak_gain_coefficient(reciprocal_centimeter!(f64::NAN))
                .is_err()
        );
        assert_relative_eq!(
            model.peak_gain_coefficient().value,
            reciprocal_centimeter!(0.5).value
        );
        Ok(())
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        let serialized = ron::to_string(&model).map_err(|e| OpossumError::Other(e.to_string()))?;
        let deserialized: MonochromaticSmallSignalGain =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(model, deserialized);
        Ok(())
    }
    #[test]
    fn a_homogeneous_medium_amplifies_by_exp_g0_l() -> OpmResult<()> {
        // The defining case: a uniformly pumped medium gives `G = exp(g0 * L)`, and for an on-axis
        // ray through a plane-parallel disk `L` is exactly the thickness. The const pump builds no
        // field: the exponent comes straight from the exact chord.
        let g_0 = reciprocal_centimeter!(0.5);
        let model = MonochromaticSmallSignalGain::new(g_0)?;
        let factor = factor_through_disk(&model, &const_pump(), &ray_at(0.0, 0.0)?)?;
        assert_relative_eq!(
            factor,
            f64::exp((g_0 * millimeter!(THICKNESS)).value),
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn a_negative_coefficient_absorbs() -> OpmResult<()> {
        // A negative peak coefficient is the same physics with the inversion turned around, so it
        // has to come out as plain Beer-Lambert absorption over the very same path.
        let g_0 = reciprocal_centimeter!(-0.5);
        let model = MonochromaticSmallSignalGain::new(g_0)?;
        let factor = factor_through_disk(&model, &const_pump(), &ray_at(0.0, 0.0)?)?;
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
        // Even a model with a nonzero coefficient leaves an unpumped medium (β = 0) untouched: the
        // pump, not the model, decides whether there is any inversion to amplify.
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
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
        // a plane-parallel disk is longer by sqrt(2), and the gain has to follow — even for the
        // const pump, whose exponent is `g₀` times the exact chord.
        //
        // The disk is deliberately made wider than the standard one here: at this angle the ray
        // wanders a full thickness across the cross section on its way through, and one that leaves
        // sideways does not pass through the body at all - see
        // `a_ray_that_does_not_cross_the_medium_is_not_amplified` for that case.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let g_0 = reciprocal_centimeter!(0.5);
        let model = MonochromaticSmallSignalGain::new(g_0)?;
        let oblique = Ray::new(
            millimeter!(0.0, -5.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let factor = factor_through(&body, &model, &const_pump(), &oblique)?;
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
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        let escaping = Ray::new(
            millimeter!(0.0, -3.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(factor_through_disk(&model, &const_pump(), &escaping)?, 1.0);
        Ok(())
    }
    #[test]
    fn the_integral_converges_when_the_grid_is_refined() -> OpmResult<()> {
        // With exact voxel traversal the only remaining error is the discretization of the
        // inversion profile onto the grid cells. Beer-Lambert has a closed form:
        // the integral of g0·exp(-α·s) from 0 to L is g0/α · (1 − exp(-α·L)).
        // Finer cells_z → smaller staircase approximation error → factor converges to exact. The
        // grid is a property of the analytic pump now, so refining it means refining the pump.
        let g_0 = reciprocal_centimeter!(0.5);
        let alpha = reciprocal_centimeter!(2.0);
        let length = millimeter!(THICKNESS);
        let model = MonochromaticSmallSignalGain::new(g_0)?;
        let exact = f64::exp(((g_0 / alpha) * (1.0 - f64::exp(-(alpha * length).value))).value);
        let ray = ray_at(0.0, 0.0)?;
        let error = |cells_z: usize| -> OpmResult<f64> {
            let config = analytic(
                TransversalProfile::Flat,
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    alpha,
                    PumpDirection::Forward,
                )?),
                (4, 4, cells_z),
            )?;
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
        // An odd cell count puts one column of cells exactly on the axis, so the axial ray really
        // samples the peak of the profile rather than a neighbour of it.
        let config = analytic(
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                Point2::new(millimeter!(0.0), millimeter!(0.0)),
                Point2::new(sigma, sigma),
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,
            (65, 65, 8),
        )?;
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
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
        // down, so a second pass sees exactly what the first one saw. A shaped pump is used so there
        // is a real field that could, in principle, be written back into.
        let body = test_disk()?;
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        let config = analytic(
            TransversalProfile::Flat,
            LongitudinalProfile::Flat,
            (8, 8, 8),
        )?;
        let mut inversion = model.build_inversion(&body, &config)?;
        let untouched = inversion.clone();
        let ray = ray_at(0.0, 0.0)?;
        let first = traverse_factor(&body, &model, &mut inversion, &ray)?;
        let second = traverse_factor(&body, &model, &mut inversion, &ray)?;
        // The model must not write back into the inversion — both passes see identical state.
        assert_eq!(inversion, untouched);
        assert_relative_eq!(first, second);
        Ok(())
    }
    #[test]
    fn a_ray_that_does_not_cross_the_medium_is_not_amplified() -> OpmResult<()> {
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(0.5))?;
        // beside the disk, running past it ...
        let beside = Ray::new(
            millimeter!(20.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(factor_through_disk(&model, &const_pump(), &beside)?, 1.0);
        // ... and one pointing away from it, which never reaches it either
        let away = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, -1.0),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        assert_relative_eq!(factor_through_disk(&model, &const_pump(), &away)?, 1.0);
        Ok(())
    }
    #[test]
    fn a_gain_that_would_overflow_is_refused() -> OpmResult<()> {
        // An absurd operating point has to stop the analysis rather than hand an infinite energy
        // down the graph, where it would only surface much later as a meaningless report.
        let model = MonochromaticSmallSignalGain::new(reciprocal_centimeter!(1.0e6))?;
        assert!(factor_through_disk(&model, &const_pump(), &ray_at(0.0, 0.0)?).is_err());
        Ok(())
    }
    #[test]
    fn a_hand_edited_file_cannot_smuggle_past_the_validation() {
        // The shim is what makes reading a file go through the very same setter as the GUI does.
        for refused in ["(peak_gain_coefficient:NaN)", "(peak_gain_coefficient:inf)"] {
            assert!(
                ron::from_str::<MonochromaticSmallSignalGain>(refused).is_err(),
                "the file content {refused} should be refused"
            );
        }
        // ... while a finite coefficient reads back fine. 50 per meter is 0.5 per centimeter.
        assert!(
            ron::from_str::<MonochromaticSmallSignalGain>("(peak_gain_coefficient:50)").is_ok()
        );
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

    /// An `InversionField` over `body` whose in-medium cells carry the normalized inversion β that
    /// makes the local gain coefficient equal `g(cell)` for a model of the given `peak`: since the
    /// model reads `g = peak · β`, that is `β = g / peak`.
    ///
    /// Bypasses the pump/profile path so tests can prescribe an exact piecewise-constant field and
    /// compute analytic expectations without profile-discretisation error.
    fn field_with(
        body: &dyn Body,
        dims: CellIndex,
        peak: ReciprocalLength,
        g: impl Fn(CellIndex) -> OpmResult<ReciprocalLength>,
    ) -> OpmResult<InversionField> {
        let mut field = InversionField::from_body(body, dims)?;
        for cell in cells(dims) {
            if field.is_inside(cell) {
                field.set_population(cell, (g(cell)? / peak).value)?;
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
        // A unit peak makes β numerically equal to the gain coefficient it stands for.
        let peak = reciprocal_centimeter!(1.0);
        let g_a = reciprocal_centimeter!(0.3_f64);
        let g_b = reciprocal_centimeter!(0.8_f64);

        let inversion = field_with(&body, (2, 1, 1), peak, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;

        // A ray from (-5, 0, 0) mm toward (5, 0, L) crosses x = 0 exactly at half the chord.
        let ray = Ray::new(
            millimeter!(-5.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, THICKNESS),
            nanometer!(1054.0),
            joule!(1.0),
        )?;
        let model = MonochromaticSmallSignalGain::new(peak)?;
        let factor = traverse_factor(&body, &model, &mut Some(Inversion::Field(inversion)), &ray)?;

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
        // Swapping g_a ↔ g_b must change the factor to exp(g_b·chord/3 + g_a·2·chord/3),
        // proving the result depends on how long the ray spends in each cell.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let peak = reciprocal_centimeter!(1.0);
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
        let inv_fwd = field_with(&body, dims, peak, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;
        let model = MonochromaticSmallSignalGain::new(peak)?;
        let factor_fwd =
            traverse_factor(&body, &model, &mut Some(Inversion::Field(inv_fwd)), &ray)?;
        let expected_fwd = f64::exp((g_a * chord / 3.0 + g_b * chord * 2.0 / 3.0).value);
        assert_relative_eq!(factor_fwd, expected_fwd, max_relative = 1e-12);

        // -- swapped: g_b in cell (0,*,*), g_a in cell (1,*,*) --
        let inv_swp = field_with(&body, dims, peak, |cell| {
            Ok(if cell.0 == 0 { g_b } else { g_a })
        })?;
        let factor_swp =
            traverse_factor(&body, &model, &mut Some(Inversion::Field(inv_swp)), &ray)?;
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
        // gives the analytic result at 1e-12 with no refinement at all.
        let body = disk(millimeter!(THICKNESS), millimeter!(20.0))?;
        let peak = reciprocal_centimeter!(1.0);
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

        let model = MonochromaticSmallSignalGain::new(peak)?;
        let inv = field_with(&body, dims, peak, |cell| {
            Ok(if cell.0 == 0 { g_a } else { g_b })
        })?;
        let factor = traverse_factor(&body, &model, &mut Some(Inversion::Field(inv)), &ray)?;
        assert_relative_eq!(factor, exact, max_relative = 1e-12);
        Ok(())
    }

    #[test]
    fn an_oblique_ray_through_a_beer_lambert_profile_matches_the_closed_form() -> OpmResult<()> {
        // A Beer-Lambert profile sets the gain as g(z) = g0 · exp(-α·z) (forward pump).
        // For a ray travelling at angle θ to the optical axis, the path element is ds = dz/cosθ,
        // so the line integral over the disk thickness L is:
        //
        //   ∫₀ᴸ g(z) · dz/cosθ  =  g0/(α·cosθ) · (1 − exp(−α·L))
        //
        // With exact voxel traversal, the path-length weighting is exact; the remaining error
        // comes only from discretizing the profile onto the pump's cells_z slices. Finer cells_z →
        // converges toward the closed form. The 1/cosθ factor is naturally produced by the longer
        // physical path, not hard-coded.
        //
        // Ray: 45° in the y-z plane from (0, -5, 0) mm toward (0, 5, L). cosθ = 1/sqrt(2).
        // Disk is made wide so the ray does not escape through the barrel.
        let g_0 = reciprocal_centimeter!(0.5_f64);
        let alpha = reciprocal_centimeter!(2.0_f64);
        let length = millimeter!(THICKNESS);
        let model = MonochromaticSmallSignalGain::new(g_0)?;
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

        // Vary cells_z on the pump; transversal resolution is flat so 4×4 is exact there.
        let error = |cells_z: usize| -> OpmResult<f64> {
            let config = analytic(
                TransversalProfile::Flat,
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    alpha,
                    PumpDirection::Forward,
                )?),
                (4, 4, cells_z),
            )?;
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

    #[test]
    fn a_curved_exit_surface_clips_the_boundary_voxel() -> OpmResult<()> {
        // Regression test for boundary-voxel clipping at curved surfaces.
        //
        // Geometry: sphere centered at the origin, radius R = 20 mm.
        //   - Flat entrance at z = 0.
        //   - Convex spherical exit: vertex at z = R (the on-axis maximum), center at
        //     z = 0. Using a negative stored radius −R places the center at
        //     vertex_z + (−R) = R − R = 0, i.e. the origin.
        //   - Sphere equation (front hemisphere): z_exit(x) = √(R² − x²).
        //
        // At x = R·√3/2 the exit is at z = R/2, giving chord = R/2 — exactly half the
        // on-axis chord R. With a single (1×1×1) voxel spanning the bounding box
        // [0, R] in z, the old code assigns ds = R (the full box) and overcounts by 2×.
        // The fix clips ds to t_body_exit = R/2, so the Field exponent must equal the
        // Uniform (exact-chord) exponent to within floating-point precision.
        let radius_mm = 20.0_f64; // R
        let peak = reciprocal_centimeter!(1.0);

        // Sphere vertex at z = R, center at z = 0 (origin).
        // Aperture = 18 mm: must contain the ray at x = R·√3/2 ≈ 17.3 mm, and must be
        // strictly less than R so the sphere does not degenerate at the aperture edge
        // (at x = R the sphere would reach z = 0 = entrance, giving zero thickness).
        let aperture_mm = 18.0_f64;
        let entrance = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity()))));
        let exit = GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new_at_position(
            millimeter!(-radius_mm), // stored radius −R → center at vertex_z − R = 0
            millimeter!(0.0, 0.0, radius_mm), // vertex at z = R
        )?)));
        let body = SurfaceBoundedBody::new(
            entrance,
            exit,
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(aperture_mm),
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        );

        // At x = R·√3/2 the chord equals R/2 exactly: √(R² − 3R²/4) = R/2.
        let x_ray_mm = radius_mm * f64::sqrt(3.0) / 2.0;
        let ray = ray_at(x_ray_mm, 0.0)?;
        let model = MonochromaticSmallSignalGain::new(peak)?;

        // Exact exponent via the Uniform path (uses path_length_inside directly).
        let chord = body
            .path_length_inside(&ray)?
            .expect("off-axis ray must cross the plano-spherical body");
        let exact_exponent = (peak * chord).value;

        // Voxel exponent via the Field path with a single cell (β = 1 everywhere).
        // One cell makes the boundary error maximal: ds spans the full bounding box.
        let inv = field_with(&body, (1, 1, 1), peak, |_| Ok(peak))?;
        let voxel_exponent =
            Extraction::path_exponent(&model, &body, &ray, &mut Some(Inversion::Field(inv)));

        assert_relative_eq!(voxel_exponent, exact_exponent, max_relative = 1e-10);
        Ok(())
    }

    #[test]
    fn a_curved_entrance_surface_clips_the_boundary_voxel() -> OpmResult<()> {
        // Regression test for boundary-voxel clipping at curved surfaces.
        //
        // Geometry: convex spherical entrance, flat exit.
        //   - Sphere center at z = R, radius R = 20 mm; vertex (minimum z of surface) at z = 0.
        //     Using a positive stored radius +R places the center at vertex_z + R = 0 + R = R.
        //   - Sphere equation (lower hemisphere): z_entrance(x) = R − √(R² − x²).
        //   - Flat exit at z = R.
        //
        // At x = R·√3/2 the entrance is at z = R/2, giving chord = R − R/2 = R/2 — exactly
        // half the on-axis chord R. With a single (1×1×1) voxel spanning the bounding box
        // [0, R] in z, the old code assigns ds = R (the full box) and overcounts by 2×.
        // The fix clips ds to t_body_enter = R/2, so the Field exponent must equal the
        // Uniform (exact-chord) exponent to within floating-point precision.
        let radius_mm = 20.0_f64; // R
        let peak = reciprocal_centimeter!(1.0);

        // Sphere vertex at z = 0, center at z = R.
        // Aperture = 18 mm: must contain the ray at x = R·√3/2 ≈ 17.3 mm, and must be
        // strictly less than R so the sphere does not degenerate at the aperture edge
        // (at x = R the entrance would reach z = R = exit, giving zero thickness).
        let aperture_mm = 18.0_f64;
        let entrance = GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new_at_position(
            millimeter!(radius_mm),     // stored radius +R → center at vertex_z + R = R
            millimeter!(0.0, 0.0, 0.0), // vertex at z = 0
        )?)));
        let exit = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(
            Isometry::new_along_z(millimeter!(radius_mm))?, // flat exit at z = R
        ))));
        let body = SurfaceBoundedBody::new(
            entrance,
            exit,
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(aperture_mm),
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        );

        // At x = R·√3/2 the entrance is at z = R/2, giving chord R − R/2 = R/2 exactly.
        let x_ray_mm = radius_mm * f64::sqrt(3.0) / 2.0;
        let ray = ray_at(x_ray_mm, 0.0)?;
        let model = MonochromaticSmallSignalGain::new(peak)?;

        // Exact exponent via the Uniform path (uses path_length_inside directly).
        let chord = body
            .path_length_inside(&ray)?
            .expect("off-axis ray must cross the plano-spherical body");
        let exact_exponent = (peak * chord).value;

        // Voxel exponent via the Field path with a single cell (β = 1 everywhere).
        // One cell makes the boundary error maximal: ds spans the full bounding box.
        let inv = field_with(&body, (1, 1, 1), peak, |_| Ok(peak))?;
        let voxel_exponent =
            Extraction::path_exponent(&model, &body, &ray, &mut Some(Inversion::Field(inv)));

        assert_relative_eq!(voxel_exponent, exact_exponent, max_relative = 1e-10);
        Ok(())
    }
}
