#![warn(missing_docs)]
use image::{DynamicImage, ImageBuffer, RgbImage};
use nalgebra::DVector;
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

    fn calc_wavefront_statistics(path_length_lambda: &DVector<f64>) -> OpmResult<(f64, f64)> {
        if path_length_lambda.is_empty() {
            Err(OpossumError::Other("Empty wavefront-data vector!".into()))
        } else {
            let max = path_length_lambda.max();
            let min = path_length_lambda.min();
            let ptv = max - min;

            let rms = f64::sqrt(
                path_length_lambda
                    .iter()
                    .map(|l| l.powi(2))
                    .collect::<Vec<f64>>()
                    .iter()
                    .sum::<f64>()
                    / f64::from(i32::try_from(path_length_lambda.len()).unwrap()),
            );
            Ok((rms, ptv))
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
        if self.light_data.is_some() {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!(
                "wavefront_diagram_{}.png",
                self.properties().name()?
            ));
            self.to_plot(&file_path, (1000, 850), PltBackEnd::BMP)
        } else {
            Err(OpossumError::Other(
                "Wavefront diagram: no light data for export available".into(),
            ))
        }
    }
    // fn export_data(&self, report_dir: &Path) -> OpmResult<()> {
    //     if let Some(data) = &self.light_data {
    //         let mut file_path = PathBuf::from(report_dir);
    //         file_path.push(format!(
    //             "wavefront_diagram_{}.svg",
    //             self.properties().name()?
    //         ));
    //         // self.to
    //         // data.export(&file_path)
    //         Ok(())
    //     } else {
    //         Ok(())
    //     }
    // }
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
                .create(
                    "Wavefront diagram",
                    "2D wavefront diagram",
                    None,
                    self.clone().into(),
                )
                .unwrap();
            let wf_data = rays.optical_path_length_at_wvl(1053.);
            if !wf_data.is_empty() {
                let (rms, ptv) = Self::calc_wavefront_statistics(&DVector::from_column_slice(
                    wf_data.column(2).as_slice(),
                ))
                .unwrap();
                props
                    .create(
                        "Wavefront rms in λ",
                        "Wavefront root mean square in units of the wavelength",
                        None,
                        format!("{rms:.2}").into(),
                    )
                    .unwrap();
                props
                    .create(
                        "Wavefront ptv in λ",
                        "Wavefront peak to valley in units of the wavelength",
                        None,
                        format!("{ptv:.2}").into(),
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

impl PdfReportable for WaveFront {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let img = self.to_plot(Path::new(""), (1000, 850), PltBackEnd::Buf)?;
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(
                img.unwrap_or_else(ImageBuffer::default),
            ))
            .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
    }
}

impl From<WaveFront> for Proptype {
    fn from(value: WaveFront) -> Self {
        Self::WaveFront(value)
    }
}

impl Dottable for WaveFront {
    fn node_color(&self) -> &str {
        "lightbrown"
    }
}

impl Plottable for WaveFront {
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

        let (plt_data_opt, plt_type) = if let Some(LightData::Geometric(rays)) = &self.light_data {
            if rays.nr_of_rays() > 10000 {
                let plt_type = PlotType::ColorMesh(plt_params);
                (self.get_plot_data(&plt_type)?, plt_type)
            } else {
                let plt_type = PlotType::ColorTriangulated(plt_params);
                // let plt_type = PlotType::TriangulatedSurface(plt_params);
                (self.get_plot_data(&plt_type)?, plt_type)
            }
        } else {
            (None, PlotType::ColorMesh(plt_params))
        };

        plt_data_opt.map_or(Ok(None), |plt_dat| plt_type.plot(&plt_dat))
    }

    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        let data = &self.light_data;
        match data {
            Some(LightData::Geometric(rays)) => {
                let path_length = rays.optical_path_length_at_wvl(1053.);
                match plt_type {
                    PlotType::ColorMesh(_) => {
                        let binned_data = self.bin_2d_scatter_data(&PlotData::Dim3(path_length));
                        Ok(binned_data)
                    }
                    PlotType::TriangulatedSurface(_) | PlotType::ColorTriangulated(_) => {
                        let triangulated_dat =
                            self.triangulate_plot_data(&PlotData::Dim3(path_length), plt_type);
                        Ok(triangulated_dat)
                    }
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}
