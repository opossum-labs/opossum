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

/// An spot diagram monitor
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
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
pub struct SpotDiagram {
    light_data: Option<LightData>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("spot diagram", "spot diagram");
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for SpotDiagram {
    /// create an ideal spectrometer.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
        }
    }
}
impl SpotDiagram {
    /// Creates a new [`SpotDiagram`].
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
impl Optical for SpotDiagram {
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
            file_path.push(format!("spot_diagram_{}.svg", self.properties().name()?));
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
                .create("Spot diagram", "2D spot diagram", None, rays.clone().into())
                .unwrap();
            props
                .create(
                    "centroid x (in mm)",
                    "x position of centroid",
                    None,
                    rays.centroid().x.get::<millimeter>().into(),
                )
                .unwrap();
            props
                .create(
                    "centroid y (in mm)",
                    "y position of centroid",
                    None,
                    rays.centroid().y.get::<millimeter>().into(),
                )
                .unwrap();
            props
                .create(
                    "geometric beam radius (in mm)",
                    "geometric beam radius",
                    None,
                    rays.beam_radius_geo().get::<millimeter>().into(),
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

impl Dottable for SpotDiagram {
    fn node_color(&self) -> &str {
        "darkorange"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spec};
    #[test]
    fn default() {
        let node = SpotDiagram::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.properties().name().unwrap(), "spot diagram");
        assert_eq!(node.properties().node_type().unwrap(), "spot diagram");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "darkorange");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = SpotDiagram::new("test");
        assert_eq!(meter.properties().name().unwrap(), "test");
        assert!(meter.light_data.is_none());
    }
    #[test]
    fn ports() {
        let meter = SpotDiagram::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = SpotDiagram::default();
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        let mut node = SpotDiagram::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn analyze_ok() {
        let mut node = SpotDiagram::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = SpotDiagram::default();
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
        let mut node = SpotDiagram::default();
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
}
