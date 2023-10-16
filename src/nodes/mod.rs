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
pub use group::PortMap;
pub use ideal_filter::{FilterType, IdealFilter};
pub use lens::{IdealLens, RealLens};
pub use reference::NodeReference;
pub use source::Source;

pub use energy_meter::EnergyMeter;
pub use energy_meter::Metertype;

pub use spectrometer::Spectrometer;
pub use spectrometer::SpectrometerType;
use uuid::Uuid;

use crate::error::OpmResult;
use crate::error::OpossumError;

use crate::optic_ref::OpticRef;

pub fn create_node_ref(node_type: &str, uuid: Option<Uuid>) -> OpmResult<OpticRef> {
    match node_type {
        "dummy" => Ok(OpticRef::new(Rc::new(RefCell::new(Dummy::default())), uuid)),
        "detector" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Detector::default())),
            uuid,
        )),
        "beam splitter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(BeamSplitter::default())),
            uuid,
        )),
        "energy meter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(EnergyMeter::default())),
            uuid,
        )),
        "group" => Ok(OpticRef::new(
            Rc::new(RefCell::new(NodeGroup::default())),
            uuid,
        )),
        "ideal filter" => Ok(OpticRef::new(
            Rc::new(RefCell::new(IdealFilter::default())),
            uuid,
        )),
        "reference" => Ok(OpticRef::new(
            Rc::new(RefCell::new(NodeReference::default())),
            uuid,
        )),
        "real lens" => Ok(OpticRef::new(
            Rc::new(RefCell::new(RealLens::default())),
            uuid,
        )),
        "light source" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Source::default())),
            uuid,
        )),
        "spectrometer" => Ok(OpticRef::new(
            Rc::new(RefCell::new(Spectrometer::default())),
            uuid,
        )),
        _ => Err(OpossumError::Other(format!(
            "cannot create node type <{}>",
            node_type
        ))),
    }
}
