#![warn(missing_docs)]
use image::{DynamicImage, ImageBuffer, RgbImage};
use itertools::izip;
use log::warn;
use plotters::style::RGBAColor;
use serde_derive::{Deserialize, Serialize};
use uom::si::f64::Length;
use uom::si::length::nanometer;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::plottable::{
    PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable, PltBackEnd,
};
use crate::properties::{Properties, Proptype};
use crate::refractive_index::refr_index_vaccuum;
use crate::reporter::{NodeReport, PdfReportable};
use crate::surface::Plane;
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
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
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("spot diagram", "spot diagram");
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for SpotDiagram {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
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
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
}

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
        let data = incoming_data.get(inport).unwrap_or(&None);
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
    fn export_data(&self, report_dir: &Path) -> OpmResult<Option<RgbImage>> {
        if self.light_data.is_some() {
            let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                "spot_diagram_{}.svg",
                self.properties().name()?
            )));
            self.to_plot(&file_path, PltBackEnd::SVG)
        } else {
            warn!("spot diagram: no light data for export available. Cannot create plot!");
            Ok(None)
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
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            props
                .create("Spot diagram", "2D spot diagram", None, self.clone().into())
                .unwrap();
            if let Some(c) = rays.centroid() {
                props
                    .create("centroid x", "x position of centroid", None, c.x.into())
                    .unwrap();

                props
                    .create("centroid y", "y position of centroid", None, c.y.into())
                    .unwrap();
            }
            if let Some(radius) = rays.beam_radius_geo() {
                props
                    .create(
                        "geo beam radius",
                        "geometric beam radius",
                        None,
                        radius.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = rays.beam_radius_rms() {
                props
                    .create("rms beam radius", "rms beam radius", None, radius.into())
                    .unwrap();
            }
        }
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            props,
        ))
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

impl PdfReportable for SpotDiagram {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let img = self.to_plot(Path::new(""), PltBackEnd::Buf)?;
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(
                img.unwrap_or_else(ImageBuffer::default),
            ))
            .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
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
                    rays.split_ray_bundle_by_wavelength(Length::new::<nanometer>(1.), true)?;
                let num_series = split_rays_bundles.len();
                let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

                let color_grad = colorous::TURBO;
                let wvl_range = if num_series == 1 {
                    1.
                } else {
                    (wavelengths[num_series - 1] * 2. - wavelengths[0] * 2.).get::<nanometer>()
                };

                for (ray_bundle, wvl) in izip!(split_rays_bundles.iter(), wavelengths.iter()) {
                    let grad_val = 0.42 + (*wvl - wavelengths[0]).get::<nanometer>() / wvl_range;
                    let rgbcolor = color_grad.eval_continuous(grad_val);
                    let series_label = format!("{:.1} nm", wvl.get::<nanometer>());
                    let data = PlotData::Dim2 {
                        xy_data: ray_bundle.get_xy_rays_pos(true),
                    };
                    plt_series.push(PlotSeries::new(
                        &data,
                        RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, 1.),
                        Some(series_label),
                    ));
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
    use crate::position_distributions::Hexapolar;
    use crate::{
        analyzer::AnalyzerType, lightdata::DataEnergy, rays::Rays,
        spectrum_helper::create_he_ne_spec,
    };
    use tempfile::NamedTempFile;
    use uom::num_traits::Zero;
    use uom::si::{
        energy::{joule, Energy},
        f64::Length,
        length::nanometer,
    };
    #[test]
    fn default() {
        let node = SpotDiagram::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.properties().name().unwrap(), "spot diagram");
        assert_eq!(node.properties().node_type().unwrap(), "spot diagram");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "darkorange");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = SpotDiagram::new("test");
        assert_eq!(meter.properties().name().unwrap(), "test");
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
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        let mut node = SpotDiagram::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn analyze_ok() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("out1").unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = SpotDiagram::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn export_data() {
        testing_logger::setup();
        let mut sd = SpotDiagram::default();
        let exp = sd.export_data(Path::new(""));
        assert!(exp.is_ok());
        assert!(exp.unwrap().is_none());
        let warning = "spot diagram: no light data for export available. Cannot create plot!";
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(captured_logs[0].body, warning);
        });
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let path = NamedTempFile::new().unwrap();
        assert!(sd.export_data(path.path().parent().unwrap()).is_err());
        sd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        assert!(sd.export_data(path.path().parent().unwrap()).is_ok());
    }
    #[test]
    fn report() {
        let mut sd = SpotDiagram::default();
        let node_report = sd.report().unwrap();
        assert_eq!(node_report.detector_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let node_report = sd.report().unwrap();
        assert!(node_report.properties().contains("Spot diagram"));
        sd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = sd.report().unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);
    }
}
