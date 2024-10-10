#![warn(missing_docs)]
use log::warn;
use nalgebra::{DVector, MatrixXx2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{meter, nanometer},
};

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    nanometer,
    optic_node::{Alignable, OpticNode},
    optic_ports::{OpticPorts, PortType},
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::analysis_report::NodeReport,
    surface::{hit_map::HitMap, OpticalSurface, Plane},
    utils::{
        geom_transformation::Isometry,
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
};
use core::f64;
use std::collections::HashMap;

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
///   - `plot_aperture`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpotDiagram {
    light_data: Option<Rays>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
impl Default for SpotDiagram {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spot diagram");
        node_attr
            .create_property(
                "plot_aperture",
                "flag that defines if the aperture is displayed in a plot",
                None,
                false.into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "in1").unwrap();
        ports.add(&PortType::Output, "out1").unwrap();
        node_attr.set_ports(ports);
        Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        }
    }
}
impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
    /// # Attributes
    /// * `name`: name of the spot diagram
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut sd = Self::default();
        sd.node_attr.set_name(name);
        sd
    }
}

impl Alignable for SpotDiagram {}

impl OpticNode for SpotDiagram {
    fn hit_maps(&self) -> HashMap<String, HitMap> {
        let mut map: HashMap<String, HitMap> = HashMap::default();
        map.insert("in1".to_string(), HitMap::default());
        map
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(rays) = data {
            let mut transformed_rays = Rays::default();
            let iso = self.effective_iso().unwrap_or_else(Isometry::identity);
            for ray in rays {
                transformed_rays.add_ray(ray.inverse_transformed_ray(&iso));
            }
            props
                .create("Spot diagram", "2D spot diagram", None, self.clone().into())
                .unwrap();
            if let Some(c) = transformed_rays.energy_weighted_centroid() {
                props
                    .create(
                        "centroid x",
                        "x position of energy-weighted centroid",
                        None,
                        c.x.into(),
                    )
                    .unwrap();

                props
                    .create(
                        "centroid y",
                        "y position of energy-weightedcentroid",
                        None,
                        c.y.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.beam_radius_geo() {
                props
                    .create(
                        "geo beam radius",
                        "geometric beam radius",
                        None,
                        radius.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.energy_weighted_beam_radius_rms() {
                props
                    .create(
                        "rms beam radius",
                        "energy-weighted rms beam radius",
                        None,
                        radius.into(),
                    )
                    .unwrap();
            }
            if self.apodization_warning {
                props
                    .create(
                        "Warning",
                        "warning during analysis",
                        None,
                        "Rays have been apodized at input aperture. Results might not be accurate."
                            .into(),
                    )
                    .unwrap();
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
    fn reset_data(&mut self) {
        self.light_data = None;
    }
}

impl Dottable for SpotDiagram {
    fn node_color(&self) -> &str {
        "darkorange"
    }
}
impl Analyzable for SpotDiagram {}
impl AnalysisGhostFocus for SpotDiagram {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        _config: &GhostFocusConfig,
    ) -> OpmResult<LightRays> {
        let (in_port, out_port) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(bouncing_rays) = incoming_data.get(in_port) else {
            return Ok(LightRays::default());
        };
        let mut rays = bouncing_rays.clone();
        if let Some(iso) = self.effective_iso() {
            let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
            rays.refract_on_surface(&mut plane, None)?;
        } else {
            return Err(OpossumError::Analysis(
                "no location for surface defined. Aborting".into(),
            ));
        }
        // merge all rays
        let mut ray_cache = self
            .light_data
            .clone()
            .map_or_else(Rays::default, |rays| rays);
        ray_cache.merge(&rays);
        self.light_data = Some(ray_cache);

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays.clone());
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for SpotDiagram {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            self.light_data = Some(rays.clone());
        }
        Ok(LightResult::from([(outport.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for SpotDiagram {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
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
                let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
                rays.refract_on_surface(&mut plane, None)?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
            if let Some(aperture) = self.ports().aperture(&PortType::Input, "in1") {
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", self as &mut dyn OpticNode);
                    self.apodization_warning = true;
                }
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            if let Some(old_rays) = &self.light_data {
                let mut rays_tob_merged = old_rays.clone();
                rays_tob_merged.merge(&rays);
                self.light_data = Some(rays_tob_merged.clone());
            } else {
                self.light_data = Some(rays.clone());
            }
            if let Some(aperture) = self.ports().aperture(&PortType::Output, "out1") {
                rays.apodize(aperture)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
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
}

impl From<SpotDiagram> for Proptype {
    fn from(value: SpotDiagram) -> Self {
        Self::SpotDiagram(value)
    }
}
impl Plottable for SpotDiagram {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x position (m)".into()))?
            .set(&PlotArgs::YLabel("y position (m)".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }

    #[allow(clippy::too_many_lines)]
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = &self.light_data;
        match data {
            Some(rays) => {
                let (split_rays_bundles, wavelengths) =
                    rays.split_ray_bundle_by_wavelength(nanometer!(0.2), true)?;
                let num_series = split_rays_bundles.len();
                let use_colorbar = if num_series > 5 {
                    plt_type.set_plot_param(&PlotArgs::CBarLabel("wavelength (nm)".into()))?;
                    plt_type.set_plot_param(&PlotArgs::PlotSize((970, 800)))?;
                    plt_type.set_plot_param(&PlotArgs::ZLim(AxLims::new(
                        wavelengths[0].get::<nanometer>(),
                        wavelengths[num_series - 1].get::<nanometer>(),
                    )))?;
                    true
                } else {
                    false
                };
                let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

                let color_grad = colorous::TURBO;
                let wvl_range = if num_series == 1 {
                    1.
                } else {
                    (wavelengths[num_series - 1] * 2. - wavelengths[0] * 2.).get::<nanometer>()
                };

                //ray plot series
                let mut x_max = f64::NEG_INFINITY;
                let mut y_max = f64::NEG_INFINITY;

                let mut xy_pos_series = Vec::<MatrixXx2<Length>>::with_capacity(num_series);
                for ray_bundle in &split_rays_bundles {
                    let iso = self.effective_iso().unwrap_or_else(Isometry::identity);
                    let xy_pos = ray_bundle.get_xy_rays_pos(true, &iso);
                    x_max = xy_pos
                        .column(0)
                        .iter()
                        .map(uom::si::f64::Length::get::<meter>)
                        .fold(x_max, |arg0, x| if x.abs() > arg0 { x.abs() } else { arg0 });
                    y_max = xy_pos
                        .column(1)
                        .iter()
                        .map(uom::si::f64::Length::get::<meter>)
                        .fold(y_max, |arg0, y| if y.abs() > arg0 { y.abs() } else { arg0 });
                    xy_pos_series.push(xy_pos);
                }

                let min_window = wavelengths[0].get::<meter>() / 2.;
                if x_max < min_window {
                    x_max = min_window;
                }
                if y_max < min_window {
                    y_max = min_window;
                }

                let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
                let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
                let y_prefix = get_prefix_for_base_unit(y_max);
                let x_prefix = get_prefix_for_base_unit(x_max);

                plt_type.set_plot_param(&PlotArgs::YLabel(format!("x position ({y_prefix}m)")))?;
                plt_type.set_plot_param(&PlotArgs::XLabel(format!("y position ({x_prefix}m)")))?;

                for (idx, xy_pos) in xy_pos_series.iter().enumerate() {
                    let grad_val =
                        0.42 + (wavelengths[idx] - wavelengths[0]).get::<nanometer>() / wvl_range;
                    let rgbcolor = color_grad.eval_continuous(grad_val);
                    let x_vals = xy_pos
                        .column(0)
                        .iter()
                        .map(|x| get_unit_value_as_length_with_format_by_exponent(*x, x_exponent))
                        .collect::<Vec<f64>>();
                    let y_vals = xy_pos
                        .column(1)
                        .iter()
                        .map(|y| get_unit_value_as_length_with_format_by_exponent(*y, y_exponent))
                        .collect::<Vec<f64>>();

                    let data = PlotData::Dim2 {
                        xy_data: MatrixXx2::from_columns(&[
                            DVector::from_vec(x_vals),
                            DVector::from_vec(y_vals),
                        ]),
                    };
                    let series_label = if legend && !use_colorbar {
                        Some(format!("{:.1} nm", wavelengths[idx].get::<nanometer>()))
                    } else {
                        None
                    };
                    plt_series.push(PlotSeries::new(
                        &data,
                        RGBAColor(rgbcolor.r, rgbcolor.g, rgbcolor.b, 1.),
                        series_label,
                    ));
                }
                x_max *= f64::powi(10., -x_exponent);
                y_max *= f64::powi(10., -y_exponent);

                plt_type.set_plot_param(&PlotArgs::XLim(AxLims::new(-x_max * 1.1, 1.1 * x_max)))?;
                plt_type.set_plot_param(&PlotArgs::YLim(AxLims::new(-y_max * 1.1, 1.1 * y_max)))?;

                //aperture / shape plot series
                if let Ok(Proptype::Bool(plot_aperture)) = self.properties().get("plot_aperture") {
                    if *plot_aperture {
                        if let Some(aperture) = self.ports().aperture(&PortType::Input, "in1") {
                            let plt_series_opt = aperture.get_plot_series(
                                &mut PlotType::Line2D(PlotParameters::default()),
                                legend,
                            )?;
                            if let Some(aperture_plt_series) = plt_series_opt {
                                plt_series.extend(aperture_plt_series);
                            }
                        }
                    }
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
    use crate::optic_ports::PortType;
    use crate::{
        joule, lightdata::DataEnergy, nodes::test_helper::test_helper::*,
        position_distributions::Hexapolar, rays::Rays, spectrum_helper::create_he_ne_spec,
    };
    use uom::num_traits::Zero;

    #[test]
    fn default() {
        let mut node = SpotDiagram::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "spot diagram");
        assert_eq!(node.node_type(), "spot diagram");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "darkorange");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let spot = SpotDiagram::new("test");
        assert_eq!(spot.name(), "test");
        assert!(spot.light_data.is_none());
    }
    #[test]
    fn ports() {
        let spot = SpotDiagram::default();
        assert_eq!(spot.ports().names(&PortType::Input), vec!["in1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut spot = SpotDiagram::default();
        spot.set_inverted(true).unwrap();
        assert_eq!(spot.ports().names(&PortType::Input), vec!["out1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<SpotDiagram>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<SpotDiagram>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("out1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<SpotDiagram>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = SpotDiagram::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    // #[test]
    // fn export_data() {
    //     testing_logger::setup();
    //     let mut sd = SpotDiagram::default();
    //     assert!(sd.export_data(Path::new(""), "").is_ok());
    //     check_warnings(vec![
    //         "spot diagram: no light data for export available. Cannot create plot!",
    //     ]);
    //     sd.light_data = Some(Rays::default());
    //     let path = NamedTempFile::new().unwrap();
    //     assert!(sd.export_data(path.path().parent().unwrap(), "").is_err());
    //     sd.light_data = Some(
    //         Rays::new_uniform_collimated(
    //             nanometer!(1053.0),
    //             joule!(1.0),
    //             &Hexapolar::new(Length::zero(), 1).unwrap(),
    //         )
    //         .unwrap(),
    //     );
    //     assert!(sd.export_data(path.path().parent().unwrap(), "").is_ok());
    // }
    #[test]
    fn report() {
        let mut sd = SpotDiagram::default();
        let node_report = sd.node_report("").unwrap();
        assert_eq!(node_report.node_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(Rays::default());
        let node_report = sd.node_report("").unwrap();
        assert!(node_report.properties().contains("Spot diagram"));
        sd.light_data = Some(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        );
        let node_report = sd.node_report("").unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);
    }
}
