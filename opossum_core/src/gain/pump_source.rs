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
//! **A pump source describes only a shape.** It writes the normalized inversion `β ∈ [0, 1]` — the
//! local fraction of the peak inversion — into the field, and nothing about *how hard* the medium is
//! pumped: that magnitude is the small-signal gain coefficient the [`GainModel`](super::GainModel)
//! carries, stated once there rather than split across every pump variant. A uniform pump is the
//! shapeless case, `β = 1` everywhere; it needs no grid at all and is handled as
//! [`Inversion::Uniform`](super::Inversion) by the model, so only the [`AnalyticPump`] ever deposits
//! into a field. Keeping the field spectroscopy-free is what lets a future three-level model read
//! the very same `β`.

use super::inversion_field::{CellIndex, InversionField, cells};
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive},
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
use uom::si::f64::{Length, ReciprocalLength};
use utoipa::ToSchema;

/// A number of cells that is guaranteed to be non-zero.
///
/// The grid a shaped pump is resolved onto lives here, on the [`AnalyticPump`], because it only
/// means something where there is a profile to resolve — a uniform pump has no shape and needs no
/// grid.
type ValidatedCellCount = validated_type!(usize, AllNotZero);

/// How many cells a shaped pump is discretised into along each axis by default.
const DEFAULT_TRANSVERSAL_CELLS: usize = 128;
const DEFAULT_LONGITUDINAL_CELLS: usize = 16;

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

/// Deserialization shim for [`BeerLambertProfile`], mirroring [`NonValidatedAnalyticPump`].
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
/// The Lambert-Beer law: a pump entering one face is attenuated by `exp(-α·s)` after a depth `s`,
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
    /// Return the absorption coefficient of the medium at the pump wavelength.
    #[must_use]
    pub const fn absorption(&self) -> ReciprocalLength {
        *self.absorption.get()
    }
    /// Return which face the pump enters through.
    #[must_use]
    pub const fn direction(&self) -> PumpDirection {
        self.direction
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

/// Deserialization shim for [`AnalyticPump`].
///
/// It lets the values read from an `.opm` file run through the very same validation as ones set
/// through the constructor, so a hand-edited file cannot smuggle in a zero-cell grid. Same pattern
/// as [`ConstGain`](super::ConstGain).
#[derive(Deserialize)]
struct NonValidatedAnalyticPump {
    transversal: TransversalProfile,
    longitudinal: LongitudinalProfile,
    cells_x: usize,
    cells_y: usize,
    cells_z: usize,
}
impl TryFrom<NonValidatedAnalyticPump> for AnalyticPump {
    type Error = String;
    fn try_from(helper: NonValidatedAnalyticPump) -> Result<Self, Self::Error> {
        Self::new(
            helper.transversal,
            helper.longitudinal,
            (helper.cells_x, helper.cells_y, helper.cells_z),
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
/// Both profiles are peak-normalised, so their product — the normalized inversion `β` this pump
/// deposits — reaches 1 exactly where they both peak: on the axis of the spot, at the face the pump
/// enters through. The gain coefficient that peak stands for is not stated here but on the
/// [`GainModel`](super::GainModel); this side only carries the shape and the grid it is resolved on.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, EnsureValidated)]
#[serde(try_from = "NonValidatedAnalyticPump")]
pub struct AnalyticPump {
    #[validate(skip)]
    transversal: TransversalProfile,
    #[validate(skip)]
    longitudinal: LongitudinalProfile,
    #[schema(value_type = usize)]
    cells_x: ValidatedCellCount,
    #[schema(value_type = usize)]
    cells_y: ValidatedCellCount,
    #[schema(value_type = usize)]
    cells_z: ValidatedCellCount,
}
impl Default for AnalyticPump {
    /// A shapeless pump on a moderate grid: flat across the face and along the axis.
    ///
    /// Flat both ways is `β = 1` everywhere, so a freshly selected analytic pump is the uniform one
    /// until a profile is dialled in — and the grid is a usable default rather than a size nobody
    /// asked for.
    fn default() -> Self {
        Self {
            transversal: TransversalProfile::Flat,
            longitudinal: LongitudinalProfile::Flat,
            cells_x: validated!(DEFAULT_TRANSVERSAL_CELLS, AllNotZero).unwrap(),
            cells_y: validated!(DEFAULT_TRANSVERSAL_CELLS, AllNotZero).unwrap(),
            cells_z: validated!(DEFAULT_LONGITUDINAL_CELLS, AllNotZero).unwrap(),
        }
    }
}
impl AnalyticPump {
    /// Create a new [`AnalyticPump`].
    ///
    /// # Arguments
    ///
    /// * `transversal` - how the pump is distributed across the cross section.
    /// * `longitudinal` - how it is distributed along the optical axis.
    /// * `grid` - how many cells the shape is resolved onto along the body's x, y and z axis.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if any of the three grid
    /// counts is zero.
    pub fn new(
        transversal: TransversalProfile,
        longitudinal: LongitudinalProfile,
        grid: CellIndex,
    ) -> OpmResult<Self> {
        let mut pump = Self {
            transversal,
            longitudinal,
            ..Self::default()
        };
        pump.set_grid(grid)?;
        Ok(pump)
    }
    /// Return how the pump is distributed across the cross section.
    #[must_use]
    pub const fn transversal(&self) -> TransversalProfile {
        self.transversal
    }
    /// Return how the pump is distributed along the optical axis.
    #[must_use]
    pub const fn longitudinal(&self) -> LongitudinalProfile {
        self.longitudinal
    }
    /// Return how many cells the shape is resolved onto along the body's x, y and z axis.
    ///
    /// This is what an [`InversionField`] is laid out with, so it bounds how finely the profile can
    /// be resolved. It is the sole convergence parameter for the gain integration.
    #[must_use]
    pub const fn grid(&self) -> CellIndex {
        (
            *self.cells_x.get(),
            *self.cells_y.get(),
            *self.cells_z.get(),
        )
    }
    /// Set how many cells the shape is resolved onto along the body's x, y and z axis.
    ///
    /// # Arguments
    ///
    /// * `grid` - the number of cells along each axis, each at least one.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Other`](crate::error::OpossumError::Other) if any of the three
    /// counts is zero. The previous grid is kept in that case, as a whole: a grid that took only the
    /// axes that happened to be valid would be a size nobody asked for.
    pub fn set_grid(&mut self, grid: CellIndex) -> OpmResult<()> {
        let (x, y, z) = grid;
        // All three are validated before any of them is written, so a rejected axis cannot leave the
        // grid half updated.
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
    /// The product of the two profiles there — the normalized inversion `β`, at most 1.
    fn weight_at(&self, position: &Point3<Length>, bounds: &BoundingBox) -> f64 {
        self.transversal
            .value_at(&Point2::new(position.x, position.y))
            * self.longitudinal.value_at(position.z, &bounds.z_range())
    }
    /// Write the normalized inversion `β` this pump's shape produces into the given field.
    ///
    /// Only cells that hold medium are written; the rest of the grid spans the body's bounding box
    /// but not the body itself, and there is nothing out there to excite. No cross section and no
    /// magnitude enter here — the field keeps the pure shape, and the [`GainModel`](super::GainModel)
    /// turns it into a gain.
    ///
    /// # Arguments
    ///
    /// * `field` - the field to fill, already laid out over the medium with this pump's grid.
    ///
    /// # Errors
    ///
    /// This function errors only if a cell reported inside the grid cannot be written, which is a
    /// programming error rather than a configuration one.
    pub fn deposit_shape(&self, field: &mut InversionField) -> OpmResult<()> {
        let bounds = field.bounds();
        for cell in cells(field.dimensions()) {
            if !field.is_inside(cell) {
                continue;
            }
            let Some(center) = field.cell_center(cell) else {
                continue;
            };
            field.set_population(cell, self.weight_at(&center, &bounds))?;
        }
        Ok(())
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
    /// A uniform inversion throughout the medium: `β = 1` everywhere it reaches.
    ///
    /// It carries no parameters — how hard the medium is pumped is the gain coefficient the
    /// [`GainModel`](super::GainModel) holds. A uniform inversion needs no grid, so the model reads
    /// it as [`Inversion::Uniform`](super::Inversion) and integrates over the exact chord.
    Const,
    /// An inversion shaped by profiles given in closed form. See [`AnalyticPump`].
    Analytic(AnalyticPump),
}
impl Display for PumpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Const => write!(f, "Const"),
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        error::OpossumError,
        gain::inversion_field::CellIndex,
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        millimeter,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use std::sync::{Arc, Mutex};
    use strum::IntoEnumIterator;
    use uom::si::f64::Length;

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
    /// Return the normalized inversion β of the given cell, or an error if it is not on the grid.
    fn beta_at(field: &InversionField, cell: CellIndex) -> OpmResult<f64> {
        field
            .population(cell)
            .ok_or_else(|| OpossumError::Other(format!("cell {cell:?} is not part of the grid")))
    }
    #[test]
    fn a_flat_pump_fills_the_medium_with_unit_beta() -> OpmResult<()> {
        // Flat both ways is the shapeless pump: β = 1 in every cell that holds medium, 0 outside.
        let mut field = field_over_a_disk()?;
        AnalyticPump::new(
            TransversalProfile::Flat,
            LongitudinalProfile::Flat,
            (8, 8, 4),
        )?
        .deposit_shape(&mut field)?;
        for cell in cells(field.dimensions()) {
            let expected = if field.is_inside(cell) { 1.0 } else { 0.0 };
            assert_relative_eq!(beta_at(&field, cell)?, expected, max_relative = 1e-12);
        }
        // ... and the mask really did leave something out, otherwise the check above is vacuous.
        assert!(cells(field.dimensions()).any(|cell| !field.is_inside(cell)));
        Ok(())
    }
    #[test]
    fn a_grid_without_cells_is_refused() {
        for refused in [(0, 4, 4), (4, 0, 4), (4, 4, 0)] {
            assert!(
                AnalyticPump::new(TransversalProfile::Flat, LongitudinalProfile::Flat, refused)
                    .is_err(),
                "a grid of {refused:?} should be refused"
            );
        }
        assert!(
            AnalyticPump::new(
                TransversalProfile::Flat,
                LongitudinalProfile::Flat,
                (1, 1, 1)
            )
            .is_ok()
        );
    }
    #[test]
    fn a_rejected_grid_keeps_the_old_one() -> OpmResult<()> {
        // A half-typed value in the GUI must not damage what is already configured, and a grid is
        // kept as a whole: the z axis below is fine, but the y one is not.
        let mut pump = AnalyticPump::new(
            TransversalProfile::Flat,
            LongitudinalProfile::Flat,
            (4, 5, 6),
        )?;
        assert!(pump.set_grid((7, 0, 9)).is_err());
        assert_eq!(pump.grid(), (4, 5, 6));
        Ok(())
    }
    #[test]
    fn default_is_unpumped() {
        assert_eq!(PumpSource::default(), PumpSource::None);
        assert!(!PumpSource::default().is_active());
        // A uniform pump and a shaped one both do something; only `None` is passive.
        assert!(PumpSource::Const.is_active());
        assert!(PumpSource::Analytic(AnalyticPump::default()).is_active());
    }
    #[test]
    fn fmt() {
        assert_eq!(format!("{}", PumpSource::None), "None");
        assert_eq!(format!("{}", PumpSource::Const), "Const");
        assert_eq!(
            format!("{}", PumpSource::Analytic(AnalyticPump::default())),
            "Analytic"
        );
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
            PumpSource::Const,
            PumpSource::Analytic(AnalyticPump::new(
                TransversalProfile::SuperGaussian(SuperGaussianShape::default()),
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    reciprocal_centimeter!(1.0),
                    PumpDirection::Backward,
                )?),
                (16, 16, 8),
            )?),
        ] {
            let serialized =
                ron::to_string(&source).map_err(|e| OpossumError::Other(e.to_string()))?;
            let deserialized: PumpSource =
                ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(source, deserialized);
        }
        // A hand-edited file has to run through the same validation as the constructor. The accepted
        // case is asserted alongside the rejected one so that the rejection is known to come from the
        // value rather than from a shape `ron` could not read in the first place - a zero-cell grid
        // is refused, the same shape with a valid grid is not.
        assert!(
            ron::from_str::<PumpSource>(
                "Analytic((transversal:Flat,longitudinal:Flat,cells_x:8,cells_y:8,cells_z:8))"
            )
            .is_ok()
        );
        assert!(
            ron::from_str::<PumpSource>(
                "Analytic((transversal:Flat,longitudinal:Flat,cells_x:0,cells_y:8,cells_z:8))"
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn a_super_gaussian_spot_peaks_on_the_axis() -> OpmResult<()> {
        let sigma = millimeter!(2.0, 2.0);
        let mut field = field_centered_on_the_axis()?;
        AnalyticPump::new(
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                millimeter!(0.0, 0.0),
                sigma,
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,
            (9, 9, 4),
        )?
        .deposit_shape(&mut field)?;
        // The middle column of nine sits on the axis, where the profile is at its peak (β = 1) ...
        assert_relative_eq!(beta_at(&field, (4, 4, 0))?, 1.0, max_relative = 1e-12);
        // ... and every cell off it follows exp(-r^2 / 2 sigma^2), worked out at the cell's own
        // center rather than at a position the grid was chosen to make round.
        for cell in [(4, 6, 0), (6, 4, 0), (2, 2, 3)] {
            let center = center_of(&field, cell)?;
            let radial = center.x.value.hypot(center.y.value);
            let expected = f64::exp(-0.5 * (radial / sigma.x.value).powi(2));
            assert_relative_eq!(beta_at(&field, cell)?, expected, max_relative = 1e-12);
        }
        Ok(())
    }
    #[test]
    fn a_decentred_spot_moves_the_maximum_with_it() -> OpmResult<()> {
        // The peak follows the center of the spot, so a spot put over one corner of the cross
        // section leaves the axis behind.
        let mut field = field_centered_on_the_axis()?;
        let off_axis = center_of(&field, (6, 6, 0))?;
        AnalyticPump::new(
            TransversalProfile::SuperGaussian(SuperGaussianShape::new(
                Point2::new(off_axis.x, off_axis.y),
                millimeter!(1.0, 1.0),
                1.0,
                degree!(0.0),
                false,
            )?),
            LongitudinalProfile::Flat,
            (9, 9, 4),
        )?
        .deposit_shape(&mut field)?;
        assert_relative_eq!(beta_at(&field, (6, 6, 0))?, 1.0, max_relative = 1e-12);
        assert!(beta_at(&field, (4, 4, 0))? < beta_at(&field, (6, 6, 0))?);
        Ok(())
    }
    #[test]
    fn beer_lambert_decays_along_the_axis() -> OpmResult<()> {
        let absorption = reciprocal_centimeter!(1.0);
        let mut field = field_over_a_disk()?;
        AnalyticPump::new(
            TransversalProfile::Flat,
            LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                absorption,
                PumpDirection::Forward,
            )?),
            (8, 8, 4),
        )?
        .deposit_shape(&mut field)?;
        // Between the first and the last slice the pump has travelled the distance between their
        // centers, and β is attenuated by exactly the Lambert-Beer factor over it.
        let (near, far) = ((3, 3, 0), (3, 3, 3));
        let travelled = center_of(&field, far)?.z - center_of(&field, near)?.z;
        assert_relative_eq!(
            beta_at(&field, far)? / beta_at(&field, near)?,
            f64::exp(-(absorption * travelled).value),
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn pumping_from_the_other_end_mirrors_the_profile() -> OpmResult<()> {
        let make = |direction| -> OpmResult<InversionField> {
            let mut field = field_over_a_disk()?;
            AnalyticPump::new(
                TransversalProfile::Flat,
                LongitudinalProfile::BeerLambert(BeerLambertProfile::new(
                    reciprocal_centimeter!(1.0),
                    direction,
                )?),
                (8, 8, 4),
            )?
            .deposit_shape(&mut field)?;
            Ok(field)
        };
        let forward = make(PumpDirection::Forward)?;
        let backward = make(PumpDirection::Backward)?;
        let (_, _, slices) = forward.dimensions();
        for (i, j, k) in cells(forward.dimensions()) {
            assert_relative_eq!(
                beta_at(&forward, (i, j, k))?,
                beta_at(&backward, (i, j, slices - 1 - k))?,
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
        AnalyticPump::new(
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
            (9, 9, 4),
        )?
        .deposit_shape(&mut field)?;
        let cell = (6, 5, 2);
        let center = center_of(&field, cell)?;
        let radial = center.x.value.hypot(center.y.value);
        // The medium starts at z = 0, so the depth the pump has travelled is the cell's own z.
        let expected = f64::exp(-0.5 * (radial / sigma.x.value).powi(2))
            * f64::exp(-(absorption * center.z).value);
        assert_relative_eq!(beta_at(&field, cell)?, expected, max_relative = 1e-12);
        Ok(())
    }
    #[test]
    fn a_pump_that_grows_as_it_is_absorbed_is_refused() {
        // An absorption coefficient may not be negative: a pump getting stronger the deeper it goes
        // is not physics. (A gain coefficient may be negative — that is an absorbing medium — but it
        // lives on the gain model now, not here.)
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
}
