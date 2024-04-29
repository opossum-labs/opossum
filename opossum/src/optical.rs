#![warn(missing_docs)]
//! Contains the basic trait representing an optical element
use bevy::math::primitives::Cuboid;
use bevy::math::Vec3;
use bevy::render::mesh::Mesh;
use image::RgbImage;
use log::warn;

use crate::analyzer::AnalyzerType;
use crate::aperture::Aperture;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::nodes::{NodeAttr, NodeGroup, NodeReference};
use crate::optic_ports::OpticPorts;
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
use crate::utils::geom_transformation::Isometry;
use core::fmt::Debug;
use std::collections::HashMap;
use std::path::Path;

/// A [`LightResult`] represents the [`LightData`], which arrives at a given (`OpticPort`)[`OpticPorts`] of an optical node.
///
pub type LightResult = HashMap<String, LightData>;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait Optical: Dottable + Send {
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
    fn set_input_aperture(&mut self, port_name: &str, aperture: &Aperture) -> OpmResult<()> {
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
    fn set_output_aperture(&mut self, port_name: &str, aperture: &Aperture) -> OpmResult<()> {
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
        warn!("{}: No analyze function defined.", self.node_type());
        Ok(LightResult::default())
    }
    /// Export analysis data to file(s) within the given directory path.
    ///
    /// This function should be overridden by a node in order to export node-specific data into a file.
    /// The default implementation does nothing.
    ///
    /// # Errors
    /// This function might return an error depending on the particular implementation.
    fn export_data(&self, _report_dir: &Path) -> OpmResult<Option<RgbImage>> {
        Ok(None)
    }
    /// Returns `true` if the [`Optical`] represents a detector that can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Returns `true` if the [`Optical`] represents a detector that can report analysis data.
    fn is_source(&self) -> bool {
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
    /// Currently this function is needed for group nodes whose internal graph structure must be synchronized with the
    /// graph stored in their properties
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

    /// Set a property of this [`Optical`].
    ///
    /// Set a property of an optical node. This property must already exist (e.g. defined in `new()` / `default()` functions of the node).
    ///
    /// # Errors
    ///
    /// This function will return an error if a non-defined property is set or the property has the wrong data type.
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()>;
    /// Set all properties of this [`Optical`].
    ///
    /// This is a convenience function. It internally calls [`set_property`](Optical::set_property) for all given properties. **Note**: Properties, which are not
    /// present for the [`Optical`] are silently ignored.
    ///
    /// # Errors
    ///
    /// This function will return an error if the Property conditions while setting a value are not met.
    fn set_properties(&mut self, properties: Properties) -> OpmResult<()> {
        let own_properties = self.properties().clone();
        for prop in &properties {
            if own_properties.contains(prop.0) {
                match prop.0.as_str() {
                    "node_type" => {}
                    "apertures" => {
                        let mut ports = self.ports();
                        if let Proptype::OpticPorts(ports_to_be_set) = prop.1.prop().clone() {
                            if self.node_type() == "group" {
                                // apertures cannot be set here for groups since no port mapping is defined yet.
                                // this will be done later dynamically in group:ports() function.
                                self.set_property("apertures", ports_to_be_set.into())?;
                            } else {
                                ports.set_apertures(ports_to_be_set)?;
                                self.set_property("apertures", ports.into())?;
                            }
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
    /// Get the [`NodeAttr`] (common attributes) of an [`Optical`].
    fn node_attr(&self) -> &NodeAttr;
    /// Get the node type of this [`Optical`]
    fn node_type(&self) -> String {
        self.node_attr().node_type()
    }
    /// Get the name of this [`Optical`]
    fn name(&self) -> String {
        self.node_attr().name()
    }
    /// Return the properties of this [`Optical`].
    ///
    /// Return all properties of an optical node.
    fn properties(&self) -> &Properties {
        self.node_attr().properties()
    }
    /// Return the [`Isometry`] of this optical node.
    fn isometry(&self) -> &Option<Isometry> {
        self.node_attr().isometry()
    }
    /// Set the [`Isometry`] (position and angle) of this optical node.
    fn set_isometry(&mut self, isometry: Isometry);
    ///
    fn mesh(&self) -> Mesh {
       let mesh: Mesh=Cuboid::new(0.5, 0.5, 0.005).into();
       if let Some(iso)=self.isometry() {
        let t=iso.translation();
        mesh.translated_by(Vec3::new(t.x.value as f32, t.y.value as f32,t.z.value as f32))
       } else {
        warn!("Node has no isometry defined. Mesh will be located at origin.");
        mesh
       }
      
       
    }
}

impl Debug for dyn Optical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.node_type())
    }
}
