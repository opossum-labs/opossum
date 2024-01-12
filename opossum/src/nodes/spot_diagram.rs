#![warn(missing_docs)]
use image::{DynamicImage, ImageBuffer, RgbImage};
use serde_derive::{Deserialize, Serialize};

use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::plottable::{PlotArgs, PlotData, PlotParameters, PlotType, Plottable, PltBackEnd};
use crate::properties::{Properties, Proptype};
use crate::reporter::{NodeReport, PdfReportable};
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
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        self.light_data = data.clone();
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn export_data(&self, report_dir: &Path) -> OpmResult<Option<RgbImage>> {
        if self.light_data.is_some() {
            let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                "spot_diagram_{}.svg",
                self.properties().name()?
            )));
            self.to_plot(&file_path, (800, 800), PltBackEnd::SVG)

            // self.to_svg_plot(&file_path, (800,800))
            // data.export(&file_path)
        } else {
            Err(OpossumError::Other(
                "spot diagram: no light data for export available".into(),
            ))
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
        let img = self.to_plot(Path::new(""), (800, 800), PltBackEnd::Buf)?;
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
    fn to_plot(
        &self,
        f_path: &Path,
        img_size: (u32, u32),
        backend: PltBackEnd,
    ) -> OpmResult<Option<RgbImage>> {
        let mut plt_params = PlotParameters::default();
        match backend {
            PltBackEnd::Buf => plt_params.set(&PlotArgs::FigSize(img_size)),
            _ => plt_params
                .set(&PlotArgs::FName(
                    f_path.file_name().unwrap().to_str().unwrap().to_owned(),
                ))
                .set(&PlotArgs::FDir(
                    f_path.parent().unwrap().to_str().unwrap().to_owned(),
                ))
                .set(&PlotArgs::FigSize(img_size)),
        };
        plt_params.set(&PlotArgs::Backend(backend));

        let plt_type = PlotType::Scatter2D(plt_params);

        let plt_data_opt = self.get_plot_data(&plt_type)?;

        plt_data_opt.map_or(Ok(None), |plt_dat| plt_type.plot(&plt_dat))
    }

    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        let data = &self.light_data;
        match data {
            Some(LightData::Geometric(rays)) => {
                let rays_xy_pos = rays.get_xy_rays_pos();
                match plt_type {
                    PlotType::Scatter2D(_) => Ok(Some(PlotData::Dim2(rays_xy_pos))),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    // fn create_plot<B: plotters::prelude::DrawingBackend>(&self, root: &plotters::prelude::DrawingArea<B, plotters::coord::Shift>) -> OpmResult<()> {

    //     let data = &self.light_data;
    //     if let Some(LightData::Geometric(rays)) = data {
    //         let rays_xy_pos = rays.get_xy_rays_pos();
    //         let marker_color = RGBAColor{0:255, 1:0, 2:0, 3:1.};
    //         let xlabel = "x (mm)";
    //         let ylabel = "y (mm)";
    //         self.plot_2d_scatter(&PlotData::Dim2(rays_xy_pos), marker_color, vec![[true, true], [true, true]], xlabel, ylabel, root);
    //     }

    //     // let mut chart = ChartBuilder::on(root)
    //     //     .margin(15)
    //     //     .x_label_area_size(100)
    //     //     .y_label_area_size(100)
    //     //     .build_cartesian_2d(x_min..x_max, y_min..y_max)
    //     //     .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;

    //     // chart
    //     //     .configure_mesh()
    //     //     .x_desc("x (mm)")
    //     //     .y_desc("y (mm)")
    //     //     .label_style(TextStyle::from(("sans-serif", 30).into_font()))
    //     //     .draw()
    //     //     .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
    //     // let points: Vec<(f64, f64)> = self.rays.iter().map(|ray| (ray.pos.x, ray.pos.y)).collect();
    //     // let series = PointSeries::of_element(points, 5, &RED, &|c, s, st| {
    //     //     EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //     //         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //     // });

    //     // chart
    //     //     .draw_series(series)
    //     //     .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
    //     // root.present()
    //     //     .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;

    //     Ok(())
    // }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType,
        lightdata::DataEnergy,
        rays::{DistributionStrategy, Rays},
        spectrum::create_he_ne_spec,
    };
    use tempfile::tempdir;
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
    // #[test]
    // fn export_data() {
    //     let mut sd = SpotDiagram::default();
    //     assert!(sd.export_data(Path::new("")).is_err());
    //     sd.light_data = Some(LightData::Geometric(Rays::default()));
    //     let tmp_dir = tempdir().unwrap();
    //     assert!(sd.export_data(tmp_dir.path()).is_ok());
    //     tmp_dir.close().unwrap();
    // }
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
                Length::zero(),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
                &DistributionStrategy::Hexapolar(1),
            )
            .unwrap(),
        ));
        let node_report = sd.report().unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);
    }
}
