#![warn(missing_docs)]
//! An ideal detector node measuring total optical energy or power.

use std::fmt::{Debug, Display};

use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use uom::si::f64::Energy;

use crate::{
    analyzers::{
        AnalyzerKind, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, node_attr::HasNodeAttr},
    error::OpmResult,
    joule,
    light::LightData,
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::{
        node_report::{NodeReport, NodeReportResult},
        report_note::{ReportLevel, ReportNote},
    },
};

#[non_exhaustive]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`EnergyMeter`]. This is currently not used.
pub enum Metertype {
    /// An ideal energy meter measuring integrated pulse energy.
    #[default]
    IdealEnergyMeter,
    /// An ideal power meter measuring continuous-wave power (currently not used).
    IdealPowerMeter,
}

impl Metertype {
    /// Returns the string slice representation of this [`Metertype`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdealEnergyMeter => "ideal energy meter",
            Self::IdealPowerMeter => "ideal power meter",
        }
    }
}

impl Display for Metertype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Self::Metertype(value)
    }
}

inventory::submit! {
    NodeRegistration::new::<EnergyMeter>("energy meter", "ideal energy meter")
}

/// An (ideal) energy / power meter.
///
/// It measures the total energy of incoming light regardless of wavelength, spatial position,
/// angle, or polarization.
///
/// ## Optical Ports
///   - Inputs
///     - `input_1`
///   - Outputs
///     - `output_1`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `meter type`
///
/// During analysis, the output port forwards an exact replica of the input beam (similar to
/// a [`Dummy`](crate::nodes::Dummy) node). This allows detector nodes to be stacked or placed
/// transparently inside an optical setup without disturbing subsequent propagation.
#[derive(OpmNode, Clone)]
#[opm_node("whitesmoke")]
pub struct EnergyMeter {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}

impl Default for EnergyMeter {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("energy meter");
        node_attr
            .create_property(
                "meter type",
                "model type of the meter",
                Metertype::default().into(),
            )
            .expect("Hardcoded property creation must not fail");
        let mut em = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        em.update_surfaces()
            .expect("Updating surfaces on default energy meter must not fail");
        em
    }
}

impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    ///
    /// # Parameters
    /// * `name`: Name of the [`EnergyMeter`].
    /// * `meter_type`: Specific [`Metertype`] of the [`EnergyMeter`].
    ///
    /// # Errors
    ///
    /// Returns an error if the property `meter type` cannot be set.
    pub fn new(name: &str, meter_type: Metertype) -> OpmResult<Self> {
        let mut energy_meter = Self::default();
        energy_meter.node_attr.set_name(name);
        energy_meter.set_meter_type(meter_type)?;
        Ok(energy_meter)
    }

    /// Returns the configured [`Metertype`] of this [`EnergyMeter`].
    #[must_use]
    pub fn meter_type(&self) -> Metertype {
        self.node_attr
            .get_property("meter type")
            .ok()
            .and_then(|prop| match prop {
                Proptype::Metertype(meter_type) => Some(*meter_type),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Sets the [`Metertype`] of this [`EnergyMeter`].
    ///
    /// # Errors
    ///
    /// Returns an error if the property `meter type` cannot be updated.
    pub fn set_meter_type(&mut self, meter_type: Metertype) -> OpmResult<()> {
        self.node_attr
            .set_property("meter type", meter_type.into())?;
        Ok(())
    }

    /// Returns the captured light data from the latest analysis pass, if any.
    #[must_use]
    pub const fn light_data(&self) -> Option<&LightData> {
        self.light_data.as_ref()
    }

    /// Returns `true` if an apodization warning occurred during light propagation.
    #[must_use]
    pub const fn has_apodization_warning(&self) -> bool {
        self.apodization_warning
    }

    /// Returns the total energy measured by this [`EnergyMeter`].
    ///
    /// Returns `None` if no light data has been captured or for incompatible data representations (e.g. Fourier).
    #[must_use]
    pub fn get_energy(&self) -> Option<Energy> {
        self.light_data.as_ref().and_then(|data| match data {
            LightData::Energy(s) => Some(joule!(s.total_energy())),
            LightData::Geometric(r) => Some(r.total_energy()),
            LightData::GhostFocus(r) => Some(
                r.iter()
                    .fold(joule!(0.0), |acc, rays| acc + rays.total_energy()),
            ),
            LightData::Fourier => None,
        })
    }
}

impl OpticNode for EnergyMeter {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }

    fn reset_data(&mut self) {
        self.set_light_data(None);
        self.apodization_warning = false;
        self.reset_optic_surfaces();
        self.node_attr_mut().clear_runtime_inversion();
    }

    fn node_report(&self, uuid: &str, _analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        let mut props = Properties::default();
        if let Some(e) = self.get_energy() {
            props.create("Energy", "Output energy", e.into())?;
        } else {
            props.create("Energy", "Output energy", "no data".into())?;
        }
        props.create("Model", "type of meter", self.meter_type().into())?;

        let mut report = NodeReport::new(self.node_type(), self.name(), uuid, props);

        if self.apodization_warning {
            report.add_note(ReportNote::new(
                ReportLevel::Warning,
                "Rays have been apodized at input aperture. Results might not be accurate.",
            ));
        }
        Ok(NodeReportResult::Report(report))
    }

    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }

    fn set_light_data(&mut self, new_data: Option<LightData>) {
        self.light_data = new_data;
    }
}

impl Debug for EnergyMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{data} (Type: {:?})", self.meter_type()),
            None => write!(f, "no data"),
        }
    }
}

impl AnalysisGhostFocus for EnergyMeter {}
impl AnalysisEnergy for EnergyMeter {}
impl AnalysisRayTrace for EnergyMeter {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{RayTraceConfig, energy::EnergyConfig},
        core_optics::{PortType, node_attr::NodePositioning},
        light::{LightResult, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        utils::geom_transformation::Isometry,
    };

    #[test]
    fn default() {
        let node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.name(), "energy meter");
        assert_eq!(node.node_type(), "energy meter");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "whitesmoke");
    }

    #[test]
    fn new() -> OpmResult<()> {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter)?;
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.name(), "test");
        Ok(())
    }

    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<EnergyMeter>()
    }

    #[test]
    fn set_meter_type() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_meter_type(Metertype::IdealPowerMeter)?;
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        Ok(())
    }

    #[test]
    fn ports() {
        let meter = EnergyMeter::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }

    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_inverted(true)?;
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }

    #[test]
    fn set_aperture() {
        test_set_aperture::<EnergyMeter>("input_1", "output_1");
    }

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<EnergyMeter>()
    }

    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light);
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(result.get("output_1"), Some(&input_data));
        assert_eq!(meter.get_energy(), Some(joule!(1.0)));
        Ok(())
    }

    #[test]
    fn analyze_raytrace() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;

        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1064.0), joule!(2.5))?;
        let rays = Rays::from(ray);

        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));

        let result = AnalysisRayTrace::analyze(&mut meter, input, &RayTraceConfig::default())?;
        assert!(result.contains_key("output_1"));
        assert_eq!(meter.get_energy(), Some(joule!(2.5)));
        Ok(())
    }

    #[test]
    fn analyze_apodization_warning() -> OpmResult<()> {
        test_analyze_apodization_warning::<EnergyMeter>()
    }

    #[test]
    fn analyze_inverted() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_inverted(true)?;
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(result.get("input_1"), Some(&input_data));
        Ok(())
    }

    #[test]
    fn reset_data() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_light_data(Some(LightData::Energy(create_he_ne_spec(1.0)?)));
        meter.set_apodization_warning(true);

        assert!(meter.light_data().is_some());
        assert!(meter.has_apodization_warning());

        meter.reset_data();

        assert!(meter.light_data().is_none());
        assert!(!meter.has_apodization_warning());
        assert_eq!(meter.get_energy(), None);
        Ok(())
    }

    #[test]
    fn debug() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        assert_eq!(format!("{meter:?}"), "no data");
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(format!("{meter:?}"), "Energy: 1 J (Type: IdealEnergyMeter)");
        Ok(())
    }

    #[test]
    fn report() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let NodeReportResult::Report(report) = meter.node_report("123", AnalyzerKind::Energy)?
        else {
            panic!("Report should not be `None`");
        };
        assert_eq!(report.name(), "energy meter");
        assert_eq!(report.node_type(), "energy meter");
        assert!(report.properties().contains("Energy"));
        assert!(report.properties().contains("Model"));
        if let Ok(Proptype::String(s)) = report.properties().get("Energy") {
            assert_eq!(s, "no data");
        } else {
            panic!("could not read Energy property");
        }
        if let Ok(Proptype::Metertype(t)) = report.properties().get("Model") {
            assert_eq!(t, &Metertype::IdealEnergyMeter);
        } else {
            panic!("could not read Model property");
        }

        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        meter.set_apodization_warning(true);

        let NodeReportResult::Report(report) = meter.node_report("123", AnalyzerKind::RayTrace)?
        else {
            panic!("Report should not be `None`");
        };
        if let Ok(Proptype::Energy(e)) = report.properties().get("Energy") {
            assert_eq!(e, &joule!(1.0));
        } else {
            panic!("could not read Energy property");
        }
        assert_eq!(report.notes().len(), 1);
        assert_eq!(report.notes()[0].level, ReportLevel::Warning);
        Ok(())
    }
}
