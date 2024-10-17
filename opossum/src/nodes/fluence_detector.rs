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
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::{OpticPorts, PortType},
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::analysis_report::NodeReport,
    surface::{OpticalSurface, Plane},
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
    light_data: Option<Rays>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for FluenceDetector {
    /// creates a fluence detector.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("fluence detector");
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "in1").unwrap();
        ports.add(&PortType::Output, "out1").unwrap();
        node_attr.set_ports(ports);
        Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        }
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
    // fn export_data(&self, report_dir: &Path, uuid: &str) -> OpmResult<()> {
    //     self.light_data.as_ref().map_or_else(
    //         || {
    //             Err(OpossumError::Other(
    //                 "Fluence detector: no light data for export available".into(),
    //             ))
    //         },
    //         |rays| {
    //             let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
    //                 "fluence_{}_{}.png",
    //                 self.name(),
    //                 uuid
    //             )));
    //             let _ = rays.calc_fluence_at_position().map_or_else(
    //                 |_| {
    //                     warn!("Fluence Detector diagram: no fluence data for export available",);
    //                     Ok(None)
    //                 },
    //                 |fluence_data| fluence_data.to_plot(&file_path, PltBackEnd::Bitmap),
    //             );
    //             Ok(())
    //         },
    //     )
    // }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(rays) = data {
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
        todo!();
        // self.surface.reset_hit_map();
    }
}

impl Dottable for FluenceDetector {
    fn node_color(&self) -> &str {
        "hotpink"
    }
}
impl Analyzable for FluenceDetector {}
impl AnalysisGhostFocus for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
    ) -> OpmResult<LightRays> {
        let (in_port, out_port) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(bouncing_rays) = incoming_data.get(in_port) else {
            return Ok(LightRays::default());
        };
        let mut rays = bouncing_rays.clone();
        if let Some(iso) = self.effective_iso() {
            let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
            rays.refract_on_surface(&mut plane, None)?;
        } else {
            return Err(OpossumError::Analysis(
                "no location for surface defined. Aborting".into(),
            ));
        }
        // merge all rays
        let mut ray_cache = self
            .light_data
            .clone()
            .map_or_else(Rays::default, |rays| rays);
        ray_cache.merge(&rays);
        self.light_data = Some(ray_cache);

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays.clone());
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for FluenceDetector {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        // self.light_data = Some(data.clone());
        Ok(LightResult::from([(outport.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            let mut rays = rays.clone();
            if let Some(iso) = self.effective_iso() {
                let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
                rays.refract_on_surface(&mut plane, None)?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
            if let Some(aperture) = self.ports().aperture(&PortType::Input, "in1") {
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", self as &mut dyn OpticNode);
                    self.apodization_warning = true;
                }
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            self.light_data = Some(rays.clone());
            if let Some(aperture) = self.ports().aperture(&PortType::Output, "out1") {
                rays.apodize(aperture)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
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
        assert_eq!(meter.ports().names(&PortType::Input), vec!["in1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = FluenceDetector::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["out1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["in1"]);
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
        input.insert("in1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
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
        input.insert("out1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1");
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
        fd.light_data = Some(Rays::default());
        let node_report = fd.node_report("123").unwrap();
        assert!(!node_report.properties().contains("Fluence"));
        fd.light_data = Some(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        );
        let node_report = fd.node_report("123").unwrap();
        assert!(node_report.properties().contains("Fluence"));
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 3);
    }
}
