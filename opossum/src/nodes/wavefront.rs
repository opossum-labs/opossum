#![warn(missing_docs)]
//! Wavefront measurment node
use image::RgbImage;
use log::warn;
use nalgebra::{DVector, DVectorSlice, MatrixXx3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nanometer,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    plottable::{
        AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable, PltBackEnd,
    },
    properties::{Properties, Proptype},
    refractive_index::refr_index_vaccuum,
    reporter::NodeReport,
    surface::Plane,
    utils::griddata::{create_linspace_axes, interpolate_3d_scatter_data},
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

/// A struct which holds the necessary data to describe the wavefront as well as some statistical values:
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
    /// Note: RMS calculation is performed from wavefront data - avg. OPD !!! (compatible with ZEMAX)
    fn calc_wavefront_statistics(wf_dat: &DVector<f64>) -> OpmResult<(f64, f64)> {
        if wf_dat.is_empty() {
            Err(OpossumError::Other("Empty wavefront-data vector!".into()))
        } else {
            let max = wf_dat.max();
            let min = wf_dat.min();
            let ptv = max - min;
            #[allow(clippy::cast_precision_loss)]
            let avg = wf_dat.sum() / wf_dat.len() as f64;
            // let avg=0.0;
            let rms = f64::sqrt(
                wf_dat
                    .iter()
                    .map(|l| (l - avg) * (l - avg))
                    .collect::<Vec<f64>>()
                    .iter()
                    .sum::<f64>()
                    / f64::from(i32::try_from(wf_dat.len()).unwrap()),
            );
            Ok((ptv, rms))
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
        if let Some(LightData::Geometric(rays)) = &self.light_data {
            let wf_data_opt = rays
                .get_wavefront_data_in_units_of_wvl(true, nanometer!(1.))
                .ok();

            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!(
                "wavefront_diagram_{}.png",
                self.properties().name()?
            ));
            if let Some(wf_data) = wf_data_opt {
                //todo! for all wavelengths
                Ok(wf_data.wavefront_error_maps[0]
                    .to_plot(&file_path, PltBackEnd::BMP)
                    .unwrap_or_else(|e| {
                        warn!("Could not export plot: {e}",);
                        None
                    }))
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
            let wf_data_opt = rays.get_wavefront_data_in_units_of_wvl(true, nanometer!(1.));

            if wf_data_opt.is_ok()
                && !wf_data_opt
                    .as_ref()
                    .unwrap()
                    .wavefront_error_maps
                    .is_empty()
            {
                let wf_data = wf_data_opt.unwrap();

                props
                .create(
                    "Wavefront Map",
                    "Wavefront error mapwith respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                    None,
                    wf_data.clone().into(),
                )
                .unwrap();

                //todo for all error maps at every wavelength!
                props
                .create(
                    "Wavefront PtV",
                    "Wavefront Peak-to-Valley value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                    None,
                    Proptype::WfLambda(wf_data.wavefront_error_maps[0].ptv, wf_data.wavefront_error_maps[0].wavelength),
                )
                .unwrap();

                //todo for all error maps at every wavelength!
                props
                .create(
                    "Wavefront RMS",
                    "Wavefront root mean square value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                    None,
                    Proptype::WfLambda(wf_data.wavefront_error_maps[0].rms, wf_data.wavefront_error_maps[0].wavelength),
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
impl From<WaveFrontData> for Proptype {
    fn from(value: WaveFrontData) -> Self {
        Self::WaveFrontStats(value)
    }
}

impl Dottable for WaveFront {
    fn node_color(&self) -> &str {
        "goldenrod1"
    }
}

impl Plottable for WaveFrontErrorMap {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x distance in mm".into()))?
            .set(&PlotArgs::YLabel("y distance in mm".into()))?
            .set(&PlotArgs::CBarLabel("wavefront error in λ".into()))?
            .set(&PlotArgs::ExpandBounds(false))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        let mut plt_type = PlotType::ColorMesh(plt_params.clone());

        if let Some(plt_series) = &self.get_plot_series(&plt_type).unwrap_or(None) {
            let ranges = plt_series[0].define_data_based_axes_bounds(false);
            let z_bounds = ranges
                .get_z_bounds()
                .unwrap_or_else(|| AxLims::new(-0.5e-3, 0.5e-3).unwrap());
            if z_bounds.min > -1e-3 && z_bounds.max < 1e-3 {
                _ = plt_type.set_plot_param(&PlotArgs::ZLim(Some(AxLims {
                    min: -1e-3,
                    max: 1e-3,
                })));
            }
        }

        plt_type
    }

    fn get_plot_series(&self, _plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>> {
        let (x_interp, _) =
            create_linspace_axes(DVectorSlice::from(&DVector::from_vec(self.x.clone())), 100.)?;
        let (y_interp, _) =
            create_linspace_axes(DVectorSlice::from(&DVector::from_vec(self.y.clone())), 100.)?;
        let scattered_data = MatrixXx3::from_columns(&[
            DVector::from_vec(self.x.clone()),
            DVector::from_vec(self.y.clone()),
            DVector::from_vec(self.wf_map.clone()),
        ]);
        let (interp_dat, _) = interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)?;

        let plt_data = PlotData::ColorMesh {
            x_dat_n: x_interp,
            y_dat_m: y_interp,
            z_dat_nxm: interp_dat,
        };
        let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
        Ok(Some(vec![plt_series]))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType, analyzer::RayTraceConfig, joule, lightdata::DataEnergy, millimeter,
        nanometer, position_distributions::Hexapolar, ray::Ray, rays::Rays,
        spectrum_helper::create_he_ne_spec,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Point3;
    use tempfile::NamedTempFile;
    use uom::num_traits::Zero;
    use uom::si::f64::Length;
    #[test]
    fn calc_wavefront_statistics() {
        let wvl = nanometer!(1000.);
        let en = joule!(1.);

        let mut rays = Rays::default();
        let ray = Ray::new_collimated(Point3::origin(), wvl, en).unwrap();
        rays.add_ray(ray);
        let mut ray = Ray::new_collimated(Point3::origin(), wvl, en).unwrap();
        ray.propagate_along_z(wvl).unwrap(); // generate a path difference of 1 lambda
        rays.add_ray(ray);
        let wavefront_error = rays.wavefront_error_at_pos_in_units_of_wvl(wvl);
        let wvf_map = WaveFrontErrorMap::new(&wavefront_error, wvl).unwrap();
        assert_eq!(wvf_map.ptv, 1.0);
        assert_abs_diff_eq!(wvf_map.rms, 0.5);
    }

    #[test]
    fn new_empty_wf_error_map() {
        let wf_dat = MatrixXx3::from_vec(Vec::<f64>::new());
        assert!(WaveFrontErrorMap::new(&wf_dat, nanometer!(1000.)).is_err());
    }

    #[test]
    fn calc_wf_stats_empty_wf_error_map() {
        let wf_dat = DVector::from_vec(Vec::<f64>::new());
        assert!(WaveFrontErrorMap::calc_wavefront_statistics(&wf_dat).is_err());
    }

    #[test]
    fn default() {
        let node = WaveFront::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.properties().name().unwrap(), "Wavefront monitor");
        assert_eq!(node.properties().node_type().unwrap(), "Wavefront monitor");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "goldenrod1");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = WaveFront::new("test");
        assert_eq!(meter.properties().name().unwrap(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = WaveFront::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = WaveFront::default();
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        let mut node = WaveFront::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn analyze_ok() {
        let mut node = WaveFront::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        );
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input.clone(), &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = node.analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()));
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
    }
    #[test]
    fn analyze_wrong() {
        let mut node = WaveFront::default();
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
        let mut node = WaveFront::default();
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
        let mut wf = WaveFront::default();
        assert!(wf.export_data(Path::new("")).is_ok());
        assert!(wf.export_data(Path::new("")).unwrap().is_none());
        wf.light_data = Some(LightData::Geometric(Rays::default()));
        let path = NamedTempFile::new().unwrap();
        assert!(wf.export_data(path.path().parent().unwrap()).is_ok());
        assert!(wf
            .export_data(path.path().parent().unwrap())
            .unwrap()
            .is_none());
        wf.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        assert!(wf.export_data(path.path().parent().unwrap()).is_ok());
    }
    #[test]
    fn report() {
        let mut wf = WaveFront::default();
        assert!(wf.report().is_none());
        wf.light_data = Some(LightData::Geometric(Rays::default()));
        assert!(wf.report().is_some());
        wf.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = wf.report().unwrap();
        assert_eq!(node_report.detector_type(), "Wavefront monitor");
        assert_eq!(node_report.name(), "Wavefront monitor");
        assert!(node_report.properties().contains("Wavefront Map"));
        assert!(node_report.properties().contains("Wavefront RMS"));
        assert!(node_report.properties().contains("Wavefront PtV"));
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 3);
    }
}
