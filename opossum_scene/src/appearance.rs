//! How each kind of component looks in the 3D view.
//!
//! An [`Appearance`] states how a face looks, independently of the glTF material it ends up as (see
//! [`material_of`](crate::material::material_of)). It never affects a simulation.

use nalgebra::Vector3;
use opossum_core::{
    coatings::{Coating, CoatingType},
    error::{OpmResult, OpossumError},
    joule,
    light::Ray,
    millimeter,
};
use uom::si::f64::Length;

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
    /// Linear RGBA base color, with each component in \[0, 1\]. For glass, its tint.
    pub color: [f32; 4],
    /// Surface roughness in \[0, 1\]; 0 is perfectly smooth (mirror-like), 1 is fully rough
    /// (diffuse/matte).
    pub roughness: f32,
    /// How metallic the surface looks, in \[0, 1\]; 1 reflects like a mirror coating.
    pub metallic: f32,
    /// The rendering finish that determines the overall look of the surface.
    pub finish: SurfaceFinish,
}

/// How opaque a thin reflecting surface is drawn where its coating reflects nothing: faint, but
/// still there to be seen and clicked.
const UNCOATED_ALPHA: f32 = 0.3;

/// How every node type is drawn, by the name it is registered under, or `None` for a type that is
/// not drawn: one without a shape of its own, such as a group or a reference, or one that is only
/// traced at, such as a source.
const LOOKS: &[(&str, Option<Appearance>)] = &[
    ("lens", Some(Appearance::glass())),
    ("cylindric lens", Some(Appearance::glass())),
    ("wedge", Some(Appearance::glass())),
    ("bored mirror", Some(Appearance::glass())),
    ("mirror", Some(Appearance::mirror())),
    ("parabolic mirror", Some(Appearance::mirror())),
    ("beam splitter", Some(Appearance::mirror())),
    ("ideal filter", Some(Appearance::transmissive())),
    ("paraxial surface", Some(Appearance::transmissive())),
    ("energy meter", Some(Appearance::detector())),
    ("fluence detector", Some(Appearance::detector())),
    ("spectrometer", Some(Appearance::detector())),
    ("spot diagram", Some(Appearance::detector())),
    ("wavefront monitor", Some(Appearance::detector())),
    ("reflective grating", Some(Appearance::grating())),
    ("dummy", None),
    ("group", None),
    ("reference", None),
    ("source port", None),
    ("ray propagation", None),
];

impl Appearance {
    /// A mirror: a shiny, teal, partly see-through surface, as it looks fully mirrored. How much of
    /// that a face shows, its coating says (see [`Appearance::coated`]).
    #[must_use]
    pub const fn mirror() -> Self {
        Self {
            color: [0.0, 0.5, 0.5, 0.6],
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
    /// A reflective grating: a shiny surface showing interference colors.
    #[must_use]
    pub const fn grating() -> Self {
        Self {
            color: [0.85, 0.85, 0.9, 1.0],
            roughness: 0.15,
            metallic: 1.0,
            finish: SurfaceFinish::Iridescent,
        }
    }
    /// Return how nodes of a type are drawn.
    ///
    /// # Arguments
    ///
    /// - `node_type`: the name the node type is registered under
    ///
    /// # Returns
    ///
    /// The appearance of the type, or `None` for a type that is not drawn.
    ///
    /// # Errors
    ///
    /// This function returns an error if no look is defined for the type.
    pub fn of_node_type(node_type: &str) -> OpmResult<Option<Self>> {
        LOOKS
            .iter()
            .find(|(name, _)| *name == node_type)
            .map(|(_, look)| *look)
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "no look is defined for the node type '{node_type}'"
                ))
            })
    }
    /// Return how one face of a component looks with the given coating on it.
    ///
    /// The reflectivity of the coating at normal incidence is the share of the face drawn as a
    /// mirror. On glass, the coating lays a mirror over the glass: the metallic share rises towards
    /// 1 with it. A reflecting surface shows its appearance as far as its coating reflects: its
    /// metallic share and its opacity rise from a bare surface, not metallic and drawn faint but
    /// still visible, to the appearance's own. Any other finish keeps its look: a grating diffracts regardless of its
    /// coating, and a detector or a filter acts by its own model.
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
        let from_to = |from: f32, to: f32, share: f32| (to - from).mul_add(share, from);
        Ok(match self.finish {
            SurfaceFinish::Glass => {
                let share = reflectivity()?;
                Self {
                    metallic: from_to(self.metallic, 1.0, share),
                    ..*self
                }
            }
            SurfaceFinish::Reflective => {
                let share = reflectivity()?;
                let [red, green, blue, alpha] = self.color;
                Self {
                    color: [red, green, blue, from_to(UNCOATED_ALPHA, alpha, share)],
                    metallic: from_to(0.0, self.metallic, share),
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
    use opossum_core::{
        coatings::{CoatingConstantR, SplittingConfigBuilder},
        nanometer,
        nodes::{create_node_ref, node_types},
        percent,
    };
    use std::collections::HashSet;

    /// Every node type that can be built has a look, so a new one cannot go undrawn unnoticed.
    #[test]
    fn every_registered_node_type_has_a_look() {
        let missing: Vec<&str> = node_types()
            .into_iter()
            .map(|(node_type, _)| node_type)
            .filter(|node_type| Appearance::of_node_type(node_type).is_err())
            .collect();
        assert!(missing.is_empty(), "no look is defined for {missing:?}");
    }

    /// Every look belongs to a node type that exists, once: a renamed or removed type leaves no
    /// entry behind, and no entry is shadowed by an earlier one of the same name.
    #[test]
    fn every_look_names_a_registered_node_type() {
        let mut seen = HashSet::new();
        for (node_type, _) in LOOKS {
            assert!(
                create_node_ref(node_type).is_ok(),
                "a look is defined for '{node_type}', which is no node type"
            );
            assert!(seen.insert(node_type), "'{node_type}' has two looks");
        }
    }

    /// A name that is no node type has no look.
    #[test]
    fn a_name_that_is_no_node_type_has_no_look() {
        assert!(Appearance::of_node_type("no such type").is_err());
    }

    /// The wavelength the coatings below are evaluated at.
    fn green() -> Length {
        nanometer!(532.0)
    }
    /// A fully reflecting coating shows a mirror as it is, a half reflecting one as a half
    /// mirrored pane, and one that reflects nothing as a faint, bare surface.
    #[test]
    fn a_reflecting_surface_is_as_mirrored_as_its_coating() -> OpmResult<()> {
        let full = Appearance::mirror().coated(
            &CoatingConstantR::new(percent!(100.0))?.into(),
            green(),
            1.0,
        )?;
        assert_eq!(full, Appearance::mirror());
        let half = Appearance::mirror().coated(
            &CoatingType::Separator(SplittingConfigBuilder::FixedRatio(0.5)),
            green(),
            1.0,
        )?;
        assert!((half.metallic - 0.5).abs() < 1e-6);
        assert!((half.color[3] - 0.45).abs() < 1e-6);
        let none = Appearance::mirror().coated(&CoatingType::IdealAR, green(), 1.0)?;
        assert!(none.metallic.abs() < 1e-6);
        assert!((none.color[3] - UNCOATED_ALPHA).abs() < 1e-6);
        assert_eq!(none.color[..3], Appearance::mirror().color[..3]);
        Ok(())
    }
    /// An uncoated glass face reflects by the index behind it: about 4 % for n = 1.5.
    #[test]
    fn a_glass_face_is_as_mirrored_as_it_reflects() -> OpmResult<()> {
        let glass = Appearance::glass().coated(&CoatingType::Fresnel, green(), 1.5)?;
        assert!((glass.metallic - 0.04).abs() < 1e-6);
        assert!((glass.color[3] - 1.0).abs() < 1e-6);
        Ok(())
    }
    /// A grating, a detector and a filter do not act through their coating, so it does not change
    /// their look.
    #[test]
    fn other_finishes_keep_their_look_under_a_coating() -> OpmResult<()> {
        let mirroring: CoatingType = CoatingConstantR::new(percent!(100.0))?.into();
        for appearance in [
            Appearance::grating(),
            Appearance::detector(),
            Appearance::transmissive(),
        ] {
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
}
