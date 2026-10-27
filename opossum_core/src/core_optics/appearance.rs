#![warn(missing_docs)]
//! Cosmetic appearance types for drawable optical components.
//!
//! These types describe how a component should look in a 3-D rendering, independently of any
//! renderer or rendering library. Every node states its own (see
//! [`OpticNode::appearance`](super::OpticNode::appearance)); it never affects a simulation.

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
/// Carries three cosmetic properties: a linear RGBA base color, a roughness value, and a
/// [`SurfaceFinish`]. These are hints to the renderer; they have no physical meaning and do not
/// affect any simulation result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Linear RGBA base color, with each component in \[0, 1\]. For glass, its tint.
    pub color: [f32; 4],
    /// Surface roughness in \[0, 1\]; 0 is perfectly smooth (mirror-like), 1 is fully rough
    /// (diffuse/matte).
    pub roughness: f32,
    /// The rendering finish that determines the overall look of the surface.
    pub finish: SurfaceFinish,
}

impl Appearance {
    /// A mirror: a shiny, light grey surface.
    #[must_use]
    pub const fn mirror() -> Self {
        Self {
            color: [0.9, 0.9, 0.92, 1.0],
            roughness: 0.05,
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
            finish: SurfaceFinish::Transmissive,
        }
    }
    /// A detector: a matte, dark grey surface.
    #[must_use]
    pub const fn detector() -> Self {
        Self {
            color: [0.25, 0.25, 0.28, 1.0],
            roughness: 0.7,
            finish: SurfaceFinish::Opaque,
        }
    }
    /// Neutral glass, for the body of a lens or a wedge.
    #[must_use]
    pub const fn glass() -> Self {
        Self {
            color: [0.3, 0.3, 0.3, 1.0],
            roughness: 0.0,
            finish: SurfaceFinish::Glass,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::OpticNode,
        nodes::{EnergyMeter, IdealFilter, Lens, ThinMirror},
    };

    /// A mirror is drawn reflective.
    #[test]
    fn a_mirror_looks_reflective() {
        let appearance = ThinMirror::default().appearance();
        assert_eq!(appearance, Some(Appearance::mirror()));
        assert_eq!(
            appearance.map(|appearance| appearance.color),
            Some([0.9, 0.9, 0.92, 1.0_f32])
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
