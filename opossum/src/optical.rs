#![warn(missing_docs)]
//! Contains the basic trait representing an optical element
use crate::analyzer::AnalyzerType;
use crate::aperture::Aperture;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::nodes::{NodeGroup, NodeReference};
use crate::optic_ports::OpticPorts;
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
use core::fmt::Debug;
use std::collections::HashMap;
use std::path::Path;

/// A [`LightResult`] represents the [`LightData`], which arrives at a given (`OpticPort`)[OpticPorts] of an optical node.
///
/// The given (`OpticPort`)[OpticPorts] might also be `None`, which indicates, that no light has already "flown" to an inpurt port of a node.
pub type LightResult = HashMap<String, Option<LightData>>;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait Optical: Dottable {
    /// Returns a reference to the name of this [`Optical`].
    // fn name(&self) -> &str; // {
    //                         //    self.node_type()
    //                         //}
    // /// Return the type of the optical component (lens, filter, ...).
    // fn node_type(&self) -> &str;
    // /// Return the available (input & output) ports of this [`Optical`].
    fn ports(&self) -> OpticPorts {
        if let Proptype::OpticPorts(ports) = self.properties().get("apertures").unwrap() {
            let mut ports = ports.clone();
            if self.properties().get_bool("inverted").unwrap() {
                ports.set_inverted(true);
            }
            ports
        } else {
            panic!("property <apertures> not found")
        }
    }
    /// Set an [`Aperture`] for a given input port name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    fn set_input_aperture(&mut self, port_name: &str, aperture: Aperture) -> OpmResult<()> {
        let mut ports = self.ports();
        if ports.inputs().contains_key(port_name) {
            ports.set_input_aperture(port_name, aperture)?;
            self.set_property("apertures", ports.into())?;
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port name <{port_name}> does not exist"
            )))
        }
    }
    /// Set an [`Aperture`] for a given output port name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    fn set_output_aperture(&mut self, port_name: &str, aperture: Aperture) -> OpmResult<()> {
        let mut ports = self.ports();
        if ports.outputs().contains_key(port_name) {
            ports.set_output_aperture(port_name, aperture)?;
            self.set_property("apertures", ports.into())?;
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "output port name <{port_name}> does not exist"
            )))
        }
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
        print!(
            "{}: No analyze function defined.",
            self.properties().node_type()?
        );
        Ok(LightResult::default())
    }
    /// Export analysis data to file(s) within the given directory path.
    ///
    /// This function should be overridden by a node in order to export node-specific data into a file.
    /// The default implementation does nothing.
    ///
    /// # Errors
    /// This function might return an error depending on the particular implemention.
    fn export_data(&self, _report_dir: &Path) -> OpmResult<()> {
        Ok(())
    }
    /// Returns `true` if the [`Optical`] represents a detector which can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Returns `true` if this [`Optical`] is inverted. The default implementation returns `false`.
    // fn inverted(&self) -> bool {
    //     false
    // }
    /// Return a downcasted reference of a [`NodeGroup`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`Optical`] does not have the `node_type` property "group".
    fn as_group(&self) -> OpmResult<&NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
    }
    /// This function is called right after a node has been deserialized (e.g. read from a file). By default, this
    /// function does nothing and returns no error.
    ///
    /// Currently thsi function is needed for group nodes whose internal graph structure must be synchronized with the
    /// graph stored in theirs properties
    ///
    /// # Errors
    ///
    /// This function will return an error if the overwritten function generates an error.
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        Ok(())
    }
    /// Return a downcasted mutable reference of a [`NodeReference`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`Optical`] does not have the `node_type` property "reference".
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
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()>;
    /// Set all properties of this [`Optical`].
    ///
    /// This is a convenience function. It internally calls [`set_property`](Optical::set_property) for all given properties. **Note**: Properties, which are not
    /// present for the [`Optical`] are silently igrnored.
    ///
    /// # Errors
    ///
    /// This function will return an error if the Property conditions while setting a value are not met.
    fn set_properties(&mut self, properties: Properties) -> OpmResult<()> {
        let own_properties = self.properties().clone();
        for prop in properties.iter() {
            if own_properties.contains(prop.0) {
                match prop.0.as_str() {
                    "node_type" => {}
                    "apertures" => {
                        let mut ports = self.ports();
                        if let Proptype::OpticPorts(ports_to_be_set) = prop.1.prop().clone() {
                            ports.set_apertures(ports_to_be_set)?;
                            self.set_property("apertures", ports.into())?;
                        }
                    }
                    _ => self.set_property(prop.0, prop.1.prop().clone())?,
                };
            }
        }
        Ok(())
    }
    /// Return a JSON representation of the current state of this [`Optical`].
    ///
    /// This function must be overridden for generating output in the analysis report. Mainly detector nodes use this feature.
    /// The default implementation is to return a JSON `null` value.
    fn report(&self) -> Option<NodeReport> {
        None
    }
}

impl Debug for dyn Optical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({})",
            self.properties().name().unwrap(),
            self.properties().node_type().unwrap()
        )
    }
}
