#![warn(missing_docs)]
//! fluence measurement node
use image::{DynamicImage, ImageBuffer, RgbImage};
use log::warn;
use nalgebra::{DMatrix, DVector};
use plotters::style::RGBAColor;
use serde_derive::{Deserialize, Serialize};

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
        if let Some(LightData::Geometric(rays)) = &self.light_data {
            let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                "fluence_{}.png",
                self.properties().name()?
            )));

            let fluence_data_opt = rays.calc_fluence_at_position().ok();
            fluence_data_opt.map_or_else(
                || {
                    warn!("Fluence Detector diagram: no fluence data for export available",);
                    Ok(None)
                },
                |fluence_data| fluence_data.to_plot(&file_path, (1000, 500), PltBackEnd::BMP),
            )
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
            let fluence_data_res = rays.calc_fluence_at_position();
            if let Ok(fluence_data) = fluence_data_res {
                props
                    .create(
                        "Fluence",
                        "2D spatial energy distribution",
                        None,
                        fluence_data.into(),
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

/// Struct to hold the fluence information of a beam
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FluenceData {
    /// peak fluence of the beam
    peak: f64,
    /// average fluence of the beam
    average: f64,
    /// 2d fluence distribution of the beam.
    interp_distribution: DMatrix<f64>,
    /// x coordinates of the fluence distribution
    x_data: DVector<f64>,
    /// y coordinates of the fluence distribution
    y_data: DVector<f64>,
}

impl FluenceData {
    /// Constructs a new [`FluenceData`] struct
    #[must_use]
    pub const fn new(
        peak: f64,
        average: f64,
        interp_distribution: DMatrix<f64>,
        x_data: DVector<f64>,
        y_data: DVector<f64>,
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
    pub const fn get_peak_fluence(&self) -> f64 {
        self.peak
    }

    /// Returns the average fluence value
    #[must_use]
    pub const fn get_average_fluence(&self) -> f64 {
        self.average
    }

    /// Returns the fluence distribution and the corresponding x and y axes in a tuple (x, y, distribution)
    #[must_use]
    pub fn get_fluence_distribution(&self) -> (DVector<f64>, DVector<f64>, DMatrix<f64>) {
        (
            self.x_data.clone(),
            self.y_data.clone(),
            self.interp_distribution.clone(),
        )
    }
}

impl PdfReportable for FluenceData {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        layout.push(genpdf::elements::Paragraph::new(format!(
            "Peak fluence: {:.1} J/cm²",
            self.peak
        )));
        layout.push(genpdf::elements::Paragraph::new(format!(
            "Average fluence: {:.1} J/cm²",
            self.average
        )));
        let img = self.to_plot(Path::new(""), (1000, 1000), PltBackEnd::Buf)?;
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
            .set(&PlotArgs::CBarLabel("fluence in J/cm²".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?
            .set(&PlotArgs::ExpandBounds(false))?;

        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::ColorMesh(plt_params.clone())
    }

    fn get_plot_series(&self, plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>> {
        match plt_type {
            PlotType::ColorMesh(_) => {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: self.x_data.clone(),
                    y_dat_m: self.y_data.clone(),
                    z_dat_nxm: self.interp_distribution.clone(),
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }
}
