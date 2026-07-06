#![warn(missing_docs)]
use crate::{
    analyzers::{
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, PortType, optic_surface::OpticSurface,
    },
    error::OpmResult,
    light::{LightData, LightResult, Rays},
    nanometer,
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::{
        node_report::NodeReport,
        plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
        report_note::{ReportLevel, ReportNote},
    },
    utils::{
        geom_transformation::Isometry,
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
};
use core::f64;
use log::warn;
use nalgebra::{DVector, MatrixXx2};
use opm_macros_lib::OpmNode;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{meter, nanometer},
};

inventory::submit! {
    NodeRegistration::new::<SpotDiagram>("spot diagram", "spot diagram detector")
}

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

impl Default for SpotDiagram {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spot diagram");
        node_attr
            .create_property(
                "plot aperture",
                "flag that defines if the aperture is displayed in a plot",
                false.into(),
            )
            .expect("Hardcoded property creation must not fail");
        let mut sd = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        sd.update_surfaces()
            .expect("Updating surfaces on a default spot diagram must not fail");
        sd
    }
}
impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
    /// # Attributes
    /// - `name`: name of the spot diagram
    ///
    /// # Errors
    ///
    /// This function returns an error if internally `update_surfaces` fails.
    pub fn new(name: &str) -> OpmResult<Self> {
        let mut sd = Self::default();
        sd.node_attr.set_name(name);
        sd.update_surfaces()?;
        Ok(sd)
    }
}
impl OpticNode for SpotDiagram {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn node_report(&self, uuid: &str) -> OpmResult<Option<NodeReport>> {
        let mut props = Properties::default();
        let data = &self.light_data;
        let mut report = if let Some(LightData::Geometric(rays)) = data {
            let mut transformed_rays = Rays::default();
            let iso = self
                .effective_surface_iso("input_1")
                .unwrap_or_else(|_| Isometry::identity());
            for ray in rays {
                transformed_rays.add_ray(ray.inverse_transformed_ray(&iso));
            }
            if let Some(hit_map) = self.get_optic_surface("input_1").map(OpticSurface::hit_map) {
                props.create("Spot diagram", "2D spot diagram", hit_map.clone().into())?;
            }
            if let Some(c) = transformed_rays.energy_weighted_centroid() {
                props.create(
                    "centroid x",
                    "x position of energy-weighted centroid",
                    c.x.into(),
                )?;

                props.create(
                    "centroid y",
                    "y position of energy-weighted centroid",
                    c.y.into(),
                )?;
            }
            if let Some(radius) = transformed_rays.beam_radius_geo() {
                props.create("geo beam radius", "geometric beam radius", radius.into())?;
            }
            if let Some(radius) = transformed_rays.energy_weighted_beam_radius_rms() {
                props.create(
                    "rms beam radius",
                    "energy-weighted rms beam radius",
                    radius.into(),
                )?;
            }
            NodeReport::new(self.node_type(), self.name(), uuid, props)
        } else {
            let mut report =
                NodeReport::new(self.node_type(), self.name(), uuid, Properties::default());
            report.add_note(ReportNote::new(
                ReportLevel::Warning,
                "A spot diagram can only be displayed for a ray tracing or ghostfocus analysis.",
            ));
            report
        };
        if self.apodization_warning {
            report.add_note(ReportNote::new(
                ReportLevel::Warning,
                "Rays have been apodized at input aperture. Results might not be accurate.",
            ));
        }
        Ok(Some(report))
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn set_light_data(&mut self, ld: Option<LightData>) {
        self.light_data = ld;
    }
}
impl AnalysisEnergy for SpotDiagram {
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
impl AnalysisGhostFocus for SpotDiagram {}
impl AnalysisRayTrace for SpotDiagram {}

impl Plottable for SpotDiagram {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("x position (m)".into()))?
            .set(&PlotArgs::YLabel("y position (m)".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?;
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
                if let Ok(Proptype::Bool(plot_aperture)) = self.properties().get("plot_aperture")
                    && *plot_aperture
                    && let Some(aperture) = self.ports().aperture(&PortType::Input, "input_1")
                {
                    let plt_series_opt = aperture.get_plot_series(
                        &mut PlotType::Line2D(PlotParameters::default()),
                        legend,
                    )?;
                    if let Some(aperture_plt_series) = plt_series_opt {
                        plt_series.extend(aperture_plt_series);
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
    use crate::{
        core_optics::{PortType, node_attr::HasNodeAttr},
        distributions::position::Hexapolar,
        joule,
        light::{Rays, light_result::LightRays, spectrum_helper::create_he_ne_spec},
        nodes::{NodeGroup, SourcePort, test_helper::test_helper::*},
        prelude::{AnalyzerType, EnergyDataBuilder, GhostFocusConfig, OpmDocument},
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
    fn new() -> OpmResult<()> {
        let spot = SpotDiagram::new("test")?;
        assert_eq!(spot.name(), "test");
        assert!(spot.light_data.is_none());
        Ok(())
    }
    #[test]
    fn ports() {
        let spot = SpotDiagram::default();
        assert_eq!(spot.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut spot = SpotDiagram::default();
        spot.set_inverted(true)?;
        assert_eq!(spot.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(spot.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
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
    fn analyze_energy_empty() -> OpmResult<()> {
        test_analyze_empty::<SpotDiagram>()
    }
    #[test]
    fn analyze_energy_wrong() -> OpmResult<()> {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_energy_ok() -> OpmResult<()> {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }
    #[test]
    fn analyze_apodization_warning() -> OpmResult<()> {
        test_analyze_apodization_warning::<SpotDiagram>()
    }
    #[test]
    fn analyze_energy_inverse() -> OpmResult<()> {
        let mut node = SpotDiagram::default();
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }
    #[test]
    fn analyze_ghostfocus_ok() -> OpmResult<()> {
        let mut node = SpotDiagram::default();
        node.set_isometry(Isometry::identity())?;
        let mut input = LightRays::default();
        let light_rays = Rays::default();
        input.insert("input_1".into(), vec![light_rays.clone()]);
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut vec![],
            0,
        )?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output[0], light_rays);
        Ok(())
    }
    #[test]
    fn report() -> OpmResult<()> {
        let mut sd = SpotDiagram::default();
        let Some(node_report) = sd.node_report("")? else {
            panic!("Node report should not be `None`");
        };
        assert_eq!(node_report.node_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);
        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let Some(node_report) = sd.node_report("")? else {
            panic!("Node report should not be `None`");
        };
        assert!(node_report.properties().contains("Spot diagram"));
        sd.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(Length::zero(), 1)?,
        )?));
        let Some(node_report) = sd.node_report("")? else {
            panic!("Node report should not be `None`");
        };
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);

        sd.set_apodization_warning(true);
        let Some(node_report) = sd.node_report("")? else {
            panic!("Node report should not be `None`");
        };
        let notes = node_report.notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].level, ReportLevel::Warning);
        assert!(notes[0].message.contains("apodized"));
        Ok(())
    }
    #[test]
    fn test_aperture_on_hitmap() -> OpmResult<()> {
        use crate::{
            analyzers::raytrace::{AnalysisRayTrace, RayTraceConfig},
            apertures::{Aperture, ApertureType},
            light::{LightData, LightResult, Ray, Rays},
            prelude::*,
        };
        use nalgebra::{Point3, Vector3};

        let mut sd = SpotDiagram::default();
        let aperture = Aperture::new_rectangle(
            millimeter!(1.0),
            millimeter!(1.0),
            ApertureType::Hole,
            None,
            None,
        )?;

        sd.set_aperture(&PortType::Input, "input_1", &aperture)?;
        sd.node_attr_mut().set_isometry(Isometry::identity());

        let mut rays = Rays::from(Ray::new(
            Point3::new(millimeter!(0.0), millimeter!(0.0), millimeter!(-1.0)),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(550.0),
            joule!(1.0),
        )?);

        // ray outside aperture
        rays.add_ray(Ray::new(
            Point3::new(millimeter!(0.0), millimeter!(2.0), millimeter!(-1.0)),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(550.0),
            joule!(1.0),
        )?);

        let analyzer = RayTraceConfig::default();
        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".to_string(), LightData::Geometric(rays));

        AnalysisRayTrace::analyze(&mut sd, incoming_data, &analyzer)?;

        let Some(node_report) = sd.node_report("")? else {
            panic!("Node report should not be `None`");
        };
        let node_props = node_report.properties(); // Returns &Properties

        // Use iterator correctly as (key, value)
        let hit_map_prop = node_props
            .iter()
            .find(|(name, _p)| *name == "Spot diagram")
            .unwrap();

        if let Proptype::HitMap(hm) = hit_map_prop.1.prop() {
            // Should verify that we only have 1 hit point (the one inside the aperture)
            // The one outside should be pruned/not present
            let merged = hm.get_merged_rays_hit_map()?;
            let points = merged.hit_map().positions();
            assert_eq!(
                points.len(),
                1,
                "HitMap should only contain rays passing through aperture"
            );
            assert!(
                points[0].y.abs().value < 1.0,
                "Remaining point should be inside the aperture"
            );
        } else {
            panic!("Property is not a HitMap");
        }
        Ok(())
    }
    #[test]
    fn show_warning_if_energy_analysis() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let i_src = scenery.add_node(SourcePort::default())?;
        let i_sd = scenery.add_node(SpotDiagram::default())?;
        scenery.connect_nodes(i_src, "output_1", i_sd, "input_1", Length::zero())?;
        let mut doc = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(i_src, EnergyDataBuilder::default());
        doc.add_analyzer(AnalyzerType::Energy(config));
        let reports = doc.analyze()?;
        let Some(report) = reports.first() else {
            panic!("No report found");
        };
        let Some(node_report) = report.node_reports().first() else {
            panic!("No node report found.")
        };
        let Some(note) = node_report.notes().first() else {
            panic!("SpotDiagram has no note in its report");
        };
        assert_eq!(note.level, ReportLevel::Warning);
        assert_eq!(
            note.message,
            "A spot diagram can only be displayed for a ray tracing or ghostfocus analysis."
        );
        Ok(())
    }
}
