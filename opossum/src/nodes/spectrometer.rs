#![warn(missing_docs)]
use log::warn;
use serde::{Deserialize, Serialize};
use uom::si::length::nanometer;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    dottable::Dottable,
    error::OpmResult,
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    nanometer,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    plottable::{PlotArgs, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::node_report::NodeReport,
};
use std::fmt::{Debug, Display};

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
///   - `spectrometer type`
///
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
        let mut spect = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        spect.update_surfaces().unwrap();
        spect
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
    #[must_use]
    pub fn new(name: &str, spectrometer_type: SpectrometerType) -> Self {
        let mut spect = Self::default();
        spect
            .node_attr
            .set_property("spectrometer type", spectrometer_type.into())
            .unwrap();
        spect.node_attr.set_name(name);
        spect.update_surfaces().unwrap();
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
impl OpticNode for Spectrometer {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(light_data) = data {
            let spectrum = match light_data {
                LightData::Energy(e) => Some(e.spectrum.clone()),
                LightData::Geometric(r) => r.to_spectrum(&nanometer!(0.2)).ok(),
                LightData::Fourier => None,
                LightData::GhostFocus(r) => {
                    let mut all_rays = Rays::default();
                    for rays in r {
                        all_rays.merge(rays);
                    }
                    all_rays.to_spectrum(&nanometer!(0.2)).ok()
                }
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
    fn reset_data(&mut self) {
        self.light_data = None;
        self.reset_optic_surfaces();
    }
}
impl Alignable for Spectrometer {}
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
impl LIDT for Spectrometer {}
impl Analyzable for Spectrometer {}
impl AnalysisGhostFocus for Spectrometer {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        AnalysisGhostFocus::analyze_single_surface_node(self, incoming_data, config)
    }
}
impl AnalysisEnergy for Spectrometer {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        self.light_data = Some(data.clone());
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for Spectrometer {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze_single_surface_node(self, incoming_data, config)
    }

    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
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

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = &self.light_data;
        match data {
            Some(LightData::Geometric(rays)) => rays
                .to_spectrum(&nanometer!(0.2))?
                .get_plot_series(plt_type, legend),
            Some(LightData::Energy(e)) => e.spectrum.get_plot_series(plt_type, legend),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule,
        lightdata::DataEnergy,
        nodes::{test_helper::test_helper::*, EnergyMeter},
        optic_ports::PortType,
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
        input.insert("input_1".into(), LightData::Fourier);
        let _ = AnalysisEnergy::analyze(&mut node, input);
        assert_eq!(format!("{:?}", node), "no spectrum data to display");
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_visible_spec(),
        });
        input.insert("input_1".into(), input_light.clone());
        AnalysisEnergy::analyze(&mut node, input).unwrap();
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
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = Spectrometer::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
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
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
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
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn report() {
        let mut sd = Spectrometer::default();
        let node_report = sd.node_report("").unwrap();
        assert_eq!(node_report.node_type(), "spectrometer");
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
        let node_report = sd.node_report("").unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 2);
        assert!(node_props.contains("Spectrum"));
        assert!(node_props.contains("Model"));
    }
}
