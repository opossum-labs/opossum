#![warn(missing_docs)]
use image::{DynamicImage, RgbImage, ImageBuffer};
use serde_derive::{Serialize, Deserialize};
use uom::si::length::millimeter;

use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::plottable::{PltBackEnd, PlotParameters, PlotArgs, PlotType, PlotData, Plottable};
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
/// This node creates a wavefront view of an incoming raybundle and can be used as an ideal wavefront-measurement device
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
        if let Some(data) = &self.light_data {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!("wavefront_diagram_{}.png", self.properties().name()?));

            println!("{:?}", file_path);
            self.to_plot(&file_path, (1000,850), PltBackEnd::BMP)

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


impl PdfReportable for WaveFront{
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let img = self.to_plot(Path::new(""), (1000,850), PltBackEnd::Buf)?;
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(img.unwrap_or(ImageBuffer::default())))
                .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
    }
}
// impl PdfReportable for WaveFront{
//     fn pdf_report(&self) -> crate::error::OpmResult<genpdf::elements::LinearLayout> {
//         let mut layout = genpdf::elements::LinearLayout::vertical();
//         let img = self.to_img_buf_plot((800,800)).unwrap();
//         layout.push(
//             genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(img))
//                 .map_err(|e| format!("adding of image failed: {e}"))?,
//         );
//         Ok(layout)
//     }
// }

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

impl Plottable for WaveFront{
    fn to_plot(&self, f_path: &Path, img_size: (u32, u32), backend: PltBackEnd) -> OpmResult<Option<RgbImage>>{
        let mut plt_params = PlotParameters::default();
        match backend{
            PltBackEnd::Buf => plt_params.set(PlotArgs::FigSize(img_size)),
            _ => {
                plt_params.set(PlotArgs::FName(f_path.file_name().unwrap().to_str().unwrap().to_owned()))
                .set(PlotArgs::FDir(f_path.parent().unwrap().to_str().unwrap().to_owned()))
                .set(PlotArgs::FigSize(img_size))
            }
        };
        plt_params.set(PlotArgs::Backend(backend));

        let (plt_data_opt, plt_type) = if let Some(LightData::Geometric(rays)) = &self.light_data{
            if rays.nr_of_rays() > 100{
                let plt_type = PlotType::ColorMesh(plt_params);
                (self.get_plot_data(&plt_type)?, plt_type)
            }
            else{
                let plt_type = PlotType::ColorScatter(plt_params);
                (self.get_plot_data(&plt_type)?, plt_type)
            }
        }
        else{
            (None, PlotType::ColorMesh(plt_params))
        };

        if let Some(plt_dat) = plt_data_opt{
            plt_type.plot(&plt_dat)
        }
        else{
            Ok(None)
        }        
    }

    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        let data = &self.light_data;
        match data{
            Some(LightData::Geometric(rays)) => {
                let path_length = rays.optical_path_length_at_wvl(1053.);
                match plt_type{
                    PlotType::ColorMesh(_) => {
                        let binned_data = self.bin_2d_scatter_data(&PlotData::Dim3(path_length));
                        Ok(binned_data)
                    },
                    PlotType::ColorScatter(_) => {
                        let triangulated_dat = self.triangulate_plot_data(&PlotData::Dim3(path_length));
                        Ok(triangulated_dat)
                    },
                    _ => Ok(None),
                }
            },
            _ => Ok(None),
        }
    }
}
