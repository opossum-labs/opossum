#![warn(missing_docs)]
//! Cosmetic appearance types for drawable optical surfaces.
//!
//! These types describe how a surface should look in a 3-D rendering, independently of any
//! renderer or rendering library. They are a property of a node's *cosmetic* intention, not its
//! optical physics — a grating is physically [`super::planar::SurfaceKind::Reflective`] but may
//! look [`SurfaceFinish::Iridescent`], and those two facts never conflict.

/// The rendering finish of a drawable surface.
///
/// This is deliberately distinct from the physical
/// [`SurfaceKind`](super::planar::SurfaceKind): a grating is physically reflective (it redirects
/// light), but cosmetically iridescent (it shows diffraction colours). The two concepts can
/// diverge freely because one governs analysis and the other governs only how the node appears in
/// the 3-D view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFinish {
    /// A shiny, mirror-like surface that specularly reflects light.
    Reflective,
    /// A partially transparent surface that light passes through (e.g. a filter or paraxial lens).
    Transmissive,
    /// A matte surface that absorbs or terminates light (e.g. a detector or beam dump).
    Opaque,
    /// An iridescent surface exhibiting interference colours, typical of a diffraction grating.
    Iridescent,
}

/// How a drawable surface should look, independent of any particular renderer.
///
/// Carries three cosmetic properties: a linear RGBA base colour, a roughness value, and a
/// [`SurfaceFinish`]. These are hints to the renderer; they have no physical meaning and do not
/// affect any simulation result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Linear RGBA base colour, with each component in \[0, 1\].
    pub color: [f32; 4],
    /// Surface roughness in \[0, 1\]; 0 is perfectly smooth (mirror-like), 1 is fully rough
    /// (diffuse/matte).
    pub roughness: f32,
    /// The rendering finish that determines the overall look of the surface.
    pub finish: SurfaceFinish,
}
