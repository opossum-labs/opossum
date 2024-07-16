//! Ray propagation monitor
#![warn(missing_docs)]
use log::warn;
use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{millimeter, nanometer},
};

use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    millimeter,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable, PltBackEnd},
    properties::{Properties, Proptype},
    rays::Rays,
    refractive_index::refr_index_vaccuum,
    reporter::NodeReport,
    surface::Plane,
};
use std::path::{Path, PathBuf};

/// A ray-propagation monitor
///
/// It generates a plot that visualizes the ray path during propagtaion through the scenery.
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
pub struct RayPropagationVisualizer {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for RayPropagationVisualizer {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ray propagation");
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
impl RayPropagationVisualizer {
    /// Creates a new [`RayPropagationVisualizer`].
    /// # Attributes
    /// * `name`: name of the `RayPropagationVisualizer`
    ///
    /// # Panics
    /// This function may panic if the property "name" can not be set.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut rpv = Self::default();
        rpv.node_attr.set_property("name", name.into()).unwrap();
        rpv
    }
}
impl Optical for RayPropagationVisualizer {
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
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", self as &mut dyn Optical);
                }
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
            if let Some(LightData::Geometric(rays)) = &self.light_data {
                let ray_prop_data = rays.get_rays_position_history()?;

                let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                    "ray_propagation_{}_{}.svg",
                    self.name(),
                    uuid
                )));
                ray_prop_data.to_plot(&file_path, PltBackEnd::SVG)?;
                Ok(())
            } else {
                Err(OpossumError::Other(
                    "ray-propagation visualizer: wrong light data".into(),
                ))
            }
        } else {
            Err(OpossumError::Other(
                "ray-propagation visualizer: no light data for export available".into(),
            ))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            if let Ok(proptype) = <Rays as TryInto<Proptype>>::try_into(rays.clone()) {
                props
                    .create(
                        "Ray Propagation visualization plot",
                        "Ray plot",
                        None,
                        proptype,
                    )
                    .unwrap();
                if self.apodization_warning {
                    props
                    .create(
                        "Warning",
                        "warning during analysis",
                        None,
                        "Rays have been apodized at input aperture. Results might not be accurate.".into(),
                    )
                    .unwrap();
                }
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

impl Dottable for RayPropagationVisualizer {
    fn node_color(&self) -> &str {
        "darkgreen"
    }
}

/// struct that holds the history of the rays' positions for rays of a specific wavelength
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RayPositionHistorySpectrum {
    /// Ray history (This is a hack...only used for visualization with bevy...)
    pub history: Vec<MatrixXx3<Length>>,
    center_wavelength: Length,
    wavelength_bin_size: Length,
}
impl RayPositionHistorySpectrum {
    ///creates a new [`RayPositionHistorySpectrum`] struct.
    /// # Attributes
    /// - `history`: position history of the ray bundle
    /// - `center_wavelength`: wavelength of this ray bundle
    /// - `wavelength_bin_size`: wavelength resolution of this ray bundle.
    /// All rays positions in this struct correspond to rays with a wavelength in the bin:
    /// [`center_wavelength` - `wavelength_bin_size/2`; `center_wavelength` + `wavelength_bin_size/2`)
    /// # Errors
    /// This function errors if the provided center wavelength or wavelength bin size is non finite or negative
    pub fn new(
        history: Vec<MatrixXx3<Length>>,
        center_wavelength: Length,
        wavelength_bin_size: Length,
    ) -> OpmResult<Self> {
        if !center_wavelength.is_normal() || center_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "center wavelength must be finite, non-zero and non-negative!".into(),
            ));
        }
        if !wavelength_bin_size.is_normal() || wavelength_bin_size.is_sign_negative() {
            return Err(OpossumError::Other(
                "wavelength bin size must be finite, non-zero and non-negative!".into(),
            ));
        }
        Ok(Self {
            history,
            center_wavelength,
            wavelength_bin_size,
        })
    }

    /// Returns the ray-position history stored in this [`RayPositionHistorySpectrum`] struct.
    #[must_use]
    pub const fn get_history(&self) -> &Vec<MatrixXx3<Length>> {
        &self.history
    }
    /// Returns the center wavelength of the rays whose position histories are stored in this [`RayPositionHistorySpectrum`] struct.
    #[must_use]
    pub const fn get_center_wavelength(&self) -> &Length {
        &self.center_wavelength
    }
    /// Returns the wavelength bin size in which all the rays of this [`RayPositionHistorySpectrum`] struct are inside.
    #[must_use]
    pub const fn get_wavelength_bin_size(&self) -> &Length {
        &self.wavelength_bin_size
    }

    /// Projects a set of 3d vectors onto a plane
    /// # Attributes
    /// `plane_normal_vec`: normal vector of the plane to project onto
    ///
    /// # Errors
    /// This function errors if the length of the plane normal vector is zero
    /// # Returns
    /// This function returns a set of 2d vectors in the defined plane projected to a view that is perpendicular to this plane.
    pub fn project_to_plane(
        &self,
        plane_normal_vec: Vector3<f64>,
    ) -> OpmResult<Vec<MatrixXx2<Length>>> {
        let vec_norm = plane_normal_vec.norm();

        if vec_norm < f64::EPSILON {
            return Err(OpossumError::Other(
                "The plane normal vector must have a non-zero length!".into(),
            ));
        }

        let normed_normal_vec = plane_normal_vec / vec_norm;

        //define an axis on the plane.
        //Do this by projection of one of the main coordinate axes onto that plane
        //Beforehand check, if these axes are not parallel to the normal vec
        let (co_ax_1, co_ax_2) = if plane_normal_vec.cross(&Vector3::x()).norm() < f64::EPSILON {
            //parallel to the x-axis
            (Vector3::z(), Vector3::y())
        } else if plane_normal_vec.cross(&Vector3::y()).norm() < f64::EPSILON {
            (Vector3::z(), Vector3::x())
        } else if plane_normal_vec.cross(&Vector3::z()).norm() < f64::EPSILON {
            (Vector3::x(), Vector3::y())
        } else {
            //arbitrarily project x-axis onto that plane
            let x_vec = Vector3::x();
            let mut proj_x = x_vec - x_vec.dot(&normed_normal_vec) * plane_normal_vec;
            proj_x /= proj_x.norm();

            //second axis defined by cross product of x-axis projection and plane normal, which yields another vector that is perpendicular to both others
            (proj_x, proj_x.cross(&normed_normal_vec))
        };

        let mut rays_pos_projection = Vec::<MatrixXx2<Length>>::with_capacity(self.history.len());
        for ray_pos in &self.history {
            let mut projected_ray_pos = MatrixXx2::<Length>::zeros(ray_pos.column(0).len());
            for (row, pos) in ray_pos.row_iter().enumerate() {
                // let pos_t = Vector3::from_vec(pos.transpose().iter().map(|p| p.get::<millimeter>()).collect::<Vec<f64>>());
                let pos_t = Vector3::from_vec(
                    pos.iter()
                        .map(uom::si::f64::Length::get::<millimeter>)
                        .collect::<Vec<f64>>(),
                );
                let proj_pos = pos_t - pos_t.dot(&normed_normal_vec) * plane_normal_vec;

                projected_ray_pos[(row, 0)] = millimeter!(proj_pos.dot(&co_ax_1));
                projected_ray_pos[(row, 1)] = millimeter!(proj_pos.dot(&co_ax_2));
            }
            rays_pos_projection.push(projected_ray_pos);
        }
        Ok(rays_pos_projection)
    }
}

/// struct that holds the history of the ray positions that is needed for report generation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RayPositionHistories {
    /// vector of ray positions for each raybundle at a specifc sptracl position
    pub rays_pos_history: Vec<RayPositionHistorySpectrum>,
}

impl RayPositionHistories {
    /// returns the center wavelengths of the individual [`RayPositionHistorySpectrum`] structs as a Vector
    #[must_use]
    pub fn get_center_wavelengths(&self) -> Vec<Length> {
        self.rays_pos_history
            .iter()
            .map(|r| *r.get_center_wavelength())
            .collect::<Vec<Length>>()
    }
}
impl Plottable for RayPositionHistories {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("distance in mm (y axis)".into()))?
            .set(&PlotArgs::PlotSize((1400, 800)))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::MultiLine2D(plt_params.clone())
    }

    fn get_plot_series(&self, _plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.rays_pos_history.is_empty() {
            Ok(None)
        } else {
            let num_series = self.rays_pos_history.len();
            let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

            let mut wavelengths = self
                .get_center_wavelengths()
                .iter()
                .map(uom::si::f64::Length::get::<nanometer>)
                .collect::<Vec<f64>>();
            wavelengths.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let color_grad = colorous::TURBO;

            let wvl_range = if num_series == 1 {
                1.
            } else {
                (wavelengths[num_series - 1] - wavelengths[0]) * 2.
            };

            for ray_pos_hist in &self.rays_pos_history {
                let wvl = ray_pos_hist.get_center_wavelength().get::<nanometer>();
                let grad_val = 0.42 + (wvl - wavelengths[0]) / wvl_range;
                let rgbcolor = color_grad.eval_continuous(grad_val);
                let projected_positions = ray_pos_hist.project_to_plane(Vector3::x())?;
                let mut proj_pos_mm =
                    Vec::<MatrixXx2<f64>>::with_capacity(projected_positions.len());
                for ray_pos in &projected_positions {
                    proj_pos_mm.push(MatrixXx2::from_vec(
                        ray_pos
                            .iter()
                            .map(uom::si::f64::Length::get::<millimeter>)
                            .collect::<Vec<f64>>(),
                    ));
                }

                let plt_data = PlotData::MultiDim2 {
                    vec_of_xy_data: proj_pos_mm,
                };
                let series_label = format!("{wvl:.1} nm");
                plt_series.push(PlotSeries::new(
                    &plt_data,
                    RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, 1.),
                    Some(series_label),
                ));
            }

            Ok(Some(plt_series))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType, joule, lightdata::DataEnergy, millimeter, nanometer,
        nodes::test_helper::test_helper::*, position_distributions::Hexapolar, rays::Rays,
        spectrum_helper::create_he_ne_spec,
    };
    use approx::assert_relative_eq;
    use tempfile::NamedTempFile;
    use uom::num_traits::Zero;
    use uom::si::length::millimeter;
    use uom::si::{f64::Length, length::nanometer};
    #[test]
    fn default() {
        let mut node = RayPropagationVisualizer::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "ray propagation");
        assert_eq!(node.node_type(), "ray propagation");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "darkgreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = RayPropagationVisualizer::new("test");
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = RayPropagationVisualizer::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = RayPropagationVisualizer::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<RayPropagationVisualizer>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<RayPropagationVisualizer>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = RayPropagationVisualizer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("out1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = RayPropagationVisualizer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<RayPropagationVisualizer>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = RayPropagationVisualizer::default();
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
        let mut rpv = RayPropagationVisualizer::default();
        assert!(rpv.export_data(Path::new(""), "").is_err());
        rpv.light_data = Some(LightData::Geometric(Rays::default()));
        let path = NamedTempFile::new().unwrap();
        assert!(rpv.export_data(path.path().parent().unwrap(), "").is_err());
        rpv.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        assert!(rpv.export_data(path.path().parent().unwrap(), "").is_ok());
    }
    #[test]
    fn report() {
        let mut fd = RayPropagationVisualizer::default();
        let node_report = fd.report("").unwrap();
        assert_eq!(node_report.detector_type(), "ray propagation");
        assert_eq!(node_report.name(), "ray propagation");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        fd.light_data = Some(LightData::Geometric(Rays::default()));
        let node_report = fd.report("").unwrap();
        assert!(!node_report
            .properties()
            .contains("Ray Propagation visualization plot"));
        fd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(1.), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = fd.report("").unwrap();
        assert!(node_report
            .properties()
            .contains("Ray Propagation visualization plot"));
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 1);
    }
    #[test]
    fn new_ray_pos_hist_spec() {
        let h = vec![
            MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
        ];
        let wb = nanometer!(1.);
        let w = nanometer!(1053.);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_ok());

        let wb = nanometer!(0.);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let wb = nanometer!(-1.);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let wb = nanometer!(f64::NAN);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let wb = nanometer!(f64::INFINITY);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let wb = nanometer!(f64::NEG_INFINITY);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let w = nanometer!(0.);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let w = nanometer!(-1.);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let w = nanometer!(f64::NAN);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let w = nanometer!(f64::INFINITY);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

        let w = nanometer!(f64::NEG_INFINITY);
        assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    }
    #[test]
    fn ray_pos_hist_spec_get_history() {
        let history = vec![
            MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
        ];
        let wavelength_bin_size = nanometer!(1.);
        let wavelength = nanometer!(1053.);
        let pos_hist =
            RayPositionHistorySpectrum::new(history.clone(), wavelength, wavelength_bin_size)
                .unwrap();

        let pos_hist_get = pos_hist.get_history();
        assert_relative_eq!(
            history[0][0].get::<millimeter>(),
            pos_hist_get[0][0].get::<millimeter>()
        );
        assert_relative_eq!(
            history[0][1].get::<millimeter>(),
            pos_hist_get[0][1].get::<millimeter>()
        );
        assert_relative_eq!(
            history[0][2].get::<millimeter>(),
            pos_hist_get[0][2].get::<millimeter>()
        );
        assert_relative_eq!(
            history[1][0].get::<millimeter>(),
            pos_hist_get[1][0].get::<millimeter>()
        );
        assert_relative_eq!(
            history[1][1].get::<millimeter>(),
            pos_hist_get[1][1].get::<millimeter>()
        );
        assert_relative_eq!(
            history[1][2].get::<millimeter>(),
            pos_hist_get[1][2].get::<millimeter>()
        );
        assert_relative_eq!(
            history[2][0].get::<millimeter>(),
            pos_hist_get[2][0].get::<millimeter>()
        );
        assert_relative_eq!(
            history[2][1].get::<millimeter>(),
            pos_hist_get[2][1].get::<millimeter>()
        );
        assert_relative_eq!(
            history[2][2].get::<millimeter>(),
            pos_hist_get[2][2].get::<millimeter>()
        );
    }
    #[test]
    fn ray_pos_hist_spec_get_wavelength() {
        let history = vec![
            MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
        ];
        let wavelength_bin_size = nanometer!(1.);
        let wavelength = nanometer!(1053.);
        let pos_hist = RayPositionHistorySpectrum {
            history,
            center_wavelength: wavelength,
            wavelength_bin_size,
        };

        assert_relative_eq!(
            pos_hist.get_center_wavelength().get::<nanometer>(),
            wavelength.get::<nanometer>()
        )
    }
    #[test]
    fn ray_pos_hist_spec_get_bin_size() {
        let history = vec![
            MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
        ];
        let wavelength_bin_size = nanometer!(1.);
        let pos_hist = RayPositionHistorySpectrum {
            history,
            center_wavelength: nanometer!(1053.),
            wavelength_bin_size,
        };

        assert_relative_eq!(
            pos_hist.get_wavelength_bin_size().get::<nanometer>(),
            wavelength_bin_size.get::<nanometer>()
        )
    }
    #[test]
    fn project_to_plane() {
        let history = vec![
            MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
            MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
        ];

        let pos_hist = RayPositionHistorySpectrum {
            history,
            center_wavelength: nanometer!(1053.),
            wavelength_bin_size: nanometer!(1.),
        };

        let projected_rays = pos_hist.project_to_plane(Vector3::x()).unwrap();
        assert_eq!(projected_rays[0][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[0][(0, 1)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[1][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[1][(0, 1)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[2][(0, 0)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[2][(0, 1)].get::<millimeter>(), 0.);

        let projected_rays = pos_hist.project_to_plane(Vector3::y()).unwrap();
        assert_eq!(projected_rays[0][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[0][(0, 1)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[1][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[1][(0, 1)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[2][(0, 0)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[2][(0, 1)].get::<millimeter>(), 0.);

        let projected_rays = pos_hist.project_to_plane(Vector3::z()).unwrap();
        assert_eq!(projected_rays[0][(0, 0)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[0][(0, 1)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[1][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[1][(0, 1)].get::<millimeter>(), 1.);
        assert_eq!(projected_rays[2][(0, 0)].get::<millimeter>(), 0.);
        assert_eq!(projected_rays[2][(0, 1)].get::<millimeter>(), 0.);
    }
}
