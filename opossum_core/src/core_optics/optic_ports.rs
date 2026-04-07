#![warn(missing_docs)]
//! Handling of input and output ports of optical elements.
//!
//! The optical ports represent an interface of an optical element. The ports define the way how nodes can be connected to each other.
//! For example, a simple filter contains one input and one output port. Each port has a unique name, an [`Aperture`] (set to
//! [`Aperture::None`] by default), and a [`CoatingType`] ([`CoatingType::IdealAR`] by default). Furthermore, [`OpticPorts`] can be
//! inverted (see inverted optic nodes). In this case input and output ports are swapped.
//! ```rust
//! use opossum_core::prelude::*;
//! use opossum_core::core_optics::OpticPorts;
//!
//! let mut ports = OpticPorts::new();
//! ports.add(&PortType::Input, "my input").unwrap();
//! let aperture = Aperture::new_circle(millimeter!(1.5), millimeter!(1.0, 1.0), ApertureType::Hole).unwrap();
//! ports.set_aperture(&PortType::Input, "my input", &aperture).unwrap();
//! ```
use crate::{
    J_per_cm2,
    apertures::Aperture,
    coatings::CoatingType,
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllPositive},
    nodes::fluence_detector::Fluence,
    validated, validated_type,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
use uom::si::radiant_exposure::joule_per_square_centimeter;

/// Helper function to provide the default LIDT value for Serde deserialization.
/// We need this because Serde's `#[serde(default)]` attribute requires a function path
/// when dealing with custom types that don't implement the standard `Default` trait
/// exactly how we need it here (with the validation macro).
fn default_lidt() -> validated_type!(Fluence, AllPositive && AllFinite) {
    validated!(J_per_cm2!(1.), AllPositive && AllFinite).unwrap()
}

/// Configuration of an optical port containing user-adjustable parameters.
///
/// This struct is purely for configuration (State) and is serialized.
/// It does NOT contain geometric runtime data like `GeoSurface` or `HitMap`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    /// The aperture of the port, defining the spatial transmission.
    #[serde(default)]
    pub aperture: Aperture,
    /// The coating of the port, defining reflection and transmission properties.
    #[serde(default)]
    pub coating: CoatingType,
    /// The Laser Induced Damage Threshold (LIDT) specific to this port.
    #[serde(default = "default_lidt")]
    pub lidt: validated_type!(Fluence, AllPositive && AllFinite),
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            aperture: Aperture::None,
            coating: CoatingType::IdealAR,
            lidt: default_lidt(),
        }
    }
}

impl Display for PortConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "aperture: {:?}, coating: {:?}, lidt: {:?} J/cm^2",
            self.aperture,
            self.coating,
            self.lidt.get().get::<joule_per_square_centimeter>()
        )
    }
}
/// Type of an [`OpticPorts`]
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum PortType {
    /// input port, receiving [`LightData`](crate::lightdata::LightData)
    Input,
    /// ouput port, sending [`LightData`](crate::lightdata::LightData)
    Output,
}
/// Structure defining the optical ports (input / output terminals) and their configuration of an [`OpticNode`](crate::core_optics::OpticNode).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct OpticPorts {
    inputs: BTreeMap<String, PortConfig>,
    outputs: BTreeMap<String, PortConfig>,
    #[serde(skip)]
    inverted: bool,
}

impl OpticPorts {
    /// Creates a new (empty) [`OpticPorts`] structure.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new input / output port with the given name.
    ///
    /// The port is initialized with a default [`PortConfig`].
    ///
    /// # Errors
    /// This function will return an error if the input port name already exists.
    pub fn add(&mut self, port_type: &PortType, name: &str) -> OpmResult<()> {
        let table = match port_type {
            PortType::Input => &mut self.inputs,
            PortType::Output => &mut self.outputs,
        };
        if table.insert(name.into(), PortConfig::default()).is_none() {
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "port with name {name} already exists",
            )))
        }
    }

    /// Returns a reference to the input / output port configurations of this [`OpticPorts`].
    #[must_use]
    pub const fn ports(&self, port_type: &PortType) -> &BTreeMap<String, PortConfig> {
        let (mut input_ports, mut output_ports) = (&self.inputs, &self.outputs);
        if self.inverted {
            (input_ports, output_ports) = (output_ports, input_ports);
        }
        match port_type {
            PortType::Input => input_ports,
            PortType::Output => output_ports,
        }
    }

    /// Returns a mutable reference to the input / output port configurations of this [`OpticPorts`].
    #[must_use]
    pub const fn ports_mut(&mut self, port_type: &PortType) -> &mut BTreeMap<String, PortConfig> {
        // We cannot use const fn here easily with mutable borrowing and swapping,
        // so we resolve it normally.
        if self.inverted {
            match port_type {
                PortType::Input => &mut self.outputs,
                PortType::Output => &mut self.inputs,
            }
        } else {
            match port_type {
                PortType::Input => &mut self.inputs,
                PortType::Output => &mut self.outputs,
            }
        }
    }

    /// Returns the input / output port names of this [`OpticPorts`].
    #[must_use]
    pub fn names(&self, port_type: &PortType) -> Vec<String> {
        self.ports(port_type).keys().cloned().collect()
    }

    /// Sets the aperture of a port with the given name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    pub fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()> {
        let ports = self.ports_mut(port_type);
        ports.get_mut(port_name).map_or_else(
            || {
                Err(OpossumError::OpticPort(format!(
                    "port name <{port_name}> does not exist",
                )))
            },
            |config| {
                config.aperture = aperture.clone();
                Ok(())
            },
        )
    }

    /// Sets the coating of a port with the given name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    pub fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()> {
        let ports = self.ports_mut(port_type);
        ports.get_mut(port_name).map_or_else(
            || {
                Err(OpossumError::OpticPort(format!(
                    "port <{port_name}> does not exist",
                )))
            },
            |config| {
                config.coating = coating.clone();
                Ok(())
            },
        )
    }

    /// Sets the LIDT of a port with the given name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist or the LIDT is invalid.
    pub fn set_lidt(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        lidt: Fluence,
    ) -> OpmResult<()> {
        let ports = self.ports_mut(port_type);
        ports.get_mut(port_name).map_or_else(
            || {
                Err(OpossumError::OpticPort(format!(
                    "port <{port_name}> does not exist",
                )))
            },
            |config| {
                config.lidt.set(lidt)?;
                Ok(())
            },
        )
    }
    /// Sets the (input & ouput port) apertures of this [`OpticPorts`] from another [`OpticPorts`].
    ///
    /// # Errors
    /// This function will return an error if the port name of the set ports does not exist in this [`OpticPorts`].
    pub fn set_apertures(&mut self, set_ports: Self) -> OpmResult<()> {
        for (name, config) in set_ports.inputs {
            self.set_aperture(&PortType::Input, &name, &config.aperture)?;
        }
        for (name, config) in set_ports.outputs {
            self.set_aperture(&PortType::Output, &name, &config.aperture)?;
        }
        Ok(())
    }

    /// Get the [`Aperture`] of the port with the given name.
    #[must_use]
    pub fn aperture(&self, port_type: &PortType, port_name: &str) -> Option<&Aperture> {
        self.ports(port_type)
            .get(port_name)
            .map(|config| &config.aperture)
    }

    /// Get the coating of the given input port.
    #[must_use]
    pub fn coating(&self, port_type: &PortType, port_name: &str) -> Option<&CoatingType> {
        self.ports(port_type)
            .get(port_name)
            .map(|config| &config.coating)
    }

    /// Get the LIDT of the given port.
    #[must_use]
    pub fn lidt(&self, port_type: &PortType, port_name: &str) -> Option<&Fluence> {
        self.ports(port_type)
            .get(port_name)
            .map(|config| config.lidt.get())
    }

    /// Mark the [`OpticPorts`] as `inverted`.
    pub const fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
}
impl Display for OpticPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "inputs:").unwrap();
        if self.inputs.is_empty() {
            writeln!(f, "  None").unwrap();
        } else {
            for (port_name, port_config) in self.ports(&PortType::Input) {
                writeln!(f, "  <{port_name}> {port_config}").unwrap();
            }
        }
        writeln!(f, "output:").unwrap();
        if self.outputs.is_empty() {
            writeln!(f, "  None").unwrap();
        } else {
            for (port_name, port_config) in self.ports(&PortType::Output) {
                writeln!(f, "  <{port_name}> {port_config}").unwrap();
            }
        }
        if self.inverted {
            writeln!(f, "ports are inverted").unwrap();
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::coatings::CoatingType;
    #[test]
    fn new() {
        let ports = OpticPorts::new();
        assert_eq!(ports.inputs.len(), 0);
        assert_eq!(ports.outputs.len(), 0);
        assert_eq!(ports.inverted, false);
    }
    #[test]
    fn add_input_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Input, "Test").is_ok());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_input_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Input, "Test").is_ok());
        assert!(ports.add(&PortType::Input, "Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Output, "Test").is_ok());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn add_output_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Output, "Test").is_ok());
        assert!(ports.add(&PortType::Output, "Test").is_err());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn inputs() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn inputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn set_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        assert_eq!(ports.inverted, true);
    }
    #[test]
    fn display_empty() {
        let ports = OpticPorts::new();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  None\noutput:\n  None\n".to_owned()
        );
    }
    #[test]
    fn display_entries() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();
        ports.add(&PortType::Output, "test2").unwrap();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test1> aperture: None, coating: IdealAR, lidt: 1.0 J/cm^2\noutput:\n  <test2> aperture: None, coating: IdealAR, lidt: 1.0 J/cm^2\n".to_owned()
        );
    }
    #[test]
    fn display_entries_inverted() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();

        ports.add(&PortType::Output, "test2").unwrap();
        ports.set_inverted(true);
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test2> aperture: None, coating: IdealAR, lidt: 1.0 J/cm^2\noutput:\n  <test1> aperture: None, coating: IdealAR, lidt: 1.0 J/cm^2\nports are inverted\n".to_owned()
        );
    }
    #[test]
    fn coating() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();
        assert!(matches!(
            ports.coating(&PortType::Input, "test1").unwrap(),
            CoatingType::IdealAR
        ));
        assert!(ports.coating(&PortType::Input, "wrong").is_none());
    }
    #[test]
    fn set_coating() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();
        assert!(matches!(
            ports.coating(&PortType::Input, "test1").unwrap(),
            CoatingType::IdealAR
        ));
        let coating = CoatingType::ConstantR { reflectivity: 0.5 };
        ports
            .set_coating(&PortType::Input, "test1", &coating)
            .unwrap();
        assert!(matches!(
            ports.coating(&PortType::Input, "test1").unwrap(),
            CoatingType::ConstantR { reflectivity: 0.5 }
        ));
    }
}
