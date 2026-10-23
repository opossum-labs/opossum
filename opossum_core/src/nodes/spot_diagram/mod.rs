#![warn(missing_docs)]
//! Spot diagram detector node.

pub mod spot_data;
pub use spot_data::SpotDiagramData;

use crate::{
    analyzers::{
        AnalyzerKind,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, PortType, SurfacePorts,
        optic_node::APERTURE_LIMIT_NOTE,
    },
    error::OpmResult,
    geometry::{
        Geometry, SurfaceShape,
        body::{OPTIC_SHAPE, default_square_optic_shape},
    },
    light::{LightData, LightResult, Rays},
    nanometer,
    nodes::NodeRegistration,
    properties::Properties,
    reporting::{
        node_report::{NodeReport, NodeReportResult},
        report_note::{ReportLevel, ReportNote},
    },
    utils::geom_transformation::Isometry,
};
use opm_macros_lib::OpmNode;

inventory::submit! {
    NodeRegistration::new::<SpotDiagram>("spot diagram", "spot diagram detector")
}

/// A spot-diagram monitor
///
/// Generates a 2D spot diagram of an incoming ray bundle.
///
/// ## Optical Ports
///   - Inputs: `in1`
///   - Outputs: `out1`
///
/// ## Properties
///   - `name`
///   - `plot_aperture`
///   - `optic shape`: the window the light is recorded within, a 25 mm square by default; light
///     outside it passes unrecorded
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(OpmNode, Clone, Debug)]
#[opm_node("darkorange")]
pub struct SpotDiagram {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}

impl Default for SpotDiagram {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("spot diagram");
        node_attr
            .create_property(
                "plot aperture",
                "flag that defines if the recording window is outlined in a plot",
                false.into(),
            )
            .expect("Hardcoded property creation must not fail");
        node_attr
            .create_property(
                OPTIC_SHAPE,
                "window the light is recorded within",
                default_square_optic_shape().into(),
            )
            .expect("Hardcoded property creation must not fail");
        let mut sd = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        sd.create_default_surface_properties()
            .expect("Hardcoded property creation must not fail");
        sd.update_surfaces()
            .expect("Updating surfaces on a default spot diagram must not fail");
        sd
    }
}

impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
    ///
    /// # Errors
    /// Returns an error if updating surfaces fails.
    pub fn new(name: &str) -> OpmResult<Self> {
        let mut sd = Self::default();
        sd.node_attr.set_name(name);
        sd.update_surfaces()?;
        Ok(sd)
    }
}

impl OpticNode for SpotDiagram {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::detector(
            SurfaceShape::Plane,
            self.cross_section()?,
        )?))
    }
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }

    fn node_report(&self, uuid: &str, analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        if analyzer == AnalyzerKind::Energy {
            return Ok(NodeReportResult::incompatible_warning(format!(
                "Node '{}' ({}): A spot diagram can only be displayed for a ray tracing or ghostfocus analysis.",
                self.name(),
                self.node_type()
            )));
        }

        let mut props = Properties::default();
        let mut report = if let Some(LightData::Geometric(rays)) = &self.light_data {
            let input_port = self
                .ports()
                .names(&PortType::Input)
                .first()
                .cloned()
                .unwrap_or_else(|| "input_1".to_string());

            let iso = self
                .effective_surface_iso(&input_port)
                .unwrap_or_else(|_| Isometry::identity());

            let mut transformed_rays = Rays::default();
            for ray in rays {
                transformed_rays.add_ray(ray.inverse_transformed_ray(&iso));
            }

            // Create compact wavelength-resolved spot diagram property
            if let Ok(spot_data) = SpotDiagramData::from_rays(rays, &iso, nanometer!(0.2)) {
                props.create("Spot diagram", "2D spot diagram", spot_data.into())?;
            } else {
                props.create("note","spot diagram warning", "Spot diagram could not be calculated. This could happen, if all rays miss the detector or are apodized.".into())?;
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
            NodeReport::new(self.node_type(), self.name(), uuid, Properties::default())
        };
        if self.apodization_warning {
            report.add_note(ReportNote::new(ReportLevel::Warning, APERTURE_LIMIT_NOTE));
        }
        Ok(NodeReportResult::Report(report))
    }

    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
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
        let result = self.unified_analyze_single_surface_node(incoming_data, config, "input_1")?;
        let out_port = &self.ports().names(&PortType::Output)[0];
        if let Some(data) = result.get(out_port) {
            self.light_data = Some(data.clone());
        }
        Ok(result)
    }
}

impl AnalysisGhostFocus for SpotDiagram {}
impl AnalysisRayTrace for SpotDiagram {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::{PortType, node_attr::NodePositioning},
        distributions::position::Hexapolar,
        joule,
        light::{Rays, light_result::LightRays, spectrum_helper::create_he_ne_spec},
        nodes::{NodeGroup, SourcePort, test_helper::helper::*},
        prelude::{AnalyzerType, EnergyDataBuilder, GhostFocusConfig, OpmDocument},
    };
    use uom::{num_traits::Zero, si::f64::Length};

    /// With "plot aperture" set, the plot outlines the window the spot diagram records within.
    ///
    /// TODO: Reenable test
    // #[test]
    // fn the_plot_outlines_the_window() -> OpmResult<()> {
    //     use crate::{
    //         analyzers::RayTraceConfig,
    //         apertures::{CircleShape, OpticShape},
    //         geometry::body::OPTIC_SHAPE,
    //         millimeter,
    //     };
    //     let mut diagram = placed_at_origin::<SpotDiagram>()?;
    //     diagram.set_property("plot aperture", true.into())?;
    //     diagram.set_property(
    //         OPTIC_SHAPE,
    //         OpticShape::from(CircleShape::new(millimeter!(5.0))?).into(),
    //     )?;
    //     let rays = rays_along_z_from(&[millimeter!(0.0, 1.0, -10.0)], nanometer!(1000.0))?;
    //     AnalysisRayTrace::analyze(
    //         &mut diagram,
    //         LightResult::from([("input_1".into(), LightData::Geometric(rays))]),
    //         &RayTraceConfig::default(),
    //     )?;
    //     let series = diagram
    //         .get_plot_series(&mut PlotType::Scatter2D(PlotParameters::default()), false)?
    //         .expect("the spot diagram has plot data");
    //     // One series for the single wavelength, one for the outline.
    //     assert_eq!(series.len(), 2);
    //     Ok(())
    // }
    #[test]
    fn default() {
        let node = SpotDiagram::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "spot diagram");
        assert_eq!(node.node_type(), "spot diagram");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "darkorange");
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
        let mut spot = SpotDiagram {
            light_data: Some(LightData::Geometric(Rays::default())),
            ..Default::default()
        };
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
        let output = output.unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }

    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<SpotDiagram>()
    }
    #[test]
    fn records_inside_and_passes_all() -> OpmResult<()> {
        test_detector_records_inside_and_passes_all::<SpotDiagram>(|diagram| {
            diagram.light_data.as_ref()
        })
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
        let output = output.unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }

    #[test]
    fn analyze_ghostfocus_ok() -> OpmResult<()> {
        let mut node = SpotDiagram::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
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
        let output = output.unwrap();
        assert_eq!(output[0], light_rays);
        Ok(())
    }

    #[test]
    fn report() -> OpmResult<()> {
        let mut sd = SpotDiagram::default();
        let res = sd.node_report("", AnalyzerKind::RayTrace)?;
        let NodeReportResult::Report(node_report) = res else {
            panic!("Node report should be `Report` variant");
        };
        assert_eq!(node_report.node_type(), "spot diagram");
        assert_eq!(node_report.name(), "spot diagram");
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 0);

        sd.light_data = Some(LightData::Geometric(Rays::default()));
        let NodeReportResult::Report(node_report) = sd.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Node report should be `Report` variant");
        };
        assert!(node_report.properties().contains("Spot diagram"));

        sd.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(Length::zero(), 1)?,
        )?));
        let NodeReportResult::Report(node_report) = sd.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Node report should be `Report` variant");
        };
        let node_props = node_report.properties();
        let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
        assert_eq!(nr_of_props, 5);

        sd.set_apodization_warning(true);
        let NodeReportResult::Report(node_report) = sd.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Node report should be `Report` variant");
        };
        let notes = node_report.notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].level, ReportLevel::Warning);
        assert_eq!(notes[0].message, APERTURE_LIMIT_NOTE);
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
        let report = reports.first().expect("No report found");

        assert!(
            report.node_reports().is_empty(),
            "Incompatible detector nodes must not generate a NodeReport"
        );

        let note = report
            .notes()
            .first()
            .expect("Expected top-level warning note in AnalysisReport");

        assert_eq!(note.level, ReportLevel::Warning);
        assert!(
            note.message
                .contains("A spot diagram can only be displayed")
        );
        assert!(note.message.contains("spot diagram"));
        Ok(())
    }
}
