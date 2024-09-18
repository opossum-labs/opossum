use crate::{
    analyzers::{energy::AnalysisEnergy, raytrace::AnalysisRayTrace},
    optic_node::OpticNode,
};
use core::fmt::Debug;
use std::fmt::Display;

pub trait Analyzable: OpticNode + AnalysisEnergy + AnalysisRayTrace {}
impl Debug for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
    }
}
impl Display for dyn Analyzable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
    }
}
