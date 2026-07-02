use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use crate::{
    core_optics::NodeAttr,
    error::OpmResult,
    geometry::{Plane, geo_surface::GeoSurfaceRef},
    nodes::NodeRegistration,
    prelude::{Isometry, OpticNode, PortType, Proptype},
};
use opm_macros_lib::OpmNode;

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

#[derive(OpmNode, Clone)]
#[opm_node("slateblue")]
/// A source port node marks the position of a light source.
///
/// A source port is a node that marks the logical position of a light source. It is used to define the position
/// and orientation of a light source in the scene. Note, that the source port does not contain any information about the light source itself.
/// During analysis it looks up its data in a source map given by the current analyzer. This allows for a decoupling of the actual light data
/// from its position and orientation in the scene. This way the same source port can be used for different light sources in different analyses,
/// e.g. for ray tracing and energy analysis.
pub struct SourcePort {
    node_attr: NodeAttr,
}

inventory::submit! {
    NodeRegistration::new::<SourcePort>("source port", "light source port")
}

impl Default for SourcePort {
    fn default() -> Self {
        let node_attr = NodeAttr::new("source port");

        let mut src = Self { node_attr };
        src.set_isometry(Isometry::identity()).unwrap();
        src.update_surfaces().unwrap();
        src
    }
}

impl SourcePort {
    /// Creates a new source port with the specified name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut node = Self::default();
        node.node_attr.set_name(name);
        node
    }
}

impl Display for SourcePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' (source port)", self.node_attr.name())
    }
}

impl OpticNode for SourcePort {
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        // A source port only has an output port, so we only need to update the flat single surface for the output port.
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));
        self.update_surface(
            &"output_1".to_string(),
            geosurface,
            Isometry::identity(),
            &PortType::Output,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{Analyzer, raytrace::RayTracingAnalyzer},
        core_optics::node_attr::HasNodeAttr,
        joule, millimeter,
        nodes::{
            NodeGroup, ParaxialSurface, RayPropagationVisualizer, round_collimated_ray_builder,
        },
        prelude::{PortType, RayTraceConfig},
    };

    #[test]
    fn default() {
        let mut node = SourcePort::default();
        assert_eq!(node.name(), "source port");
        assert_eq!(node.node_type(), "source port");
        assert_eq!(node.isometry(), Some(Isometry::identity()));
        assert_eq!(node.node_attr().inverted(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let source = SourcePort::new("test");
        assert_eq!(source.name(), "test");
    }
    #[test]
    fn is_invertable() {
        let mut node = SourcePort::default();
        assert!(node.set_inverted(false).is_ok());
        assert!(node.set_inverted(true).is_ok());
    }
    #[test]
    fn ports() {
        let node = SourcePort::default();
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn integration_test() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let i_src = scenery.add_node(SourcePort::default())?;
        let i_l = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
        let i_sd = scenery.add_node(RayPropagationVisualizer::default())?;
        scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(i_l, "output_1", i_sd, "input_1", millimeter!(150.0))?;

        let ray_data_builder = round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 10)?;
        let mut ray_trace_config = RayTraceConfig::default();
        ray_trace_config.map_source(i_src, ray_data_builder);
        let analyzer = RayTracingAnalyzer::new(ray_trace_config);
        assert!(analyzer.analyze(&mut scenery).is_ok());
        Ok(())
    }
}
