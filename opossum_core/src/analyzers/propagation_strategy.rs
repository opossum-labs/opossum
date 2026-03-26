//! Defines the Strategy Pattern for optical ray propagation.
//!
//! This module abstracts the analyzer-specific behavior (e.g., sequential ray tracing, ghost focus, or energy analysis)
//! away from the core physical surface interactions. By implementing the [`PropagationStrategy`] trait,
//! different analysis modes can inject custom rules—such as handling missed surfaces, evaluating fluences,
//! or applying energy thresholds—directly into the propagation pipeline without modifying the optical nodes themselves.
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::{
    core_optics::optic_surface::OpticSurface, error::OpmResult, rays::Rays,
    utils::default_from_name::DefaultFromName,
};

/// Strategy to use if a [`Ray`](crate::ray::Ray) misses a surface
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize, EnumIter)]
pub enum MissedSurfaceStrategy {
    /// The [`Ray`](crate::ray::Ray) it is set as invalid and does no longer propagate.
    #[default]
    Stop,
    /// The [`Ray`](crate::ray::Ray) is not altered in any way, thus skipping the surface and propagating
    /// further through the system.
    Ignore,
}
impl DefaultFromName for MissedSurfaceStrategy {}

impl Display for MissedSurfaceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop => write!(f, "Stop"),
            Self::Ignore => write!(f, "Ignore"),
        }
    }
}

/// Defines analyzer-specific behavior during surface propagation.
pub trait PropagationStrategy {
    /// Determines how rays missing a surface should be handled.
    fn missed_surface_strategy(&self) -> MissedSurfaceStrategy;

    /// Hook executed immediately after a ray bundle interacts with a surface.
    /// Allows the analyzer to perform specific tasks, like evaluating fluence
    /// or storing caching data (e.g., for `GhostFocus`).
    ///
    /// # Errors
    ///
    /// Returns an error if the analyzer-specific interaction tasks (such as fluence
    /// evaluation or hit-map calculations) fail
    fn on_surface_interaction(
        &self,
        _surf: &mut OpticSurface,
        _rays: &mut Rays,
        _reflected_rays: Rays,
        _backward: bool,
    ) -> OpmResult<()> {
        Ok(())
    }
    /// Hook executed after apodization to apply thresholds or energy invalidation.
    ///
    /// # Errors
    /// Returns an error if the analyzer-specific post-apodization tasks (such as energy thresholding) fail.
    fn on_after_apodization(&self, _rays: &mut Rays) -> OpmResult<()> {
        Ok(())
    }
}
