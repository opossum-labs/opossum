#![warn(missing_docs)]
use log::warn;
use serde::{Deserialize, Serialize};
use uom::si::length::nanometer;

use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nanometer,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    plottable::{PlotArgs, PlotParameters, PlotSeries, PlotType, Plottable, PltBackEnd},
    properties::{Properties, Proptype},
    refractive_index::refr_index_vaccuum,
    reporter::NodeReport,
    surface::Plane,
};
use std::{
    fmt::{Debug, Display},
    path::{Path, PathBuf},
};

#[non_exhaustive]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`Spectrometer`]. This is currently not used.
pub enum SpectrometerType {
    /// an ideal energy meter
    #[default]
    Ideal,
    /// Ocean Optics HR2000
    HR2000,
}
impl Display for SpectrometerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HR2000 => write!(f, "Ocean Optics HR2000"),
            Self::Ideal => write!(f, "ideal spectrometer"),
        }
    }
}
impl From<SpectrometerType> for Proptype {
    fn from(value: SpectrometerType) -> Self {
        Self::SpectrometerType(value)
    }
}

impl From<Spectrometer> for Proptype {
    fn from(value: Spectrometer) -> Self {
        Self::Spectrometer(value)
    }
}
/// An (ideal) spectrometer
///
/// It normally measures / displays the spectrum of the incoming light.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `spectrometer type
/// `
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(Serialize, Deserialize, Clone)]
pub struct Spectrometer {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for Spectrometer {
    /// create an ideal spectrometer.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spectrometer");
        node_attr
            .create_property(
                "spectrometer type",
                "model type of the spectrometer",
                None,
                SpectrometerType::Ideal.into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.create_input("in1").unwrap();
        ports.create_output("out1").unwrap();
        node_attr.set_apertures(ports);
        Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        }
    }
}
impl Spectrometer {
    /// Creates a new [`Spectrometer`] of the given [`SpectrometerType`].
    /// # Attributes
    /// * `name`: name of the  [`Spectrometer`]
    /// * `spectrometer_type`: [`SpectrometerType`] of the  [`Spectrometer`]
    ///
    /// # Panics
    /// This function panics if
    /// - the property "spectrometer" type can not be set.
    /// - the property "name" type can not be set.
    #[must_use]
    pub fn new(name: &str, spectrometer_type: SpectrometerType) -> Self {
        let mut spect = Self::default();
        spect
            .node_attr
            .set_property("spectrometer type", spectrometer_type.into())
            .unwrap();
        spect.node_attr.set_name(name);
        spect
    }
    /// Returns the meter type of this [`Spectrometer`].
    ///
    /// # Panics
    /// This function panics if
    /// - the property "spectrometer type" is not defined or
    /// - the meter type has the wrong data format
    #[must_use]
    pub fn spectrometer_type(&self) -> SpectrometerType {
        let meter_type = self
            .node_attr
            .get_property("spectrometer type")
            .unwrap()
            .clone();
        if let Proptype::SpectrometerType(meter_type) = meter_type {
            meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`Spectrometer`].
    /// /// # Attributes
    /// * `meter_type`: [`SpectrometerType`] of the  [`Spectrometer`]
    ///
    /// # Errors
    /// This function returns an error if
    /// - the property "spectrometer type" type can not be set.
    pub fn set_spectrometer_type(&mut self, meter_type: SpectrometerType) -> OpmResult<()> {
        self.node_attr
            .set_property("spectrometer type", meter_type.into())?;
        Ok(())
    }
}
impl Optical for Spectrometer {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
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
    fn export_data(&self, report_dir: &Path, uuid: &str) -> OpmResult<()> {
        if self.light_data.is_some() {
            let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                "spectrometer_{}_{}.svg",
                self.name(),
                uuid
            )));
            self.to_plot(&file_path, PltBackEnd::SVG)?;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "spectrometer: no light data available".into(),
            ))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(light_data) = data {
            let spectrum = match light_data {
                LightData::Energy(e) => Some(e.spectrum.clone()),
                LightData::Geometric(r) => r.to_spectrum(&nanometer!(0.2)).ok(),
                LightData::Fourier => None,
            };
            if spectrum.is_some() {
                props
                    .create("Spectrum", "Output spectrum", None, self.clone().into())
                    .unwrap();
                props
                    .create(
                        "Model",
                        "Spectrometer model",
                        None,
                        self.node_attr
                            .get_property("spectrometer type")
                            .unwrap()
                            .clone(),
                    )
                    .unwrap();
                if self.apodization_warning {
                    props
                    .create(
                        "Warning",
                        "warning during analysis",
                        None,
                        "Rays have been apodized at input aperture. Results might not be accurate.".into(),
                    )
                    .unwrap();
                }
            }
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

impl Debug for Spectrometer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => match data {
                LightData::Energy(data_energy) => {
                    let spectrum_range = data_energy.spectrum.range();
                    write!(
                        f,
                        "Spectrum {:.3} - {:.3} nm (Type: {:?})",
                        spectrum_range.start.get::<nanometer>(),
                        spectrum_range.end.get::<nanometer>(),
                        self.spectrometer_type()
                    )
                }
                _ => write!(f, "no spectrum data to display"),
            },
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Spectrometer {
    fn node_color(&self) -> &str {
        "lightseagreen"
    }
}
impl Plottable for Spectrometer {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("wavelength in nm".into()))?
            .set(&PlotArgs::YLabel("spectrum in arb. units".into()))?
            .set(&PlotArgs::PlotSize((1200, 800)))?
            .set(&PlotArgs::AxisEqual(false))?;

        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Line2D(plt_params.clone())
    }

    fn get_plot_series(&self, plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = &self.light_data;
        match data {
            Some(LightData::Geometric(rays)) => rays
                .to_spectrum(&nanometer!(0.2))?
                .get_plot_series(plt_type),
            Some(LightData::Energy(e)) => e.spectrum.get_plot_series(plt_type),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType,
        joule,
        lightdata::DataEnergy,
        nodes::{test_helper::test_helper::*, EnergyMeter},
        position_distributions::Hexapolar,
        rays::Rays,
        spectrum_helper::{create_he_ne_spec, create_visible_spec},
    };
    use num::Zero;
    use uom::si::f64::Length;

    #[test]
    fn debug() {
        let mut node = Spectrometer::default();
        assert_eq!(format!("{:?}", node), "no data");
        let mut input = LightResult::default();
        input.insert("in1".into(), LightData::Fourier);
        let _ = node.analyze(input, &AnalyzerType::Energy);
        assert_eq!(format!("{:?}", node), "no spectrum data to display");
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_visible_spec(),
        });
        input.insert("in1".into(), input_light.clone());
        let _ = node.analyze(input, &AnalyzerType::Energy);
        assert_eq!(
            format!("{:?}", node),
            "Spectrum 380.000 - 749.900 nm (Type: Ideal)"
        );
    }
    #[test]
    fn default() {
        let mut node = Spectrometer::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.spectrometer_type(), SpectrometerType::Ideal);
        assert_eq!(node.name(), "spectrometer");
        assert_eq!(node.node_type(), "spectrometer");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightseagreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = Spectrometer::new("test", SpectrometerType::HR2000);
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn set_meter_type() {
        let mut meter = Spectrometer::new("test", SpectrometerType::Ideal);
        meter
            .set_spectrometer_type(SpectrometerType::HR2000)
            .unwrap();
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn ports() {
        let meter = Spectrometer::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = Spectrometer::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<EnergyMeter>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<EnergyMeter>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Spectrometer::default();
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
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodazation_warning() {
        test_analyze_apodization_warning::<Spectrometer>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Spectrometer::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), input_light.clone());

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn report() {
        let mut sd = Spectrometer::default();
        let node_report = sd.report("").unwrap();
        assert_eq!(node_report.detector_type(), "spectrometer");
        assert_eq!(node_report.name(), "spectrometer");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = sd.report("").unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 2);
        assert!(node_props.contains("Spectrum"));
        assert!(node_props.contains("Model"));
    }
}
