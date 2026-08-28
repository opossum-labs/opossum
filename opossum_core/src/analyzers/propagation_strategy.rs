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
    core_optics::optic_surface::OpticSurface, error::OpmResult, gain::PumpConfig, light::Rays,
    refractive_index::RefractiveIndexType, utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

/// Strategy to use if a [`Ray`](crate::light::ray::Ray) misses a surface
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize, EnumIter)]
pub enum MissedSurfaceStrategy {
    /// The [`Ray`](crate::light::ray::Ray) it is set as invalid and does no longer propagate.
    #[default]
    Stop,
    /// The [`Ray`](crate::light::ray::Ray) is not altered in any way, thus skipping the surface and propagating
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

    fn ambient_refractive_index(&self) -> RefractiveIndexType;
    /// The whole [`PumpConfig`] the node with the given [`Uuid`] runs under in this analysis.
    ///
    /// Whether a component amplifies is a property of the operating point being analyzed, not of the
    /// component - so it is the analysis that has to be asked, not the node. This is the one way the
    /// [`PumpScenario`](crate::gain::PumpScenario) of a run reaches the medium it applies to: the
    /// strategy is the only analyzer-specific object handed all the way down into the propagation.
    ///
    /// The operating point arrives as **one object** rather than as two independent lookups: a gain
    /// model that reads the state of the medium needs both halves at once, and asking for them
    /// separately would make it possible to see the extraction model of one scenario next to the
    /// pumping of another.
    ///
    /// The default is [`PumpConfig::default`], which neither pumps nor amplifies - an analysis that
    /// knows no operating point leaves every component passive.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node asking on its own behalf.
    ///
    /// # Returns
    ///
    /// What that node does in this analysis.
    fn pump_config(&self, _node_id: Uuid) -> PumpConfig {
        PumpConfig::default()
    }

    /// Returns `true` if this config is driving the geometry-positioning run.
    ///
    /// During the positioning run [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume)
    /// has not been called yet, so no medium is available and gain models must skip amplification.
    /// The default is `false`; analyzers override this for the config they pass to
    /// [`AnalysisRayTrace::calc_node_positions`](crate::analyzers::raytrace::AnalysisRayTrace::calc_node_positions).
    fn is_positioning_run(&self) -> bool {
        false
    }

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
