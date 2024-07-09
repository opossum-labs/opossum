#![warn(missing_docs)]
use itertools::izip;
use log::warn;
use nalgebra::MatrixXx2;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::length::{millimeter, nanometer};

use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nanometer,
    optic_ports::OpticPorts,
    optical::{Alignable, LightResult, Optical},
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable, PltBackEnd},
    properties::{Properties, Proptype},
    rays::Rays,
    refractive_index::refr_index_vaccuum,
    reporter::NodeReport,
    surface::Plane,
    utils::geom_transformation::Isometry,
};
use std::path::{Path, PathBuf};

/// A spot-diagram monitor
///
/// It simply generates a spot diagram of an incoming ray bundle.
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
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpotDiagram {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for SpotDiagram {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spot diagram");
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
impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
    /// # Attributes
    /// * `name`: name of the spot diagram
    ///
    /// # Panics
    /// This function may panic if the property "name" can not be set.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut sd = Self::default();
        sd.node_attr.set_property("name", name.into()).unwrap();
        sd.node_attr
            .set_property("plot_aperture", false.into())
            .unwrap();
        sd
    }
}

impl Alignable for SpotDiagram {}

impl Optical for SpotDiagram {
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
                "spot_diagram_{}_{}.svg",
                self.name(),
                uuid
            )));
            self.to_plot(&file_path, PltBackEnd::SVG)?;
        } else {
            warn!("spot diagram: no light data for export available. Cannot create plot!");
        }
        Ok(())
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            let mut transformed_rays = Rays::default();
            let iso = self.effective_iso().unwrap_or_else(Isometry::identity);
            for ray in rays {
                transformed_rays.add_ray(ray.inverse_transformed_ray(&iso));
            }
            props
                .create("Spot diagram", "2D spot diagram", None, self.clone().into())
                .unwrap();
            if let Some(c) = transformed_rays.energy_weighted_centroid() {
                props
                    .create(
                        "centroid x",
                        "x position of energy-weighted centroid",
                        None,
                        c.x.into(),
                    )
                    .unwrap();

                props
                    .create(
                        "centroid y",
                        "y position of energy-weightedcentroid",
                        None,
                        c.y.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.beam_radius_geo() {
                props
                    .create(
                        "geo beam radius",
                        "geometric beam radius",
                        None,
                        radius.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.energy_weighted_beam_radius_rms() {
                props
                    .create(
                        "rms beam radius",
                        "energy-weighted rms beam radius",
                        None,
                        radius.into(),
                    )
                    .unwrap();
            }
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

impl Dottable for SpotDiagram {
    fn node_color(&self) -> &str {
        "darkorange"
    }
}

impl From<SpotDiagram> for Proptype {
    fn from(value: SpotDiagram) -> Self {
        Self::SpotDiagram(value)
    }
}
impl Plottable for SpotDiagram {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm".into()))?
            .set(&PlotArgs::YLabel("distance in mm".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }

    fn get_plot_series(&self, plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = &self.light_data;
        match data {
            Some(LightData::Geometric(rays)) => {
                let (split_rays_bundles, wavelengths) =
                    rays.split_ray_bundle_by_wavelength(nanometer!(1.), true)?;
                let num_series = split_rays_bundles.len();
                let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

                let color_grad = colorous::TURBO;
                let wvl_range = if num_series == 1 {
                    1.
                } else {
                    (wavelengths[num_series - 1] * 2. - wavelengths[0] * 2.).get::<nanometer>()
                };

                //ray plot series
                for (ray_bundle, wvl) in izip!(split_rays_bundles.iter(), wavelengths.iter()) {
                    let grad_val = 0.42 + (*wvl - wavelengths[0]).get::<nanometer>() / wvl_range;
                    let rgbcolor = color_grad.eval_continuous(grad_val);
                    let series_label = format!("{:.1} nm", wvl.get::<nanometer>());
                    let iso = self.effective_iso().unwrap_or_else(Isometry::identity);
                    let xy_pos = ray_bundle.get_xy_rays_pos(true, &iso);
                    let data = PlotData::Dim2 {
                        xy_data: MatrixXx2::from_iterator(
                            xy_pos.nrows(),
                            xy_pos.iter().map(uom::si::f64::Length::get::<millimeter>),
                        ),
                    };
                    plt_series.push(PlotSeries::new(
                        &data,
                        RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, 1.),
                        Some(series_label),
                    ));
                }

                //aperture / shape plot series
                if let Ok(Proptype::Bool(plot_aperture)) = self.properties().get("plot_aperture") {
                    if *plot_aperture {
                        if let Some(aperture) = self.ports().input_aperture("in1") {
                            let plt_series_opt = aperture
                                .get_plot_series(&PlotType::Line2D(PlotParameters::default()))?;
                            if let Some(aperture_plt_series) = plt_series_opt {
                                plt_series.extend(aperture_plt_series);
                            }
                        }
                    }
                }

                match plt_type {
                    PlotType::Scatter2D(_) => Ok(Some(plt_series)),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::test_helper::test_helper::check_warnings;
    use crate::{
        analyzer::AnalyzerType, joule, lightdata::DataEnergy, nodes::test_helper::test_helper::*,
        position_distributions::Hexapolar, rays::Rays, spectrum_helper::create_he_ne_spec,
    };
    use tempfile::NamedTempFile;
    use uom::num_traits::Zero;
    use uom::si::f64::Length;
    #[test]
    fn default() {
        let mut node = SpotDiagram::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "spot diagram");
        assert_eq!(node.node_type(), "spot diagram");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.is_source(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "darkorange");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = SpotDiagram::new("test");
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = SpotDiagram::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = SpotDiagram::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<SpotDiagram>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<SpotDiagram>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("out1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<SpotDiagram>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = SpotDiagram::default();
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
    fn export_data() {
        testing_logger::setup();
        let mut sd = SpotDiagram::default();
        assert!(sd.export_data(Path::new(""), "").is_ok());
        check_warnings(vec![
            "spot diagram: no light data for export available. Cannot create plot!",
        ]);
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let path = NamedTempFile::new().unwrap();
        assert!(sd.export_data(path.path().parent().unwrap(), "").is_err());
        sd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        assert!(sd.export_data(path.path().parent().unwrap(), "").is_ok());
    }
    #[test]
    fn report() {
        let mut sd = SpotDiagram::default();
        let node_report = sd.report("").unwrap();
        assert_eq!(node_report.detector_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let node_report = sd.report("").unwrap();
        assert!(node_report.properties().contains("Spot diagram"));
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
        assert_eq!(nr_of_props, 5);
    }
}
