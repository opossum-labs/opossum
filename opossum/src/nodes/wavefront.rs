#![warn(missing_docs)]
use image::{DynamicImage, ImageBuffer, RgbImage};
use log::warn;
use nalgebra::{DVector, MatrixXx3};
use serde_derive::{Deserialize, Serialize};
use uom::si::f64::Length;
use uom::si::length::nanometer;

use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::plottable::{
    AxLims, PlotArgs, PlotData, PlotParameters, PlotType, Plottable, PltBackEnd,
};
use crate::properties::{Properties, Proptype};
use crate::reporter::{NodeReport, PdfReportable};
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A wavefront monitor node
///
/// This node creates a wavefront view of an incoming ray bundle and can be used as an ideal wavefront-measurement device
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
pub struct WaveFront {
    light_data: Option<LightData>,
    props: Properties,
}

impl WaveFront {
    /// Creates a new [`WaveFront`] Monitou.
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

/// This [`WaveFrontData`] struct holds a vector of wavefront-error maps.
/// The vector of [`WaveFrontErrorMap`] is necessary, e.g., to store the wavefront data for each spectral component of a pulse
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveFrontData {
    /// vector of [`WaveFrontErrorMap`]. May contain only a single [`WaveFrontErrorMap`] if only calculated for a single wavelength
    pub wavefront_error_maps: Vec<WaveFrontErrorMap>,
}

/// This [`WaveFrontErrorMap`] struct holds the necessary data to describe the wavefront as well as some statistical values:
/// - `wavelength`: the wavelength that was used to calculate this wavefront map in units of a specific wavelength
/// - `ptv`: the peak-to-valley value of the wavefront map in units of milli-lambda
/// - `rms`: the root-mean-square value of the wavefront map in units of milli-lambda
/// - `x`: the x axis of the wavefront map
/// - `y`: the y axis of the wavefront map
/// - `wf_map`: the wavefront map
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveFrontErrorMap {
    wavelength: Length,
    ptv: f64,
    rms: f64,
    x: Vec<f64>,
    y: Vec<f64>,
    wf_map: Vec<f64>,
}

impl WaveFrontErrorMap {
    /// Creates a new [`WaveFrontErrorMap`]
    /// # Attributes
    /// - `wf_dat`: wavefront data as Matrix with 3 columns and dynamix number of rows. Columns are used as 1:x, 2:y, 3:z
    /// - `wavelength`: wave length that is used for this `WavefrontErrorMap`
    ///
    /// # Returns
    /// This method returns a [`WaveFrontErrorMap`] struct
    ///
    /// # Errors
    /// This method will return an error if the wavefront data is empty or if `calc_wavefront_statistics()` fails.
    pub fn new(wf_dat: &MatrixXx3<f64>, wavelength: Length) -> OpmResult<Self> {
        if wf_dat.is_empty() {
            Err(OpossumError::Other("Empty wavefront-data vector!".into()))
        } else {
            let len_wf_dat = wf_dat.len();
            let mut x = Vec::<f64>::with_capacity(len_wf_dat);
            let mut y = Vec::<f64>::with_capacity(len_wf_dat);
            let mut wf_map = Vec::<f64>::with_capacity(len_wf_dat);
            for row in wf_dat.row_iter() {
                x.push(row[0]);
                y.push(row[1]);
                wf_map.push(row[2]);
            }
            let (ptv, rms) = Self::calc_wavefront_statistics(&DVector::from_vec(wf_map.clone()))?;
            Ok(Self {
                wavelength,
                ptv,
                rms,
                x,
                y,
                wf_map,
            })
        }
    }
    fn calc_wavefront_statistics(wf_dat: &DVector<f64>) -> OpmResult<(f64, f64)> {
        if wf_dat.is_empty() {
            Err(OpossumError::Other("Empty wavefront-data vector!".into()))
        } else {
            let max = wf_dat.max();
            let min = wf_dat.min();
            let ptv = max - min;

            let rms = f64::sqrt(
                wf_dat
                    .iter()
                    .map(|l| l.powi(2))
                    .collect::<Vec<f64>>()
                    .iter()
                    .sum::<f64>()
                    / f64::from(i32::try_from(wf_dat.len()).unwrap()),
            );
            Ok((rms * 1000., ptv * 1000.))
        }
    }
}

fn create_default_props() -> Properties {
    let mut props = Properties::new("Wavefront monitor", "Wavefront monitor");
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for WaveFront {
    /// create a wavefront monitor.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
        }
    }
}

impl Optical for WaveFront {
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
            let wf_data_opt =
                rays.get_wavefront_data_in_units_of_wvl(true, Length::new::<nanometer>(1.))?;

            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!(
                "wavefront_diagram_{}.png",
                self.properties().name()?
            ));
            if let Some(wf_data) = wf_data_opt {
                //todo! for all wavelengths
                wf_data.wavefront_error_maps[0].to_plot(&file_path, (1000, 850), PltBackEnd::BMP)
            } else {
                warn!("Wavefront diagram: no wavefront data for export available",);
                Ok(None)
            }
        } else {
            warn!("Wavefront diagram: no light data for export available",);
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
            let wf_data_opt =
                rays.get_wavefront_data_in_units_of_wvl(true, Length::new::<nanometer>(1.));

            if wf_data_opt.is_ok() && wf_data_opt.as_ref().unwrap().is_some() {
                let wf_data = wf_data_opt.unwrap().unwrap();

                props
                .create(
                    "Wavefront Data",
                    "Wavefront error in λ, rms in mλ and ptv in mλ with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                    None,
                    wf_data.into(),
                )
                .unwrap();
            }

            Some(NodeReport::new(
                self.properties().node_type().unwrap(),
                self.properties().name().unwrap(),
                props,
            ))
        } else {
            None
        }
    }
}

impl PdfReportable for WaveFrontData {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();

        layout.push(genpdf::elements::Paragraph::new(format!(
            "ptv: {:.1} mλ",
            self.wavefront_error_maps[0].ptv
        )));
        layout.push(genpdf::elements::Paragraph::new(format!(
            "rms: {:.1} mλ",
            self.wavefront_error_maps[0].rms
        )));
        //todo! for all wavefronts!
        let img =
            self.wavefront_error_maps[0].to_plot(Path::new(""), (1000, 850), PltBackEnd::Buf)?;
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(
                img.unwrap_or_else(ImageBuffer::default),
            ))
            .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
    }
}

impl From<WaveFrontData> for Proptype {
    fn from(value: WaveFrontData) -> Self {
        Self::WaveFrontStats(value)
    }
}

impl Dottable for WaveFront {
    fn node_color(&self) -> &str {
        "lightbrown"
    }
}

impl Plottable for WaveFrontErrorMap {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x distance in mm".into()))?
            .set(&PlotArgs::YLabel("y distance in mm".into()))?
            .set(&PlotArgs::CBarLabel("wavefront error in λ".into()))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        let mut plt_type = if self.x.is_empty() || self.x.len() > 10000 {
            PlotType::ColorMesh(plt_params.clone())
        } else {
            PlotType::ColorTriangulated(plt_params.clone())
        };

        if let Some(plt_data) = &self.get_plot_data(&plt_type).unwrap_or(None) {
            let ranges = plt_data.get_axes_min_max_ranges();
            if ranges[2].min > -1e-3 && ranges[2].max < 1e-3 {
                _ = plt_type.set_plot_param(&PlotArgs::ZLim(Some(AxLims {
                    min: -1e-3,
                    max: 1e-3,
                })));
            }
        }

        plt_type
    }

    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        let plt_data = PlotData::Dim3(MatrixXx3::from_columns(&[
            DVector::from_vec(self.x.clone()),
            DVector::from_vec(self.y.clone()),
            DVector::from_vec(self.wf_map.clone()),
        ]));
        Ok(self.bin_or_triangulate_data(plt_type, &plt_data))
    }
}
