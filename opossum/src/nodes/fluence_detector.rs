#![warn(missing_docs)]
use image::{DynamicImage, ImageBuffer, RgbImage};
use log::warn;
use nalgebra::{DMatrix, DVector};
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

/// A fluence monitor
///
/// It simply calculates the fluence (spatial energy distribution) of an incoming ray bundle.
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
pub struct FluenceDetector {
    light_data: Option<LightData>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("fluence detector", "fluence detector");
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for FluenceDetector {
    /// creates a fluence detector.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
        }
    }
}
impl FluenceDetector {
    /// Creates a new [`FluenceDetector`].
    /// # Attributes
    /// * `name`: name of the fluence detector
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

impl Optical for FluenceDetector {
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
        if let Some(LightData::Geometric(rays)) = &self.light_data {
            let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                "fluence_{}.png",
                self.properties().name()?
            )));

            let fluence_data_opt = rays.calc_transversal_fluence(None, None).ok();
            if let Some(fluence_data) = fluence_data_opt {
                fluence_data.to_plot(&file_path, (800, 800), PltBackEnd::BMP)
            } else {
                warn!("Fluence Detector diagram: no wavefront data for export available",);
                Ok(None)
            }
            // data.export(&file_path)
        } else {
            Err(OpossumError::Other(
                "Fluence detector: no light data for export available".into(),
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
            let fluence_data = rays.calc_transversal_fluence(None, None);
            if fluence_data.is_ok() {
                props
                    .create(
                        "Fluence",
                        "2D spatial energy distribution",
                        None,
                        fluence_data.unwrap().into(),
                    )
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

impl Dottable for FluenceDetector {
    fn node_color(&self) -> &str {
        "lightpurple"
    }
}

impl From<FluenceData> for Proptype {
    fn from(value: FluenceData) -> Self {
        Self::FluenceDetector(value)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FluenceData {
    pub peak: f64,
    pub average: f64,
    pub distribution: DMatrix<f64>,
    pub x_data: DVector<f64>,
    pub y_data: DVector<f64>,
}

impl FluenceData {
    pub fn new(
        peak: f64,
        average: f64,
        distribution: DMatrix<f64>,
        x_data: DVector<f64>,
        y_data: DVector<f64>,
    ) -> Self {
        Self {
            peak,
            average,
            distribution,
            x_data,
            y_data,
        }
    }
}

impl PdfReportable for FluenceData {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        layout.push(genpdf::elements::Paragraph::new(format!(
            "Peak fluence: {:.1} W/cm²",
            self.peak
        )));
        layout.push(genpdf::elements::Paragraph::new(format!(
            "Average fluence: {:.1} W/cm²",
            self.average
        )));
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

impl Plottable for FluenceData {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm".into()))?
            .set(&PlotArgs::YLabel("distance in mm".into()))?
            .set(&PlotArgs::CBarLabel("fluence in W/cm²".into()))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::ColorMesh(plt_params.clone())
    }

    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        match plt_type {
            PlotType::ColorMesh(_) => Ok(Some(PlotData::ColorMesh(
                self.x_data.clone(),
                self.y_data.clone(),
                self.distribution.clone(),
            ))),
            _ => Ok(None),
        }
    }
}
