use std::fmt::Display;

use crate::{
    error::OpmResult,
    nodes::{NodeAttr, NodeRegistration},
    prelude::{Isometry, OpticNode, Proptype},
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
unsafe impl Send for SourcePort {}

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
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::prelude::PortType;

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
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
}
