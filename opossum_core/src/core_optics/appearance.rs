#![warn(missing_docs)]
//! Cosmetic appearance types for drawable optical components.
//!
//! These types describe how a component should look in a 3-D rendering, independently of any
//! renderer or rendering library. Every node states its own (see
//! [`OpticNode::appearance`](super::OpticNode::appearance)); it never affects a simulation.

use nalgebra::Vector3;
use uom::si::f64::Length;

use crate::{
    coatings::{Coating, CoatingType},
    error::OpmResult,
    joule,
    light::Ray,
    millimeter,
};

/// The rendering finish of a drawable component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFinish {
    /// A shiny, mirror-like surface that specularly reflects light.
    Reflective,
    /// A partially transparent surface that light passes through (e.g. a filter or paraxial lens).
    Transmissive,
    /// A matte surface that absorbs or terminates light (e.g. a detector or beam dump).
    Opaque,
    /// An iridescent surface exhibiting interference colors, typical of a diffraction grating.
    Iridescent,
    /// Clear glass: the body of a lens or a wedge, drawn with the refractive index of the material
    /// its solid is filled with.
    Glass,
}

/// How a drawable component should look, independent of any particular renderer.
///
/// Carries four cosmetic properties: a linear RGBA base color, a roughness value, a metallic share
/// and a [`SurfaceFinish`]. These are hints to the renderer; they have no physical meaning and do
/// not affect any simulation result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Linear RGBA base color, with each component in \[0, 1\]. For glass, its tint; for a thin
    /// reflecting surface, its alpha is how see-through the bare surface is.
    pub color: [f32; 4],
    /// Surface roughness in \[0, 1\]; 0 is perfectly smooth (mirror-like), 1 is fully rough
    /// (diffuse/matte).
    pub roughness: f32,
    /// How metallic the surface looks, in \[0, 1\]; 1 reflects like a mirror coating.
    pub metallic: f32,
    /// The rendering finish that determines the overall look of the surface.
    pub finish: SurfaceFinish,
}

impl Appearance {
    /// A thin reflecting surface, such as a mirror: a pale, see-through pane that its coating
    /// mirrors (see [`Appearance::coated`]).
    #[must_use]
    pub const fn mirror() -> Self {
        Self {
            color: [0.0, 0.9, 0.0, 0.3],
            roughness: 0.05,
            metallic: 1.0,
            finish: SurfaceFinish::Reflective,
        }
    }
    /// A surface light passes through without entering a body of material, such as an idealized
    /// filter: pale and partly transparent.
    #[must_use]
    pub const fn transmissive() -> Self {
        Self {
            color: [0.85, 0.9, 0.85, 0.4],
            roughness: 0.5,
            metallic: 0.0,
            finish: SurfaceFinish::Transmissive,
        }
    }
    /// A detector: a matte, dark grey surface.
    #[must_use]
    pub const fn detector() -> Self {
        Self {
            color: [0.25, 0.25, 0.28, 1.0],
            roughness: 0.7,
            metallic: 0.0,
            finish: SurfaceFinish::Opaque,
        }
    }
    /// Neutral glass, for the body of a lens or a wedge.
    #[must_use]
    pub const fn glass() -> Self {
        Self {
            color: [0.3, 0.3, 0.3, 1.0],
            roughness: 0.0,
            metallic: 0.0,
            finish: SurfaceFinish::Glass,
        }
    }
    /// Return how one face of a component looks with the given coating on it.
    ///
    /// The reflectivity of the coating at normal incidence is the share of the face drawn as a
    /// mirror: it raises the metallic share of glass and of a reflecting surface, and the opacity of
    /// the latter. Any other finish keeps its look: a grating diffracts regardless of its coating,
    /// and a detector or a filter acts by its own model.
    ///
    /// # Arguments
    ///
    /// - `coating`: the coating of the face
    /// - `wavelength`: the wavelength the coating is evaluated at
    /// - `n2`: the refractive index behind the face, by which an uncoated face reflects
    ///
    /// # Returns
    ///
    /// The appearance of the coated face.
    ///
    /// # Errors
    ///
    /// This function returns an error if the coating cannot be built or states no reflectivity at
    /// the given wavelength.
    // A reflectivity in [0, 1] loses nothing visible as an f32.
    #[allow(clippy::cast_possible_truncation)]
    pub fn coated(&self, coating: &CoatingType, wavelength: Length, n2: f64) -> OpmResult<Self> {
        let reflectivity = || -> OpmResult<f32> {
            let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), wavelength, joule!(1.0))?;
            let reflectivity = <Box<dyn Coating>>::try_from(coating)?.calc_reflectivity(
                &ray,
                -Vector3::z(),
                n2,
            )?;
            Ok(reflectivity.value as f32)
        };
        let toward_one = |value: f32, share: f32| (1.0 - value).mul_add(share, value);
        Ok(match self.finish {
            SurfaceFinish::Glass => {
                let share = reflectivity()?;
                Self {
                    metallic: toward_one(self.metallic, share),
                    ..*self
                }
            }
            SurfaceFinish::Reflective => {
                let share = reflectivity()?;
                let [red, green, blue, alpha] = self.color;
                Self {
                    color: [red, green, blue, toward_one(alpha, share)],
                    metallic: toward_one(self.metallic, share),
                    ..*self
                }
            }
            SurfaceFinish::Opaque | SurfaceFinish::Transmissive | SurfaceFinish::Iridescent => {
                *self
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        coatings::{CoatingConstantR, SplittingConfigBuilder},
        core_optics::OpticNode,
        nanometer,
        nodes::{EnergyMeter, IdealFilter, Lens, ReflectiveGrating, ThinMirror},
        percent,
    };
    use approx::assert_abs_diff_eq;

    /// A mirror is drawn reflective: a pale pane, until its coating mirrors it.
    #[test]
    fn a_mirror_looks_reflective() {
        let appearance = ThinMirror::default().appearance();
        assert_eq!(appearance, Some(Appearance::mirror()));
        assert_eq!(
            appearance.map(|appearance| appearance.color),
            Some([0.9, 0.9, 0.92, 0.3_f32])
        );
    }
    /// The wavelength the coatings below are evaluated at.
    fn green() -> Length {
        nanometer!(532.0)
    }
    /// A fully reflecting coating turns a reflecting surface into an opaque mirror, a half
    /// reflecting one into a half mirrored pane, and one that reflects nothing leaves the pane.
    #[test]
    fn a_reflecting_surface_is_as_mirrored_as_its_coating() -> OpmResult<()> {
        let full = Appearance::mirror().coated(
            &CoatingConstantR::new(percent!(100.0))?.into(),
            green(),
            1.0,
        )?;
        assert_abs_diff_eq!(full.metallic, 1.0);
        assert_abs_diff_eq!(full.color[3], 1.0);
        let half = Appearance::mirror().coated(
            &CoatingType::Separator(SplittingConfigBuilder::FixedRatio(0.5)),
            green(),
            1.0,
        )?;
        assert_abs_diff_eq!(half.metallic, 0.5);
        assert_abs_diff_eq!(half.color[3], 0.65);
        let none = Appearance::mirror().coated(&CoatingType::IdealAR, green(), 1.0)?;
        assert_eq!(none, Appearance::mirror());
        Ok(())
    }
    /// An uncoated glass face reflects by the index behind it: about 4 % for n = 1.5.
    #[test]
    fn a_glass_face_is_as_mirrored_as_it_reflects() -> OpmResult<()> {
        let glass = Appearance::glass().coated(&CoatingType::Fresnel, green(), 1.5)?;
        assert_abs_diff_eq!(glass.metallic, 0.04, epsilon = 1e-6);
        assert_abs_diff_eq!(glass.color[3], 1.0);
        Ok(())
    }
    /// A grating, a detector and a filter do not act through their coating, so it does not change
    /// their look.
    #[test]
    fn other_finishes_keep_their_look_under_a_coating() -> OpmResult<()> {
        let mirroring: CoatingType = CoatingConstantR::new(percent!(100.0))?.into();
        let grating = ReflectiveGrating::default()
            .appearance()
            .ok_or_else(|| crate::error::OpossumError::Other("a grating is drawn".into()))?;
        for appearance in [grating, Appearance::detector(), Appearance::transmissive()] {
            assert_eq!(appearance.coated(&mirroring, green(), 1.5)?, appearance);
        }
        Ok(())
    }
    /// A coating cannot be evaluated at a wavelength that is none.
    #[test]
    fn a_coating_is_not_evaluated_without_a_wavelength() {
        assert!(
            Appearance::mirror()
                .coated(&CoatingType::Fresnel, nanometer!(0.0), 1.0)
                .is_err()
        );
    }
    /// A filter is drawn transmissive.
    #[test]
    fn a_filter_looks_transmissive() {
        let appearance = IdealFilter::default().appearance();
        assert_eq!(appearance, Some(Appearance::transmissive()));
        assert_eq!(
            appearance.map(|appearance| appearance.color),
            Some([0.85, 0.9, 0.85, 0.4_f32])
        );
    }
    /// A detector is drawn opaque.
    #[test]
    fn a_detector_looks_opaque() {
        let appearance = EnergyMeter::default().appearance();
        assert_eq!(appearance, Some(Appearance::detector()));
        assert_eq!(
            appearance.map(|appearance| appearance.color),
            Some([0.25, 0.25, 0.28, 1.0_f32])
        );
    }
    /// A lens is drawn as neutral glass.
    #[test]
    fn a_lens_looks_like_neutral_glass() {
        let appearance = Lens::default().appearance();
        assert_eq!(appearance, Some(Appearance::glass()));
        assert_eq!(
            appearance.map(|appearance| appearance.color),
            Some([0.3, 0.3, 0.3, 1.0_f32])
        );
    }
}
