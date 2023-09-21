//! This module contains the concrete node types (lenses, filters, etc...)
mod beam_splitter;
mod detector;
mod dummy;
mod energy_meter;
mod group;
mod ideal_filter;
mod lens;
mod reference;
mod source;
mod spectrometer;

use std::cell::RefCell;
use std::rc::Rc;

pub use beam_splitter::BeamSplitter;
pub use detector::Detector;
pub use dummy::Dummy;
pub use group::NodeGroup;
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::{IdealLens, RealLens};
pub use reference::NodeReference;
pub use source::Source;

pub use energy_meter::EnergyMeter;
pub use energy_meter::Metertype;

pub use spectrometer::Spectrometer;
pub use spectrometer::SpectrometerType;

use crate::error::OpossumError;
use crate::optical::OpticRef;

pub fn create_node_ref(node_type: &str) -> Result<OpticRef,OpossumError> {
  match node_type {
    "dummy" => Ok(OpticRef(Rc::new(RefCell::new(Dummy::default())))),
    "detector" => Ok(OpticRef(Rc::new(RefCell::new(Detector::default())))),
    _ => Err(OpossumError::Other(format!("cannot create node type {}", node_type)))
  }
}