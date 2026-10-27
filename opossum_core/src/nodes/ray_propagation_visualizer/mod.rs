//! Ray propagation monitor node.
#![warn(missing_docs)]

pub mod history;
#[cfg(test)]
mod tests;

pub use history::{RayPositionHistories, RayPositionHistorySpectrum};

use nalgebra::Vector3;
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};

use crate::{
    analyzers::{
        AnalyzerKind,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{Appearance, NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, SurfacePorts},
    error::OpmResult,
    geometry::Geometry,
    light::{LightData, LightResult, Spectrum},
    nodes::NodeRegistration,
    properties::{Properties, Proptype, validator::Validator},
    reporting::{
        node_report::{NodeReport, NodeReportResult},
        report_note::{ReportLevel, ReportNote},
    },
};

inventory::submit! {
    NodeRegistration::new::<RayPropagationVisualizer>("ray propagation", "ray propagation plotter")
}

/// A ray-propagation monitor.
///
/// It generates a plot that visualizes the ray path during propagation through the scenery.
///
/// ## Optical Ports
///   - Inputs: `input_1`
///   - Outputs: `output_1`
///
/// ## Properties
///   - `view_direction`: Normal vector defining the projection plane (default: x-axis -> y-z plane).
///   - `ray transparency`: Alpha value in [0.0, 1.0] for ray rendering.
#[derive(OpmNode, Serialize, Deserialize, Clone, Debug)]
#[opm_node("darkgreen")]
pub struct RayPropagationVisualizer {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}

impl Default for RayPropagationVisualizer {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ray propagation");
        node_attr
            .create_property(
                "view_direction",
                "plane to project the ray positions onto, defined by the normal vector. default: y-z plane",
                Proptype::Vec3(Vector3::x()),
            )
            .unwrap();

        node_attr
            .create_property_with_validator(
                "ray transparency",
                "transparency (alpha) value of the ray colors to be plotted",
                Validator::NumericInRange { min: 0., max: 1. },
                0.4.into(),
            )
            .unwrap();

        let mut rpv = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        rpv.create_default_surface_properties().unwrap();
        rpv.update_surfaces().unwrap();
        rpv
    }
}

impl RayPropagationVisualizer {
    /// Creates a new [`RayPropagationVisualizer`].
    ///
    /// # Arguments
    /// * `name` - Name of the node.
    /// * `view_normal_vector` - Optional normal vector of the projection plane.
    ///
    /// # Errors
    /// Returns an error if the `view_direction` property cannot be set.
    pub fn new(name: &str, view_normal_vector: Option<Vector3<f64>>) -> OpmResult<Self> {
        let mut rpv = Self::default();
        rpv.node_attr.set_name(name);
        if let Some(view_vec) = view_normal_vector {
            rpv.node_attr
                .set_property("view_direction", Proptype::Vec3(view_vec))?;
        } else {
            rpv.node_attr
                .set_property("view_direction", Proptype::Vec3(Vector3::x()))?;
        }
        Ok(rpv)
    }
}

impl OpticNode for RayPropagationVisualizer {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(None)))
    }
    /// The visualizer records the rays passing it; it is not drawn.
    fn appearance(&self) -> Option<Appearance> {
        None
    }
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }

    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
    }

    fn node_report(&self, uuid: &str, analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        if analyzer == AnalyzerKind::Energy {
            return Ok(NodeReportResult::incompatible_warning(format!(
                "Node '{}' ({}): A propagation plot can only be calculated during a ray tracing or ghostfocus analysis.",
                self.name(),
                self.node_type()
            )));
        }

        let mut props = Properties::default();
        let mut report =
            NodeReport::new(self.node_type(), self.name(), uuid, Properties::default());

        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data
            && let Ok(mut ray_position_histories) = rays.get_rays_position_history(true)
        {
            if let Ok(Proptype::Vec3(view_vec)) = self.properties().get("view_direction") {
                ray_position_histories.plot_view_direction = Some(*view_vec);
            }
            if let Ok(Proptype::F64(transparency)) = self.properties().get("ray transparency") {
                ray_position_histories.ray_transparency = *transparency;
            }
            props.create(
                "Ray plot",
                "Ray plot",
                Proptype::RayPositionHistory(ray_position_histories),
            )?;

            report = NodeReport::new(self.node_type(), self.name(), uuid, props);

            if self.apodization_warning {
                report.add_note(ReportNote::new(
                    ReportLevel::Warning,
                    "Rays were lost while passing this node. Results might not be accurate.",
                ));
            }
        }

        Ok(NodeReportResult::Report(report))
    }

    fn set_light_data(&mut self, new_data: Option<LightData>) {
        self.light_data = new_data;
    }
}

impl AnalysisGhostFocus for RayPropagationVisualizer {}

impl AnalysisEnergy for RayPropagationVisualizer {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let result = self.unified_analyze_single_surface_node(incoming_data, config, "input_1")?;
        // Energy analysis is not supported for propagation visualization; triggers report warning
        self.light_data = Some(LightData::Energy(Spectrum::default()));
        Ok(result)
    }
}

impl AnalysisRayTrace for RayPropagationVisualizer {}
