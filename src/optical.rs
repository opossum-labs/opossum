use serde_json::json;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::nodes::{NodeGroup, NodeReference};
use crate::optic_ports::OpticPorts;
use crate::properties::{Properties, Property};
use core::fmt::Debug;
use std::collections::HashMap;
use std::path::Path;

pub type LightResult = HashMap<String, Option<LightData>>;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait Optical: Dottable {
    /// Returns a reference to the name of this [`Optical`].
    fn name(&self) -> &str; // {
                            //    self.node_type()
                            //}
    /// Return the type of the optical component (lens, filter, ...).
    fn node_type(&self) -> &str;
    /// Return the available (input & output) ports of this [`Optical`].
    fn ports(&self) -> OpticPorts {
        OpticPorts::default()
    }
    /// Perform an analysis of this element. The type of analysis is given by an [`AnalyzerType`].
    ///
    /// This function is normally only called by [`OpticScenery::analyze()`](crate::optic_scenery::OpticScenery::analyze()).
    ///
    /// # Errors
    ///
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        print!("{}: No analyze function defined.", self.node_type());
        Ok(LightResult::default())
    }
    /// Export analysis data to file(s) within the given directory path.
    ///
    /// This function should be overridden by a node in order to export node-specific data into a file.
    /// The default implementation does nothing.
    fn export_data(&self, _report_dir: &Path) {}
    /// Returns `true` if the [`Optical`] represents a detector which can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Returns `true` if this [`Optical`] is inverted.
    fn inverted(&self) -> bool {
        false
    }
    fn as_group(&self) -> OpmResult<&NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
    }
    fn as_refnode_mut(&mut self) -> OpmResult<&mut NodeReference> {
        Err(OpossumError::Other("cannot cast to reference node".into())) 
    }
    /// Return the properties of this [`Optical`].
    ///
    /// Return all properties of an optical node. Note, that some properties might be read-only.
    fn properties(&self) -> &Properties;
    /// Set a property of this [`Optical`].
    ///
    /// Set a property of an optical node. This property must already exist (e.g. defined in new() / default() functions of the node).
    ///
    /// # Errors
    ///
    /// This function will return an error if a non-defined property is set or the property has the wrong data type.
    fn set_property(&mut self, name: &str, property: Property) -> OpmResult<()>;
    fn set_properties(&mut self, properties: &Properties) -> OpmResult<()> {
        let own_properties = self.properties().props.clone();

        for prop in properties.props.iter() {
            if own_properties.contains_key(prop.0) {
                self.set_property(prop.0, prop.1.clone())?;
            }
        }
        Ok(())
    }
    /// Return a JSON representation of the current state of this [`Optical`].
    ///
    /// This function must be overridden for generating output in the analysis report. Mainly detector nodes use this feature.
    /// The default implementation is to return a JSON `null` value.
    fn report(&self) -> serde_json::Value {
        json!(null)
    }
}

impl Debug for dyn Optical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.node_type())
    }
}
