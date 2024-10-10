//! Marker trait for an optical node that can be analyzed
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    optic_node::OpticNode,
};
use core::fmt::Debug;
use std::fmt::Display;

/// Marker trait for an optical node that can be analyzed
pub trait Analyzable: OpticNode + AnalysisEnergy + AnalysisRayTrace + AnalysisGhostFocus {}
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
