#![warn(missing_docs)]
//! Wavefront measurement node
use log::warn;
use nalgebra::{DVector, DVectorView, MatrixXx2, MatrixXx3};
use opm_macros_lib::OpmNode;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    analyzers::{
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, OpticNode, PortType},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
    nanometer,
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::node_report::NodeReport,
    reporting::plottable::{
        AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable,
    },
    utils::{
        geom_transformation::Isometry,
        griddata::{create_linspace_axes, grid_interpolate_3d_scatter_data},
        to_f64,
    },
};

inventory::submit! {
    NodeRegistration::new::<WaveFront>("wavefront monitor", "wavefront detector")
}

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
#[derive(OpmNode, Serialize, Deserialize, Clone, Debug)]
#[opm_node("goldenrod1")]
pub struct WaveFront {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
unsafe impl Send for WaveFront {}

impl Default for WaveFront {
    /// create a wavefront monitor.
    fn default() -> Self {
        let mut wf = Self {
            light_data: None,
            node_attr: NodeAttr::new("wavefront monitor"),
            apodization_warning: false,
        };
        wf.update_surfaces().unwrap();
        wf
    }
}
impl WaveFront {
    /// Creates a new [`WaveFront`] Monitor with the given `name`.
    /// # Attributes
    /// - `name`: name of the [`WaveFront`] Monitor
    /// # Panics
    /// This function panics if `update_surfaces` fails.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut wf = Self::default();
        wf.node_attr.set_name(name);
        wf.update_surfaces().unwrap();
        wf
    }
}
/// This [`WaveFrontData`] struct holds a vector of wavefront-error maps.
/// The vector of [`WaveFrontErrorMap`] is necessary, e.g., to store the wavefront data for each spectral component of a pulse
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WaveFrontErrorMap {
    wavelength: Length,
    ptv: f64,
    rms: f64,
    x: Vec<f64>,
    y: Vec<f64>,
    wf_map: Vec<f64>,
}

impl Default for WaveFrontErrorMap {
    fn default() -> Self {
        Self {
            wavelength: nanometer!(1054.),
            ptv: 0.0,
            rms: 0.0,
            x: vec![0.0],
            y: vec![0.],
            wf_map: vec![0.0],
        }
    }
}

impl WaveFrontErrorMap {
    /// Creates a new [`WaveFrontErrorMap`]
    /// # Attributes
    /// - `wf_dat`: wavefront data as Matrix with 3 columns and dynamix number of rows. Columns are used as 1:x, 2:y, 3:z
    /// - `wavelength`: wavelength that is used for this `WavefrontErrorMap`
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
            let avg = wf_dat.sum() / to_f64(wf_dat.len());
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
impl OpticNode for WaveFront {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            let iso = self
                .effective_surface_iso("input_1")
                .unwrap_or_else(|_| Isometry::identity());
            let wf_data_opt = rays.get_wavefront_data_in_units_of_wvl(true, false, &iso);

            if let Ok(ref wf_data) = wf_data_opt
                && !wf_data.wavefront_error_maps.is_empty()
            {
                for wf_error_map in &wf_data.wavefront_error_maps {
                    props
                    .create(
                        &format!("Wavefront Map at {:.3} nm", wf_error_map.wavelength.get::<uom::si::length::nanometer>()),
                        "Wavefront error map with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        wf_error_map.clone().into(),
                    )
                    .unwrap();

                    //todo for all error maps at every wavelength!
                    props
                    .create(
                        &format!("Wavefront PtV at {:.3} nm", wf_error_map.wavelength.get::<uom::si::length::nanometer>()),
                        "Wavefront Peak-to-Valley value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.ptv, wf_error_map.wavelength),
                    )
                    .unwrap();

                    //todo for all error maps at every wavelength!
                    props
                    .create(
                        &format!("Wavefront RMS at {:.3} nm", wf_error_map.wavelength.get::<uom::si::length::nanometer>()),
                        "Wavefront root mean square value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.rms, wf_error_map.wavelength),
                    )
                    .unwrap();
                }

                if self.apodization_warning {
                    props
                .create(
                    "Warning",
                    "warning during analysis",
                    "Rays have been apodized at input aperture. Results might not be accurate.".into(),
                )
                .unwrap();
                }
            } else {
                props
                .create(
                    "Warning",
                    "warning during wavefront calculation",
                    "This warning might have been created if the Wavefront monitor was used with zero distance from Source or with multiple wavelengths in a completely paraxial setup.".into(),
                )
                .unwrap();
            }

            Some(NodeReport::new(
                &self.node_type(),
                &self.name(),
                uuid,
                props,
            ))
        } else {
            None
        }
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn reset_data(&mut self) {
        self.light_data = None;
        self.reset_optic_surfaces();
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
    }
}
impl From<WaveFrontErrorMap> for Proptype {
    fn from(value: WaveFrontErrorMap) -> Self {
        Self::WaveFrontData(value)
    }
}
impl AnalysisGhostFocus for WaveFront {}
impl AnalysisEnergy for WaveFront {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let result =
            self.unified_analyze_single_surface_node(incoming_data, config, "input_1", None)?;
        let out_port = &self.ports().names(&PortType::Output)[0];
        if let Some(data) = result.get(out_port) {
            self.light_data = Some(data.clone());
        }
        Ok(result)
    }
}
impl AnalysisRayTrace for WaveFront {}

impl Plottable for WaveFrontErrorMap {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            // 2D Map Case
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("y position in mm".into()))?
                .set(&PlotArgs::CBarLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::ExpandBounds(false))?
                .set(&PlotArgs::AxisEqual(true))?
                .set(&PlotArgs::PlotAutoSize(true))?;
        } else if range_x > eps {
            // 1D Line Cut (X varies)
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        } else if range_y > eps {
            // 1D Line Cut (Y varies)
            plt_params
                .set(&PlotArgs::XLabel("y position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        }
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            // 2D Map Case
            let mut plt_type = PlotType::ColorMesh(plt_params.clone());
            let legend = plt_params.get_legend_flag().unwrap_or(false);

            // Adjust Z-axis bounds for 2D plots if data is nearly flat
            if let Some(plt_series) = &self.get_plot_series(&mut plt_type, legend).unwrap_or(None)
                && !plt_series.is_empty()
            {
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
        } else {
            // 1D Line or Point Case
            PlotType::Line2D(plt_params.clone())
        }
    }
    fn get_plot_series(
        &self,
        _plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        // Case 0: Single Point (0D)
        if range_x <= eps && range_y <= eps {
            warn!("Wavefront data has zero dimension in X and Y. Cannot plot.");
            return Ok(None);
        }

        // Case 1: 1D Line Cut (One dimension is effectively zero)
        if range_x <= eps || range_y <= eps {
            // Select the varying axis and corresponding values
            let mut data: Vec<(f64, f64)> = if range_x > eps {
                // X varies, Y is constant
                self.x
                    .iter()
                    .zip(self.wf_map.iter())
                    .map(|(&x, &z)| (x, z))
                    .collect()
            } else {
                // Y varies, X is constant
                self.y
                    .iter()
                    .zip(self.wf_map.iter())
                    .map(|(&y, &z)| (y, z))
                    .collect()
            };

            // Sort data by the independent axis to ensure correct line plotting
            data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut mat = MatrixXx2::zeros(data.len());
            for (i, (coord, val)) in data.iter().enumerate() {
                mat[(i, 0)] = *coord;
                mat[(i, 1)] = *val;
            }

            let plt_series = PlotSeries::new(
                &PlotData::Dim2 { xy_data: mat },
                RGBAColor(255, 0, 0, 1.),
                None,
            );
            return Ok(Some(vec![plt_series]));
        }

        // Case 2: 2D Map (Original Logic)
        if let (Ok((x_interp, _)), Ok((y_interp, _))) = (
            create_linspace_axes(DVectorView::from(&DVector::from_vec(self.x.clone())), 100),
            create_linspace_axes(DVectorView::from(&DVector::from_vec(self.y.clone())), 100),
        ) {
            let scattered_data = MatrixXx3::from_columns(&[
                DVector::from_vec(self.x.clone()),
                DVector::from_vec(self.y.clone()),
                DVector::from_vec(self.wf_map.clone()),
            ]);
            if let Ok((interp_dat, _)) =
                grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)
            {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: x_interp,
                    y_dat_m: y_interp,
                    z_dat_nxm: interp_dat,
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            } else {
                warn!(
                    "Could not create interpolated wavefront map for plotting! Returning no plot data."
                );
                Ok(None)
            }
        } else {
            warn!("Could not create axes from provided data! Returning no plot data.");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test_wavefront_error_map {
    use super::*;
    use crate::{
        joule,
        light::{Ray, Rays},
        nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Point3;
    #[test]
    fn calc_wavefront_statistics() {
        let wvl = nanometer!(1000.);
        let en = joule!(1.);

        let mut rays = Rays::from(Ray::new_collimated(Point3::origin(), wvl, en).unwrap());
        let mut ray = Ray::new_collimated(Point3::origin(), wvl, en).unwrap();
        ray.propagate(wvl).unwrap();
        rays.add_ray(ray);
        let wavefront_error =
            rays.wavefront_error_at_pos_in_units_of_wvl(wvl, &Isometry::identity());
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
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig, core_optics::PortType, distributions::position::Hexapolar,
        joule, light::Rays, light::spectrum_helper::create_he_ne_spec, millimeter, nanometer,
        nodes::test_helper::test_helper::*, utils::geom_transformation::Isometry,
    };
    #[test]
    fn default() {
        let mut node = WaveFront::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "wavefront monitor");
        assert_eq!(node.node_type(), "wavefront monitor");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "goldenrod1");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let meter = WaveFront::new("test");
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = WaveFront::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = WaveFront::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<WaveFront>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<WaveFront>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = WaveFront::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = WaveFront::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        );
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input.clone(), &EnergyConfig::default());
        assert!(output.is_ok());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default());
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
    }
    #[test]
    fn analyze_apodazation_warning() {
        test_analyze_apodization_warning::<WaveFront>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = WaveFront::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default());
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn report() {
        let mut wf = WaveFront::default();
        assert!(wf.node_report("").is_none());
        wf.light_data = Some(LightData::Geometric(Rays::default()));
        assert!(wf.node_report("").is_some());
        wf.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = wf.node_report("").unwrap();
        assert_eq!(node_report.node_type(), "wavefront monitor");
        assert_eq!(node_report.name(), "wavefront monitor");
        assert!(
            node_report
                .properties()
                .contains("Wavefront Map at 1053.000 nm")
        );
        assert!(
            node_report
                .properties()
                .contains("Wavefront RMS at 1053.000 nm")
        );
        assert!(
            node_report
                .properties()
                .contains("Wavefront PtV at 1053.000 nm")
        );
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 3);
    }
}
