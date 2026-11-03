#![warn(missing_docs)]
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use uom::si::length::{meter, nanometer};

use crate::{
    analyzers::{
        AnalyzerKind, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, SurfacePorts,
        observable::{MetricDimension, ObservableDescriptor},
        optic_node::APERTURE_LIMIT_NOTE,
    },
    error::OpmResult,
    geometry::{
        Geometry, SurfaceShape,
        body::{OPTIC_SHAPE, default_square_optic_shape},
    },
    light::{LightData, Rays, Spectrum},
    nodes::NodeRegistration,
    properties::{Properties, Proptype, validator::Validator},
    reporting::node_report::{NodeReport, NodeReportResult},
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

inventory::submit! {
    NodeRegistration::new::<Spectrometer>("spectrometer", "ideal spectrometer")
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
///   - `optic shape`: the window the light is recorded within, a 25 mm square by default; light
///     outside it passes unrecorded
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(OpmNode, Clone)]
#[opm_node("lightseagreen")]
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
                SpectrometerType::Ideal.into(),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                OPTIC_SHAPE,
                "window the light is recorded within",
                Validator::OpticShape {
                    may_be_open: true,
                    bored: false,
                },
                default_square_optic_shape().into(),
            )
            .unwrap();
        let mut spect = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        spect.create_default_surface_properties().unwrap();
        spect.update_surfaces().unwrap();
        spect
    }
}
impl Spectrometer {
    /// Creates a new [`Spectrometer`] of the given [`SpectrometerType`].
    /// # Attributes
    /// * `name`: name of the  [`Spectrometer`]
    /// * `spectrometer type`: [`SpectrometerType`] of the  [`Spectrometer`]
    ///
    /// # Errors
    ///
    /// This function returns an error if internally the property `spectrometer type` can not be set.
    pub fn new(name: &str, spectrometer_type: SpectrometerType) -> OpmResult<Self> {
        let mut spect = Self::default();
        spect
            .node_attr
            .set_property("spectrometer type", spectrometer_type.into())?;
        spect.node_attr.set_name(name);
        spect.update_surfaces()?;
        Ok(spect)
    }
    /// Returns the meter type of this [`Spectrometer`].
    #[must_use]
    pub fn spectrometer_type(&self) -> SpectrometerType {
        self.node_attr
            .get_property("spectrometer type")
            .ok()
            .and_then(|prop| {
                if let Proptype::SpectrometerType(meter_type) = prop {
                    Some(*meter_type)
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
    /// Sets the meter type of this [`Spectrometer`].
    ///
    /// # Errors
    /// This function returns an error if
    /// - the property `spectrometer type` type can not be set.
    pub fn set_spectrometer_type(&mut self, meter_type: SpectrometerType) -> OpmResult<()> {
        self.node_attr
            .set_property("spectrometer type", meter_type.into())?;
        Ok(())
    }
    /// Returns the spectrum stored in the lightdata of this [`Spectrometer`].
    ///
    /// Returns `None` if no lightdata is available, Some(Spectrum) otherwise.
    #[must_use]
    pub fn get_spectrum(&self) -> Option<Spectrum> {
        self.light_data
            .as_ref()
            .and_then(|light_data| match light_data {
                LightData::Energy(s) => Some(s.clone()),
                LightData::Geometric(r) => r.to_auto_spectrum().ok(),
                LightData::Fourier => None,
                LightData::GhostFocus(r) => {
                    let mut all_rays = Rays::default();
                    for rays in r {
                        all_rays.merge(rays);
                    }
                    // Prefer adaptive binning consistent with geometric rays
                    all_rays.to_auto_spectrum().ok()
                }
            })
    }
}
impl OpticNode for Spectrometer {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::detector(
            SurfaceShape::Plane,
            self.cross_section()?,
        )?))
    }

    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }

    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
    }

    fn node_report(&self, uuid: &str, _analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        let mut props = Properties::default();
        if let Some(spectrum) = self.get_spectrum() {
            props.create("Spectrum", "Output spectrum", spectrum.into())?;
            props.create(
                "Model",
                "Spectrometer model",
                self.spectrometer_type().into(),
            )?;
        }

        let mut report = NodeReport::new(self.node_type(), self.name(), uuid, props);

        if self.apodization_warning {
            report.add_note(crate::reporting::report_note::ReportNote::new(
                crate::reporting::report_note::ReportLevel::Warning,
                APERTURE_LIMIT_NOTE,
            ));
        }

        Ok(NodeReportResult::Report(report))
    }

    fn set_light_data(&mut self, ld: Option<LightData>) {
        self.light_data = ld;
    }

    fn observable_metrics(&self, _analyzer: AnalyzerKind) -> Vec<ObservableDescriptor> {
        vec![
            ObservableDescriptor::new(
                "center wavelength",
                "Spectral center wavelength (first moment)",
                MetricDimension::Length,
            )
            .with_default_target(0.0),
            ObservableDescriptor::new(
                "peak wavelength",
                "Spectral peak wavelength",
                MetricDimension::Length,
            )
            .with_default_target(0.0),
            ObservableDescriptor::new(
                "bandwidth rms",
                "Spectral RMS bandwidth",
                MetricDimension::Length,
            )
            .with_default_target(0.0),
            ObservableDescriptor::new(
                "fwhm",
                "Full width at half maximum (FWHM)",
                MetricDimension::Length,
            )
            .with_default_target(0.0),
            ObservableDescriptor::new(
                "total energy",
                "Total integrated spectral energy",
                MetricDimension::Energy,
            )
            .with_default_target(0.0),
        ]
    }

    fn read_observable(&self, metric_name: &str) -> Option<f64> {
        let spectrum = self.get_spectrum()?;
        let lower = metric_name.to_lowercase();
        match lower.as_str() {
            "center wavelength" | "central wavelength" | "centroid wavelength" => {
                let center = spectrum.center_wavelength();
                if center.value.is_finite() && center.value > 0.0 {
                    Some(center.get::<meter>())
                } else {
                    None
                }
            }
            "peak wavelength" => spectrum.peak_wavelength().map(|l| l.get::<meter>()),
            "bandwidth rms" | "spectral bandwidth rms" | "bandwidth" => {
                spectrum.spectral_bandwidth_rms().map(|l| l.get::<meter>())
            }
            "fwhm" | "bandwidth fwhm" => spectrum.fwhm().map(|l| l.get::<meter>()),
            "total energy" | "energy" => Some(spectrum.total_energy()),
            _ => None,
        }
    }
}
impl Debug for Spectrometer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.light_data.is_none() {
            return write!(f, "no data");
        }

        // Use get_spectrum to cover Energy, Geometric, and GhostFocus light data
        if let Some(spectrum) = self.get_spectrum() {
            let spectrum_range = spectrum.range();
            write!(
                f,
                "Spectrum {:.3} - {:.3} nm (Type: {:?})",
                spectrum_range.start.get::<nanometer>(),
                spectrum_range.end.get::<nanometer>(),
                self.spectrometer_type()
            )
        } else {
            write!(f, "no spectrum data to display")
        }
    }
}
impl AnalysisGhostFocus for Spectrometer {}
impl AnalysisEnergy for Spectrometer {}
impl AnalysisRayTrace for Spectrometer {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        core_optics::PortType,
        distributions::position::Hexapolar,
        joule,
        light::{
            LightResult, Rays,
            spectrum_helper::{create_he_ne_spec, create_visible_spec},
        },
        nanometer,
        nodes::test_helper::helper::*,
    };
    use num_traits::Zero;
    use uom::si::f64::Length;

    #[test]
    fn debug() -> OpmResult<()> {
        let mut node = Spectrometer::default();
        assert_eq!(format!("{:?}", node), "no data");
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Fourier);
        let _ = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default());
        assert_eq!(format!("{:?}", node), "no spectrum data to display");
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_visible_spec()?);
        input.insert("input_1".into(), input_light.clone());
        AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert_eq!(
            format!("{:?}", node),
            "Spectrum 380.000 - 750.000 nm (Type: Ideal)"
        );
        Ok(())
    }
    #[test]
    fn default() {
        let node = Spectrometer::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.spectrometer_type(), SpectrometerType::Ideal);
        assert_eq!(node.name(), "spectrometer");
        assert_eq!(node.node_type(), "spectrometer");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "lightseagreen");
    }
    #[test]
    fn new() -> OpmResult<()> {
        let sm = Spectrometer::new("test", SpectrometerType::HR2000)?;
        assert_eq!(sm.name(), "test");
        assert!(sm.light_data.is_none());
        assert_eq!(sm.spectrometer_type(), SpectrometerType::HR2000);
        Ok(())
    }
    #[test]
    fn set_meter_type() -> OpmResult<()> {
        let mut sm = Spectrometer::new("test", SpectrometerType::Ideal)?;
        sm.set_spectrometer_type(SpectrometerType::HR2000)?;
        assert_eq!(sm.spectrometer_type(), SpectrometerType::HR2000);
        Ok(())
    }
    #[test]
    fn ports() {
        let sm = Spectrometer::default();
        assert_eq!(sm.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(sm.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut sm = Spectrometer::default();
        sm.set_inverted(true)?;
        assert_eq!(sm.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(sm.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<Spectrometer>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<Spectrometer>()
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert_eq!(output, Some(&input_light));
        Ok(())
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<Spectrometer>()
    }
    #[test]
    fn records_inside_and_passes_all() -> OpmResult<()> {
        test_detector_records_inside_and_passes_all::<Spectrometer>(|spectrometer| {
            spectrometer.light_data.as_ref()
        })
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = Spectrometer::default();
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert_eq!(output, Some(&input_light));
        Ok(())
    }
    #[test]
    fn report() -> OpmResult<()> {
        let mut sd = Spectrometer::default();
        let NodeReportResult::Report(node_report) = sd.node_report("", AnalyzerKind::Energy)?
        else {
            panic!("Node report should not be `None`");
        };
        assert_eq!(node_report.node_type(), "spectrometer");
        assert_eq!(node_report.name(), "spectrometer");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(Length::zero(), 1)?,
        )?));
        let NodeReportResult::Report(node_report) = sd.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Node report should not be `None`");
        };
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 2);
        assert!(node_props.contains("Spectrum"));
        assert!(node_props.contains("Model"));
        Ok(())
    }
    #[test]
    fn observable_metrics_spectrometer() {
        let spectrometer = Spectrometer::default();
        // Spectrometer metrics are valid across all analyzer kinds
        for kind in [
            AnalyzerKind::Energy,
            AnalyzerKind::RayTrace,
            AnalyzerKind::GhostFocus,
        ] {
            let metrics = spectrometer.observable_metrics(kind);
            assert_eq!(metrics.len(), 5);
            assert_eq!(metrics[0].name, "center wavelength");
            assert_eq!(metrics[0].dimension, MetricDimension::Length);
            assert_eq!(metrics[1].name, "peak wavelength");
            assert_eq!(metrics[1].dimension, MetricDimension::Length);
            assert_eq!(metrics[2].name, "bandwidth rms");
            assert_eq!(metrics[2].dimension, MetricDimension::Length);
            assert_eq!(metrics[3].name, "fwhm");
            assert_eq!(metrics[3].dimension, MetricDimension::Length);
            assert_eq!(metrics[4].name, "total energy");
            assert_eq!(metrics[4].dimension, MetricDimension::Energy);
        }
    }
    #[test]
    fn read_observable_spectrometer() -> OpmResult<()> {
        let mut spectrometer = Spectrometer::default();
        assert_eq!(spectrometer.read_observable("center wavelength"), None);

        // 1. Evaluate with spectral light data (He-Ne laser at 632.8 nm, 1.0 J)
        spectrometer.set_light_data(Some(LightData::Energy(create_he_ne_spec(1.0)?)));

        let center_wvl = spectrometer
            .read_observable("center wavelength")
            .expect("Center wavelength must be measurable");
        approx::assert_relative_eq!(center_wvl, 632.8e-9, epsilon = 1.0e-9);

        let peak_wvl = spectrometer
            .read_observable("peak wavelength")
            .expect("Peak wavelength must be measurable");
        approx::assert_relative_eq!(peak_wvl, 632.8e-9, epsilon = 1.0e-9);

        let energy = spectrometer
            .read_observable("total energy")
            .expect("Total energy must be measurable");
        approx::assert_relative_eq!(energy, 1.0, epsilon = 1.0e-6);

        // Case-insensitivity check
        assert!(spectrometer.read_observable("Center Wavelength").is_some());
        assert!(spectrometer.read_observable("fwhm").is_some());
        assert_eq!(spectrometer.read_observable("invalid_metric"), None);
        Ok(())
    }
}
