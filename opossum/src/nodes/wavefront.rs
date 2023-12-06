#![warn(missing_docs)]
use uom::si::length::millimeter;

use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::lightdata::LightData;
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
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
pub struct WaveFront {
    light_data: Option<LightData>,
    props: Properties,
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
    fn export_data(&self, report_dir: &Path) -> OpmResult<()> {
        if let Some(data) = &self.light_data {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!("wavefront_diagram_{}.svg", self.properties().name()?));
            data.export(&file_path)
        } else {
            Ok(())
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
                .create("Wavefront diagram", "2D wavefront diagram", None, rays.clone().into())
                .unwrap();
            let wf_data = rays.wavefront_at_wvl(1053.);
            
            if let Some(c) = rays.centroid() {
                props
                    .create(
                        "centroid x (mm)",
                        "x position of centroid",
                        None,
                        c.x.get::<millimeter>().into(),
                    )
                    .unwrap();

                props
                    .create(
                        "centroid y (mm)",
                        "y position of centroid",
                        None,
                        c.y.get::<millimeter>().into(),
                    )
                    .unwrap();
            }
            if let Some(radius) = rays.beam_radius_geo() {
                props
                    .create(
                        "geo beam radius (mm)",
                        "geometric beam radius",
                        None,
                        radius.get::<millimeter>().into(),
                    )
                    .unwrap();
            }
        }
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            props,
        ))
    }
}

impl Dottable for WaveFront {
    fn node_color(&self) -> &str {
        "lightbrown"
    }
}