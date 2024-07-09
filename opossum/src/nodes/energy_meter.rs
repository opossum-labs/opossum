#![warn(missing_docs)]
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    joule,
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    refractive_index::refr_index_vaccuum,
    reporter::NodeReport,
    surface::Plane,
};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use uom::si::f64::Energy;

use super::node_attr::NodeAttr;

#[non_exhaustive]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`EnergyMeter`]. This is currently not used.
pub enum Metertype {
    /// an ideal energy meter
    #[default]
    IdealEnergyMeter,
    /// an ideal power meter (currently not used)
    IdealPowerMeter,
}
impl Display for Metertype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdealEnergyMeter => write!(f, "ideal energy meter"),
            Self::IdealPowerMeter => write!(f, "ideal power meter"),
        }
    }
}
impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Self::Metertype(value)
    }
}
/// An (ideal) energy / power meter.
///
/// It normally measures the total energy of the incoming light regardless of the wavelength, position, angle, polarization etc...
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Propertied
///   - `name`
///   - `inverted`
///   - `meter type`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
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
                None,
                Metertype::default().into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.create_input("in1").unwrap();
        ports.create_output("out1").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        }
    }
}
impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    /// # Attributes
    /// * `name`:           name of the [`EnergyMeter`]
    /// * `meter_type`:     specific [`Metertype`] of the [`EnergyMeter`]
    ///
    /// # Panics
    /// This function panics if the [`Properties`] `name` or `meter type` can not be set.
    #[must_use]
    pub fn new(name: &str, meter_type: Metertype) -> Self {
        let mut energy_meter = Self::default();
        energy_meter
            .node_attr
            .set_property("name", name.into())
            .unwrap();
        energy_meter
            .node_attr
            .set_property("meter type", meter_type.into())
            .unwrap();
        energy_meter
    }
    /// Returns the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if
    /// - the property "meter type" does not exist.
    /// - the data format is wrong.
    #[must_use]
    pub fn meter_type(&self) -> Metertype {
        if let Ok(Proptype::Metertype(meter_type)) = self.node_attr.get_property("meter type") {
            *meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if the property "meter type" can not be set.
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.node_attr
            .set_property("meter type", meter_type.into())
            .unwrap();
    }
}
impl Optical for EnergyMeter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.properties().inverted()? {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        self.light_data = Some(data.clone());
        if let LightData::Geometric(rays) = data {
            let mut rays = rays.clone();
            if let Some(iso) = self.effective_iso() {
                let plane = Plane::new(&iso);
                rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
            if let Some(aperture) = self.ports().input_aperture("in1") {
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", self as &mut dyn Optical);
                    self.apodization_warning = true;
                }
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            self.light_data = Some(LightData::Geometric(rays.clone()));
            if let Some(aperture) = self.ports().output_aperture("out1") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("output aperture not found".into()));
            };
            Ok(LightResult::from([(
                outport.into(),
                LightData::Geometric(rays),
            )]))
        } else {
            Ok(LightResult::from([(outport.into(), data.clone())]))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        let mut energy: Option<Energy> = None;
        if let Some(light_data) = &self.light_data {
            energy = match light_data {
                LightData::Energy(e) => Some(joule!(e.spectrum.total_energy())),
                LightData::Geometric(r) => Some(r.total_energy()),
                LightData::Fourier => None,
            };
        };
        let mut props = Properties::default();
        if let Some(e) = energy {
            props
                .create("Energy", "Output energy", None, e.into())
                .unwrap();
        } else {
            props
                .create("Energy", "Output energy", None, "no data".into())
                .unwrap();
        }
        props
            .create(
                "Model",
                "type of meter",
                None,
                self.node_attr.get_property("meter type").unwrap().clone(),
            )
            .unwrap();
        if self.apodization_warning {
            props
                .create(
                    "Warning",
                    "warning during analysis",
                    None,
                    "Rays have been apodized at input aperture. Results might not be accurate."
                        .into(),
                )
                .unwrap();
        }
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            uuid,
            props,
        ))
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
}

impl Debug for EnergyMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{} (Type: {:?})", data, self.meter_type()),
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for EnergyMeter {
    fn node_color(&self) -> &str {
        "whitesmoke"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType, lightdata::DataEnergy, nodes::test_helper::test_helper::*,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.name(), "energy meter");
        assert_eq!(node.node_type(), "energy meter");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.is_source(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "whitesmoke");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.name(), "test");
    }
    #[test]
    fn inverted() {
        test_inverted::<EnergyMeter>()
    }
    #[test]
    fn set_meter_type() {
        let mut meter = EnergyMeter::default();
        meter.set_meter_type(Metertype::IdealPowerMeter);
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
    }
    #[test]
    fn ports() {
        let meter = EnergyMeter::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = EnergyMeter::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<EnergyMeter>("in1", "out1");
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<EnergyMeter>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_data.clone());
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.contains_key("out1"));
        assert_eq!(result.get("out1").unwrap(), &input_data);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<EnergyMeter>()
    }
    #[test]
    fn analyze_inverted() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_inverted(true).unwrap();
        let input_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), input_data.clone());
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.contains_key("in1"));
        assert_eq!(result.get("in1").unwrap(), &input_data);
    }
    #[test]
    fn debug() {
        let mut meter = EnergyMeter::default();
        assert_eq!(format!("{meter:?}"), "no data");
        let mut input = LightResult::default();
        let input_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_data.clone());
        meter.analyze(input, &AnalyzerType::Energy).unwrap();
        assert_eq!(format!("{meter:?}"), "Energy: 1 J (Type: IdealEnergyMeter)");
    }
    #[test]
    fn report() {
        let mut meter = EnergyMeter::default();
        let report = meter.report("123").unwrap();
        assert_eq!(report.name(), "energy meter");
        assert_eq!(report.detector_type(), "energy meter");
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
        let input_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_data.clone());
        meter.analyze(input, &AnalyzerType::Energy).unwrap();
        let report = meter.report("123").unwrap();
        if let Ok(Proptype::Energy(e)) = report.properties().get("Energy") {
            assert_eq!(e, &joule!(1.0));
        } else {
            panic!("could not read Energy property");
        }
    }
}
