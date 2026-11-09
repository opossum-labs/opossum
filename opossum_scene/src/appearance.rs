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
};
use uom::si::f64::Length;

/// How a drawable component looks: one variant per kind of look, each with only what it is drawn by.
///
/// These are hints to the renderer; they have no physical meaning and do not affect any simulation
/// result. A `color` is linear RGB or RGBA, each component in \[0, 1\], its alpha how see-through
/// the surface is. A `roughness` runs from 0, smooth like a mirror, to 1, matte; a `metallic`
/// share of 1 reflects like a mirror coating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Appearance {
    /// A shiny surface that specularly reflects light.
    Mirror {
        color: [f32; 4],
        roughness: f32,
        metallic: f32,
    },
    /// A partially transparent surface light passes through, such as a filter or a paraxial lens.
    Translucent { color: [f32; 4] },
    /// A matte surface that absorbs or terminates light, such as a detector.
    Opaque {
        color: [f32; 4],
        metallic: f32,
        roughness: f32,
    },
    /// A surface showing interference colors, as a diffraction grating does.
    Iridescent { color: [f32; 3], roughness: f32 },
    /// Clear glass, the body of a lens or a wedge, drawn with the refractive index of the material
    /// its solid is filled with; `color` is its tint.
    Glass { color: [f32; 3], metallic: f32 },
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
        Self::Mirror {
            color: [0.0, 0.5, 0.5, 0.6],
            roughness: 0.05,
            metallic: 1.0,
        }
    }
    /// A surface light passes through without entering a body of material, such as an idealized
    /// filter: pale and partly transparent.
    #[must_use]
    pub const fn transmissive() -> Self {
        Self::Translucent {
            color: [0.85, 0.9, 0.85, 0.4],
        }
    }
    /// A detector: a matte, dark grey surface.
    #[must_use]
    pub const fn detector() -> Self {
        Self::Opaque {
            color: [0.25, 0.25, 0.28, 1.0],
            metallic: 0.0,
            roughness: 0.7,
        }
    }
    /// Neutral glass, for the body of a lens or a wedge.
    #[must_use]
    pub const fn glass() -> Self {
        Self::Glass {
            color: [0.3, 0.3, 0.3],
            metallic: 0.0,
        }
    }
    /// A reflective grating: a shiny surface showing interference colors.
    #[must_use]
    pub const fn grating() -> Self {
        Self::Iridescent {
            color: [0.85, 0.85, 0.9],
            roughness: 0.15,
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
    /// still visible, to the appearance's own. Any other look stays as it is: a grating diffracts
    /// regardless of its coating, and a detector or a filter acts by its own model.
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
            let ray = Ray::origin_along_z(wavelength, joule!(1.0))?;
            let reflectivity = <Box<dyn Coating>>::try_from(coating)?.calc_reflectivity(
                &ray,
                -Vector3::z(),
                n2,
            )?;
            Ok(reflectivity.value as f32)
        };
        let from_to = |from: f32, to: f32, share: f32| (to - from).mul_add(share, from);
        Ok(match *self {
            Self::Glass { color, metallic } => Self::Glass {
                color,
                metallic: from_to(metallic, 1.0, reflectivity()?),
            },
            Self::Mirror {
                color: [red, green, blue, alpha],
                roughness,
                metallic,
            } => {
                let share = reflectivity()?;
                Self::Mirror {
                    color: [red, green, blue, from_to(UNCOATED_ALPHA, alpha, share)],
                    roughness,
                    metallic: from_to(0.0, metallic, share),
                }
            }
            other @ (Self::Translucent { .. } | Self::Opaque { .. } | Self::Iridescent { .. }) => {
                other
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
        let (half_color, half_metallic) = mirror_parts(Appearance::mirror().coated(
            &CoatingType::Separator(SplittingConfigBuilder::FixedRatio(0.5)),
            green(),
            1.0,
        )?);
        assert!((half_metallic - 0.5).abs() < 1e-6);
        assert!((half_color[3] - 0.45).abs() < 1e-6);
        let (none_color, none_metallic) =
            mirror_parts(Appearance::mirror().coated(&CoatingType::IdealAR, green(), 1.0)?);
        assert!(none_metallic.abs() < 1e-6);
        assert!((none_color[3] - UNCOATED_ALPHA).abs() < 1e-6);
        assert_eq!(none_color[..3], mirror_parts(Appearance::mirror()).0[..3]);
        Ok(())
    }
    /// The color and the metallic share of a mirror's look.
    fn mirror_parts(appearance: Appearance) -> ([f32; 4], f32) {
        let Appearance::Mirror {
            color, metallic, ..
        } = appearance
        else {
            panic!("a coated mirror is still a mirror");
        };
        (color, metallic)
    }
    /// An uncoated glass face reflects by the index behind it: about 4 % for n = 1.5.
    #[test]
    fn a_glass_face_is_as_mirrored_as_it_reflects() -> OpmResult<()> {
        let Appearance::Glass { metallic, .. } =
            Appearance::glass().coated(&CoatingType::Fresnel, green(), 1.5)?
        else {
            panic!("coated glass is still glass");
        };
        assert!((metallic - 0.04).abs() < 1e-6);
        Ok(())
    }
    /// A grating, a detector and a filter do not act through their coating, so it does not change
    /// their look.
    #[test]
    fn other_looks_stay_as_they_are_under_a_coating() -> OpmResult<()> {
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
