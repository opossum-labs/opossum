#![warn(missing_docs)]
//! fluence measurement node
use log::warn;
use nalgebra::{DMatrix, DVector};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter, radiant_exposure::joule_per_square_centimeter};

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
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::node_report::NodeReport,
};

///alias for uom `RadiantExposure`, as this name is rather uncommon to use for laser scientists
pub type Fluence = uom::si::f64::RadiantExposure;

/// A fluence monitor
///
/// It simply calculates the fluence (spatial energy distribution) of an incoming [`Ray`](crate::ray::Ray) bundle.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(Clone, Debug)]
pub struct FluenceDetector {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for FluenceDetector {
    /// creates a fluence detector.
    fn default() -> Self {
        let mut fld = Self {
            light_data: None,
            node_attr: NodeAttr::new("fluence detector"),
            apodization_warning: false,
        };
        fld.update_surfaces().unwrap();
        fld
    }
}
impl FluenceDetector {
    /// Creates a new [`FluenceDetector`].
    /// # Attributes
    /// * `name`: name of the fluence detector
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut fld = Self::default();
        fld.node_attr.set_name(name);
        fld
    }
}

impl OpticNode for FluenceDetector {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            let fluence_data_res = rays.calc_fluence_at_position();
            if let Ok(fluence_data) = fluence_data_res {
                props
                    .create(
                        "Fluence",
                        "2D spatial energy distribution",
                        None,
                        fluence_data.clone().into(),
                    )
                    .unwrap();

                props
                    .create(
                        "Peak Fluence",
                        "Peak fluence of the distribution",
                        None,
                        Proptype::Fluence(fluence_data.peak),
                    )
                    .unwrap();

                props
                    .create(
                        "Average Fluence",
                        "Average Fluence of the distribution",
                        None,
                        Proptype::Fluence(fluence_data.average),
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

impl Alignable for FluenceDetector {}
impl Dottable for FluenceDetector {
    fn node_color(&self) -> &str {
        "hotpink"
    }
}
impl LIDT for FluenceDetector {}
impl Analyzable for FluenceDetector {}
impl AnalysisGhostFocus for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        self.analyze_single_surface_detector(incoming_data, config)
    }
}
impl AnalysisEnergy for FluenceDetector {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        // self.light_data = Some(data.clone());
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.raytrace_single_surface_detector(incoming_data, config)
    }
    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
    }
}

impl From<FluenceData> for Proptype {
    fn from(value: FluenceData) -> Self {
        Self::FluenceDetector(value)
    }
}

/// Struct to hold the fluence information of a beam
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FluenceData {
    /// peak fluence of the beam
    peak: Fluence,
    /// average fluence of the beam
    average: Fluence,
    /// 2d fluence distribution of the beam.
    interp_distribution: DMatrix<Fluence>,
    /// x coordinates of the fluence distribution
    x_data: DVector<Length>,
    /// y coordinates of the fluence distribution
    y_data: DVector<Length>,
}

impl FluenceData {
    /// Constructs a new [`FluenceData`] struct
    #[must_use]
    pub const fn new(
        peak: Fluence,
        average: Fluence,
        interp_distribution: DMatrix<Fluence>,
        x_data: DVector<Length>,
        y_data: DVector<Length>,
    ) -> Self {
        Self {
            peak,
            average,
            interp_distribution,
            x_data,
            y_data,
        }
    }

    /// Returns the peak fluence value
    #[must_use]
    pub const fn get_peak_fluence(&self) -> Fluence {
        self.peak
    }

    /// Returns the average fluence value
    #[must_use]
    pub const fn get_average_fluence(&self) -> Fluence {
        self.average
    }

    /// Returns the fluence distribution and the corresponding x and y axes in a tuple (x, y, distribution)
    #[must_use]
    pub fn get_fluence_distribution(&self) -> (DVector<Length>, DVector<Length>, DMatrix<Fluence>) {
        (
            self.x_data.clone(),
            self.y_data.clone(),
            self.interp_distribution.clone(),
        )
    }

    /// Returns length of the x data points (columns)
    #[must_use]
    pub fn len_x(&self) -> usize {
        self.x_data.len()
    }

    /// Returns length of the y data points (rows)
    #[must_use]
    pub fn len_y(&self) -> usize {
        self.y_data.len()
    }

    /// Returns the shape of the interpolation distribution in pixels
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        self.interp_distribution.shape()
    }
}
impl Plottable for FluenceData {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm".into()))?
            .set(&PlotArgs::YLabel("distance in mm".into()))?
            .set(&PlotArgs::CBarLabel("fluence in J/cm²".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?
            .set(&PlotArgs::ExpandBounds(false))?;

        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::ColorMesh(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let (nrows, ncols) = self.interp_distribution.shape();

        match plt_type {
            PlotType::ColorMesh(_) => {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: DVector::from_iterator(
                        self.len_x(),
                        self.x_data
                            .iter()
                            .map(uom::si::f64::Length::get::<millimeter>),
                    ),
                    y_dat_m: DVector::from_iterator(
                        self.len_y(),
                        self.y_data
                            .iter()
                            .map(uom::si::f64::Length::get::<millimeter>),
                    ),
                    z_dat_nxm: DMatrix::from_iterator(
                        nrows,
                        ncols,
                        self.interp_distribution
                            .iter()
                            .map(uom::si::f64::RadiantExposure::get::<joule_per_square_centimeter>),
                    ),
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::optic_ports::PortType;
    use crate::{
        joule, lightdata::DataEnergy, millimeter, nanometer, nodes::test_helper::test_helper::*,
        position_distributions::Hexapolar, rays::Rays, spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = FluenceDetector::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "fluence detector");
        assert_eq!(node.node_type(), "fluence detector");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "hotpink");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = FluenceDetector::new("test");
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = FluenceDetector::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = FluenceDetector::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<FluenceDetector>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<FluenceDetector>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = FluenceDetector::default();
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
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<FluenceDetector>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = FluenceDetector::default();
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
    // #[test]
    // fn export_data() {
    //     let mut fd = FluenceDetector::default();
    //     assert!(fd.export_data(Path::new(""), "").is_err());
    //     fd.light_data = Some(Rays::default());
    //     let path = NamedTempFile::new().unwrap();
    //     assert!(fd.export_data(path.path().parent().unwrap(), "").is_ok());
    //     fd.light_data = Some(
    //         Rays::new_uniform_collimated(
    //             nanometer!(1053.0),
    //             joule!(1.0),
    //             &Hexapolar::new(Length::zero(), 1).unwrap(),
    //         )
    //         .unwrap(),
    //     );
    //     assert!(fd.export_data(path.path().parent().unwrap(), "").is_ok());
    // }
    #[test]
    fn report() {
        let mut fd = FluenceDetector::default();
        let node_report = fd.node_report("123").unwrap();
        assert_eq!(node_report.node_type(), "fluence detector");
        assert_eq!(node_report.name(), "fluence detector");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        fd.light_data = Some(LightData::Geometric(Rays::default()));
        let node_report = fd.node_report("123").unwrap();
        assert!(!node_report.properties().contains("Fluence"));
        fd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = fd.node_report("123").unwrap();
        assert!(node_report.properties().contains("Fluence"));
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 3);
    }
}
