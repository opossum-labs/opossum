#![warn(missing_docs)]
use log::warn;
use nalgebra::{DVector, MatrixXx2};
use opm_macros_lib::OpmNode;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{meter, nanometer},
};

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    error::OpmResult,
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    nanometer,
    optic_node::OpticNode,
    optic_ports::PortType,
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::node_report::NodeReport,
    utils::{
        geom_transformation::Isometry,
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
};
use core::f64;

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
#[derive(OpmNode, Serialize, Deserialize, Clone, Debug)]
#[opm_node("darkorange")]
pub struct SpotDiagram {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
unsafe impl Send for SpotDiagram {}

impl Default for SpotDiagram {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spot diagram");
        node_attr
            .create_property(
                "plot_aperture",
                "flag that defines if the aperture is displayed in a plot",
                false.into(),
            )
            .unwrap();
        let mut sd = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        sd.update_surfaces().unwrap();
        sd
    }
}
impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
    /// # Attributes
    /// - `name`: name of the spot diagram
    /// # Panics    
    /// This function panics if `update_surfaces` fails.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut sd = Self::default();
        sd.node_attr.set_name(name);
        sd.update_surfaces().unwrap();
        sd
    }
}
impl OpticNode for SpotDiagram {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            let mut transformed_rays = Rays::default();
            let iso = self
                .effective_surface_iso("input_1")
                .unwrap_or_else(|_| Isometry::identity());
            for ray in rays {
                transformed_rays.add_ray(ray.inverse_transformed_ray(&iso));
            }
            props
                .create("Spot diagram", "2D spot diagram", self.clone().into())
                .unwrap();
            if let Some(c) = transformed_rays.energy_weighted_centroid() {
                props
                    .create(
                        "centroid x",
                        "x position of energy-weighted centroid",
                        c.x.into(),
                    )
                    .unwrap();

                props
                    .create(
                        "centroid y",
                        "y position of energy-weightedcentroid",
                        c.y.into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.beam_radius_geo() {
                props
                    .create("geo beam radius", "geometric beam radius", radius.into())
                    .unwrap();
            }
            if let Some(radius) = transformed_rays.energy_weighted_beam_radius_rms() {
                props
                    .create(
                        "rms beam radius",
                        "energy-weighted rms beam radius",
                        radius.into(),
                    )
                    .unwrap();
            }
            if self.apodization_warning {
                props
                    .create(
                        "Warning",
                        "warning during analysis",
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
        self.reset_optic_surfaces();
    }

    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
}
impl AnalysisEnergy for SpotDiagram {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(_) = data {
            self.light_data = Some(data.clone());
        }
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisGhostFocus for SpotDiagram {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        AnalysisGhostFocus::analyze_single_surface_node(self, incoming_data, config)
    }
}
impl AnalysisRayTrace for SpotDiagram {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze_single_surface_node(self, incoming_data, config)
    }

    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
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
            Some(LightData::Geometric(rays)) => {
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
                    let iso = self.effective_surface_iso("input_1")?;
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
                x_max = x_max.max(min_window);
                y_max = y_max.max(min_window);

                let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
                let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
                let y_prefix = get_prefix_for_base_unit(y_max);
                let x_prefix = get_prefix_for_base_unit(x_max);

                plt_type.set_plot_param(&PlotArgs::YLabel(format!("y in {y_prefix}m")))?;
                plt_type.set_plot_param(&PlotArgs::XLabel(format!("x in {x_prefix}m")))?;

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
                        if let Some(aperture) = self.ports().aperture(&PortType::Input, "input_1") {
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
        joule, nodes::test_helper::test_helper::*, position_distributions::Hexapolar, rays::Rays,
        spectrum_helper::create_he_ne_spec,
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
        assert!(node.as_group_mut().is_err());
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
        assert_eq!(spot.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut spot = SpotDiagram::default();
        spot.set_inverted(true).unwrap();
        assert_eq!(spot.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<SpotDiagram>()
    }
    #[test]
    fn reset_data() {
        let mut spot = SpotDiagram::default();
        spot.light_data = Some(LightData::Geometric(Rays::default()));
        spot.reset_data();
        assert!(spot.light_data.is_none());
    }
    #[test]
    fn analyze_energy_empty() {
        test_analyze_empty::<SpotDiagram>()
    }
    #[test]
    fn analyze_energy_wrong() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<SpotDiagram>()
    }
    #[test]
    fn analyze_energy_inverse() {
        let mut node = SpotDiagram::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_ghostfocus_ok() {
        let mut node = SpotDiagram::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let mut input = LightRays::default();
        let light_rays = Rays::default();
        input.insert("input_1".into(), vec![light_rays.clone()]);
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut vec![],
            0,
        )
        .unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output[0], light_rays);
    }
    #[test]
    fn report() {
        let mut sd = SpotDiagram::default();
        let node_report = sd.node_report("").unwrap();
        assert_eq!(node_report.node_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let node_report = sd.node_report("").unwrap();
        assert!(node_report.properties().contains("Spot diagram"));
        sd.light_data = Some(LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1053.0),
                joule!(1.0),
                &Hexapolar::new(Length::zero(), 1).unwrap(),
            )
            .unwrap(),
        ));
        let node_report = sd.node_report("").unwrap();
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);
    }
}
