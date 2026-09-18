use crate::analyzers::AnalyzerType;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use utoipa::ToSchema;
use uuid::Uuid;

/// A structure containing the [`AnalyzerType`] together with its position on a frontend GUI.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnalyzerInfo {
    analyzer_type: AnalyzerType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: Option<(f64, f64)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pump_scenarios: Vec<Uuid>,
    /// Optional default wavelength used when configuring or auto-mapping source ports
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    default_wavelength: Option<Length>,
}

impl AnalyzerInfo {
    /// Creates a new [`AnalyzerInfo`] with a defined GUI position.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(analyzer_type: AnalyzerType, gui_position: Point2<f64>) -> Self {
        Self {
            analyzer_type,
            name: None,
            gui_position: Some((gui_position.x, gui_position.y)),
            pump_scenarios: Vec::new(),
            default_wavelength: None,
        }
    }

    /// Creates a new [`AnalyzerInfo`] without an initial GUI position.
    #[must_use]
    pub const fn new_without_position(analyzer_type: AnalyzerType) -> Self {
        Self {
            analyzer_type,
            name: None,
            gui_position: None,
            pump_scenarios: Vec::new(),
            default_wavelength: None,
        }
    }

    /// Returns the user-assigned name of this [`AnalyzerInfo`], if set.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Sets the user-assigned name of this [`AnalyzerInfo`].
    ///
    /// Empty or whitespace-only strings are normalized to `None`.
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name.filter(|n| !n.trim().is_empty());
    }

    /// Convenience helper to set the name directly from a string slice.
    pub fn set_name_str(&mut self, name: &str) {
        self.set_name(Some(name.to_string()));
    }

    /// Returns the user-assigned name if set, otherwise falls back to the analyzer type label.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.analyzer_type.to_string())
    }

    /// Returns the default wavelength of this analyzer, if configured.
    #[must_use]
    pub const fn default_wavelength(&self) -> Option<Length> {
        self.default_wavelength
    }

    /// Sets the default wavelength of this analyzer.
    pub fn set_default_wavelength(&mut self, default_wavelength: Option<Length>) {
        self.default_wavelength = default_wavelength;
    }

    /// Returns the [`PumpScenario`]s this analyzer is run in.
    ///
    /// An analyzer referring to no scenario at all is run once on the passive model.
    #[must_use]
    pub fn pump_scenarios(&self) -> &[Uuid] {
        &self.pump_scenarios
    }

    /// Sets the [`PumpScenario`]s this analyzer is run in.
    ///
    /// The analyzer produces one report per listed scenario, in the given order.
    pub fn set_pump_scenarios(&mut self, pump_scenarios: Vec<Uuid>) {
        self.pump_scenarios = pump_scenarios;
    }

    /// Stops running this analyzer in the [`PumpScenario`] with the given [`Uuid`].
    ///
    /// Called when that scenario is deleted to prevent pointing at missing operating points.
    pub(crate) fn remove_pump_scenario(&mut self, id: Uuid) {
        self.pump_scenarios.retain(|scenario_id| *scenario_id != id);
    }

    /// Returns the gui position of this [`AnalyzerInfo`].
    #[must_use]
    pub fn gui_position(&self) -> Option<Point2<f64>> {
        self.gui_position.map(|(x, y)| Point2::new(x, y))
    }

    /// Sets the gui position of this [`AnalyzerInfo`].
    pub fn set_gui_position(&mut self, gui_position: Option<Point2<f64>>) {
        self.gui_position = gui_position.map(|gp| (gp.x, gp.y));
    }

    /// Sets the raw coordinates tuple for GUI positioning.
    pub const fn set_gui_position_tuple(&mut self, gui_position: Option<(f64, f64)>) {
        self.gui_position = gui_position;
    }

    /// Returns a reference to the analyzer type of this [`AnalyzerInfo`].
    #[must_use]
    pub const fn analyzer_type(&self) -> &AnalyzerType {
        &self.analyzer_type
    }

    /// Sets the analyzer type of this [`AnalyzerInfo`].
    pub fn set_analyzer_type(&mut self, analyzer_type: &AnalyzerType) {
        self.analyzer_type = analyzer_type.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::{GhostFocusConfig, energy::EnergyConfig};
    use nalgebra::Point2;

    #[test]
    fn analyzer_info_set_analyzer_type() {
        let mut at = AnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            Point2::new(1.0, 2.0),
        );
        at.set_analyzer_type(&AnalyzerType::GhostFocus(GhostFocusConfig::default()));
        assert!(matches!(at.analyzer_type, AnalyzerType::GhostFocus(_)));
    }

    #[test]
    fn analyzer_info_set_gui_position() {
        let mut at = AnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            Point2::new(1.0, 2.0),
        );
        let new_position = Point2::new(3.0, 4.0);
        at.set_gui_position(Some(new_position));
        assert_eq!(at.gui_position(), Some(new_position));

        at.set_gui_position_tuple(Some((5.0, 6.0)));
        assert_eq!(at.gui_position(), Some(Point2::new(5.0, 6.0)));
    }

    #[test]
    fn analyzer_info_name_handling() {
        let mut info = AnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            Point2::new(0.0, 0.0),
        );
        assert_eq!(info.name(), None);
        assert_eq!(info.display_name(), "Energy");

        // Set using Option<String>
        info.set_name(Some("Main Measurement".to_string()));
        assert_eq!(info.name(), Some("Main Measurement"));
        assert_eq!(info.display_name(), "Main Measurement");

        // Set using convenience method
        info.set_name_str("Second Run");
        assert_eq!(info.name(), Some("Second Run"));

        // Whitespace-only should normalize to None
        info.set_name_str("   ");
        assert_eq!(info.name(), None);
        assert_eq!(info.display_name(), "Energy");

        // Explicit None clears
        info.set_name(None);
        assert_eq!(info.name(), None);
    }

    #[test]
    fn analyzer_info_pump_scenarios_management() {
        let mut info =
            AnalyzerInfo::new_without_position(AnalyzerType::Energy(EnergyConfig::default()));
        assert!(info.gui_position().is_none());
        assert!(info.pump_scenarios().is_empty());

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        info.set_pump_scenarios(vec![id1, id2]);
        assert_eq!(info.pump_scenarios(), &[id1, id2]);

        info.remove_pump_scenario(id1);
        assert_eq!(info.pump_scenarios(), &[id2]);
    }
}
