#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how a [`NodeGroup`](`crate::nodes::NodeGroup`) is analyzed.
//! For example, the energy flow for a scenery can be calculated as a simple analysis. On the other hand, a full
//! Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
pub mod analyzable;
pub mod energy;
pub mod ghostfocus;
pub mod propagation_strategy;
pub mod raytrace;

use crate::{
    error::OpmResult, gain::PumpScenario, nodes::NodeGroup,
    reporting::analysis_report::AnalysisReport,
};
pub use analyzable::Analyzable;
pub use energy::EnergyConfig;
pub use ghostfocus::GhostFocusConfig;
pub use raytrace::RayTraceConfig;

use serde::{Deserialize, Serialize};
use std::fmt::Display;
use utoipa::ToSchema;

/// Lightweight discriminant representing the kind of analyzer without configuration payload.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnalyzerKind {
    /// Simple energy flow analysis.
    Energy,
    /// Sequential ray tracing analysis.
    RayTrace,
    /// Ghost focus analysis with back reflections.
    GhostFocus,
}

/// Type of analysis to be performed.
///
/// While the individual analyzers are implemented as traits, this enum is necessary for serialization / desrialization.
#[derive(PartialEq, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub enum AnalyzerType {
    /// Simple energy flow analysis of an optical spectrum.
    ///
    /// **Note**: This mode does not consider any geometric aspects of an optical setup (so far). In particular,
    /// possible apertures of optical elements are ignored.
    #[schema(value_type=())]
    Energy(EnergyConfig),
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    #[schema(value_type=())]
    RayTrace(RayTraceConfig),
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces with a
    /// given number of bounces.
    #[schema(value_type=())]
    GhostFocus(GhostFocusConfig),
}

/// Struct to hold all info about an analyzer type
pub struct AnalyzerRegistration {
    /// Function to create a default instance of the analyzer type configuration.
    pub factory: fn() -> AnalyzerType,
    /// Function to create an analyzer instance from the configuration.
    pub builder: fn(&AnalyzerType) -> Option<Box<dyn Analyzer>>,
}

impl AnalyzerRegistration {
    /// Create a new analyzer registration
    #[must_use]
    pub const fn new(
        factory: fn() -> AnalyzerType,
        builder: fn(&AnalyzerType) -> Option<Box<dyn Analyzer>>,
    ) -> Self {
        Self { factory, builder }
    }
}

inventory::collect!(AnalyzerRegistration);

impl AnalyzerType {
    /// Returns the available analyzer types.
    ///
    /// This function returns a vector of all available analyzer types. This is needed for
    /// the backend / gui to determine which analyzers are available.
    #[must_use]
    pub fn analyzer_types() -> Vec<Self> {
        inventory::iter::<AnalyzerRegistration>
            .into_iter()
            .map(|reg| (reg.factory)())
            .collect()
    }
    /// Set the [`PumpScenario`] the analysis run about to be performed happens in.
    ///
    /// Every kind of analysis has to be told the operating point, because every one of them can meet
    /// an amplifying component. This is run state rather than configuration - see
    /// [`ActiveScenario`](crate::gain::ActiveScenario): it is set on a copy of the configuration for
    /// the duration of one run and never written to file.
    ///
    /// # Arguments
    ///
    /// * `pump_scenario` - the operating point, or `None` for a passive run.
    pub fn set_active_pump_scenario(&mut self, pump_scenario: Option<PumpScenario>) {
        match self {
            Self::Energy(config) => config.set_active_pump_scenario(pump_scenario),
            Self::RayTrace(config) => config.set_active_pump_scenario(pump_scenario),
            Self::GhostFocus(config) => config.set_active_pump_scenario(pump_scenario),
        }
    }
    /// Return the configuration this analyzer places the nodes of a model with.
    ///
    /// Where a component ends up depends on the light that reaches it — an off-axis source, a
    /// different alignment wavelength or another ambient medium all move it. Anything that needs to
    /// know where the components of a model are, rather than to run the analysis itself, asks here
    /// instead of assembling a configuration of its own, so that a drawing of the setup shows the
    /// same geometry the analysis worked on.
    ///
    /// # Returns
    ///
    /// The configuration for the positioning run, or `None` for an analyzer that does not place
    /// anything: an energy analysis carries no geometry at all.
    #[must_use]
    pub fn positioning_config(&self) -> Option<RayTraceConfig> {
        match self {
            Self::Energy(_) => None,
            Self::RayTrace(config) => Some(config.for_positioning()),
            Self::GhostFocus(config) => {
                // A ghost focus analysis places its nodes with a plain ray trace and takes only the
                // sources across: the bounce limits and the ambient medium of the ghost focus run
                // describe the parasitic passes, not the alignment.
                let mut positioning = RayTraceConfig::default();
                positioning.set_source_map(config.source_map().clone());
                positioning.set_positioning_run(true);
                Some(positioning)
            }
        }
    }
    /// Returns the corresponding [`AnalyzerKind`] for this analyzer configuration.
    #[must_use]
    pub const fn kind(&self) -> AnalyzerKind {
        match self {
            Self::Energy(_) => AnalyzerKind::Energy,
            Self::RayTrace(_) => AnalyzerKind::RayTrace,
            Self::GhostFocus(_) => AnalyzerKind::GhostFocus,
        }
    }
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy(_) => "Energy",
            Self::RayTrace(_) => "RayTracing",
            Self::GhostFocus(_) => "GhostFocus",
        };
        write!(f, "{msg}")
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::light::lightdata::ray_data_builder::RayDataBuilder;
    #[test]
    fn display() {
        assert_eq!(
            format!("{}", AnalyzerType::Energy(EnergyConfig::default())),
            "Energy"
        );
        assert_eq!(
            format!("{}", AnalyzerType::RayTrace(RayTraceConfig::default())),
            "RayTracing"
        );
        assert_eq!(
            format!("{}", AnalyzerType::GhostFocus(GhostFocusConfig::default())),
            "GhostFocus"
        );
    }
    /// Only an analysis that traces light through space can say where anything is.
    #[test]
    fn only_the_geometric_analyses_place_anything() {
        assert!(
            AnalyzerType::Energy(EnergyConfig::default())
                .positioning_config()
                .is_none()
        );
        assert!(
            AnalyzerType::RayTrace(RayTraceConfig::default())
                .positioning_config()
                .is_some()
        );
        assert!(
            AnalyzerType::GhostFocus(GhostFocusConfig::default())
                .positioning_config()
                .is_some()
        );
    }
    /// The sources have to come across into the positioning run, which has nothing to start the
    /// optical axis from without them.
    #[test]
    fn a_positioning_config_keeps_the_sources() {
        let source = uuid::Uuid::new_v4();
        let mut ray_trace = RayTraceConfig::default();
        ray_trace.map_source(source, RayDataBuilder::default());
        let mut ghost_focus = GhostFocusConfig::default();
        ghost_focus.map_source(source, RayDataBuilder::default());
        for analyzer in [
            AnalyzerType::RayTrace(ray_trace),
            AnalyzerType::GhostFocus(ghost_focus),
        ] {
            let config = analyzer
                .positioning_config()
                .expect("both of these place their nodes");
            assert!(
                config.get_source(&source).is_some(),
                "the {analyzer} analysis lost its sources on the way"
            );
        }
    }
    #[test]
    fn debug() {
        let at = AnalyzerType::Energy(EnergyConfig::default());
        assert_eq!(
            format!("{:?}", at),
            "Energy(EnergyConfig { source_map: {}, active_pump_scenario: ActiveScenario(None) })"
        );
    }
}

/// Marker trait for all Analyzers
pub trait Analyzer {
    /// Analyze a [`NodeGroup`].
    ///
    /// # Errors
    /// This function returns an error if the concrete implementation of the [`Analyzer`] returns an error.
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()>;
    /// Generate an analysis report for this [`NodeGroup`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the concrete implementation of the [`Analyzer`] returns an error..
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport>;
}
