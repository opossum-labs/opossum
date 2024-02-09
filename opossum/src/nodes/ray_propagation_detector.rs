#![warn(missing_docs)]
use image::RgbImage;
use serde_derive::{Deserialize, Serialize};

use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::plottable::{Plottable, PltBackEnd};
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
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
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("ray propagation", "ray propagation");
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for RayPropagationVisualizer {
    /// create a spot-diagram monitor.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
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
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
}

impl Optical for RayPropagationVisualizer {
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
            if let Some(LightData::Geometric(rays)) = &self.light_data {
                let ray_prop_data = rays.get_rays_position_history_in_mm();

                let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                    "ray_propagation_{}.svg",
                    self.properties().name()?
                )));
                ray_prop_data.to_plot(&file_path, (800, 800), PltBackEnd::SVG)
                // data.export(&file_path)
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
                    "Ray Propagation visualization plot",
                    "Ray plot",
                    None,
                    rays.clone().into(),
                )
                .unwrap();
        }
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            props,
        ))
    }
}

impl Dottable for RayPropagationVisualizer {
    fn node_color(&self) -> &str {
        "darkgreen"
    }
}
