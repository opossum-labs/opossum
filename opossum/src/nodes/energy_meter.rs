#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::properties::{Properties, Proptype};
use crate::refractive_index::refr_index_vaccuum;
use crate::reporter::{NodeReport, PdfReportable};
use crate::surface::Plane;
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use uom::si::energy::joule;
use uom::si::f64::Energy;

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
impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Self::Metertype(value)
    }
}
impl PdfReportable for Metertype {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let element = match self {
            Self::IdealEnergyMeter => genpdf::elements::Text::new("ideal energy meter"),
            Self::IdealPowerMeter => genpdf::elements::Text::new("ideal power meter"),
        };
        let mut l = genpdf::elements::LinearLayout::vertical();
        l.push(element);
        Ok(l)
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
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::new("energy meter", "energy meter");
    props
        .create(
            "meter type",
            "model type of the meter",
            None,
            Metertype::default().into(),
        )
        .unwrap();
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}

impl Default for EnergyMeter {
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
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
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        props.set("meter type", meter_type.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
    /// Returns the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if
    /// - the property "meter type" does not exist.
    /// - the data format is wrong.
    #[must_use]
    pub fn meter_type(&self) -> Metertype {
        if let Ok(Proptype::Metertype(meter_type)) = self.props.get("meter type") {
            *meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if the property "meter type" can not be set.
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.props.set("meter type", meter_type.into()).unwrap();
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
        let data = incoming_data.get(inport).unwrap_or(&None);
        self.light_data = data.clone();
        if let Some(LightData::Geometric(rays)) = data {
            let mut rays = rays.clone();
            let z_position = rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
            let plane = Plane::new(z_position)?;
            rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
            if let Some(aperture) = self.ports().input_aperture("in1") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            if let Some(aperture) = self.ports().output_aperture("out1") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            self.light_data = Some(LightData::Geometric(rays.clone()));
            Ok(HashMap::from([(
                outport.into(),
                Some(LightData::Geometric(rays)),
            )]))
        } else {
            Ok(HashMap::from([(outport.into(), data.clone())]))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
    fn report(&self) -> Option<NodeReport> {
        let mut energy: Option<Energy> = None;
        if let Some(light_data) = &self.light_data {
            energy = match light_data {
                LightData::Energy(e) => Some(Energy::new::<joule>(e.spectrum.total_energy())),
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
                self.props.get("meter type").unwrap().clone(),
            )
            .unwrap();
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            props,
        ))
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
        analyzer::AnalyzerType, lightdata::DataEnergy, spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.properties().name().unwrap(), "energy meter");
        assert_eq!(node.properties().node_type().unwrap(), "energy meter");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "whitesmoke");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.properties().name().unwrap(), "test");
    }
    #[test]
    fn inverted() {
        let mut meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.properties().inverted().unwrap(), true);
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
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn analyze() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        }));
        input.insert("in1".into(), input_data.clone());
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.contains_key("out1"));
        assert_eq!(result.get("out1").unwrap(), &input_data);
    }
    #[test]
    fn analyze_inverted() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_property("inverted", true.into()).unwrap();
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        }));
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
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        }));
        input.insert("in1".into(), input_data.clone());
        meter.analyze(input, &AnalyzerType::Energy).unwrap();
        assert_eq!(format!("{meter:?}"), "Energy: 1 J (Type: IdealEnergyMeter)");
    }
    #[test]
    fn report() {
        let mut meter = EnergyMeter::default();
        let report = meter.report().unwrap();
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
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        }));
        input.insert("in1".into(), input_data.clone());
        meter.analyze(input, &AnalyzerType::Energy).unwrap();
        let report = meter.report().unwrap();
        if let Ok(Proptype::Energy(e)) = report.properties().get("Energy") {
            assert_eq!(e, &Energy::new::<joule>(1.0));
        } else {
            panic!("could not read Energy property");
        }
    }
}
