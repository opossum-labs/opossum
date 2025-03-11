//! Marker trait for an optical node that can be analyzed
#![warn(missing_docs)]
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    optic_node::OpticNode,
};
use core::fmt::Debug;
use std::fmt::Display;

/// Marker trait for an optical node that can be analyzed
pub trait Analyzable:
    OpticNode + AnalysisEnergy + AnalysisRayTrace + AnalysisGhostFocus + Send
{
}
impl Debug for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}
impl Display for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
    }
}

#[cfg(test)]
mod test {
    use crate::{analyzers::Analyzable, nodes::Dummy};
    #[test]
    fn fmt() {
        assert_eq!(
            format!("{}", &Dummy::new("test") as &dyn Analyzable),
            "'test' (dummy)"
        );
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!("{:?}", &Dummy::new("test") as &dyn Analyzable),
            "'test' (dummy)"
        );
    }
}
