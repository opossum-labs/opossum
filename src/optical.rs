use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::light::Light;
use crate::lightdata::LightData;
use crate::nodes::{create_node_ref, Dummy, NodeGroup};
use crate::optic_ports::OpticPorts;
use crate::properties::{Properties, Property};
use core::fmt::Debug;
use petgraph::prelude::DiGraph;
use petgraph::stable_graph::NodeIndex;
use serde::de::{self, Deserialize, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
pub type LightResult = HashMap<String, Option<LightData>>;
type Result<T> = std::result::Result<T, OpossumError>;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait Optical: Dottable {
    /// Sets the name of this [`Optical`].
    fn set_name(&mut self, _name: &str) {}
    /// Returns a reference to the name of this [`Optical`].
    fn name(&self) -> &str {
        self.node_type()
    }
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
    ) -> Result<LightResult> {
        print!("{}: No analyze function defined.", self.node_type());
        Ok(LightResult::default())
    }
    /// Export analysis data to file with the given name.
    fn export_data(&self, _file_name: &str) {
        println!(
            "no export_data function implemented for nodetype <{}>",
            self.node_type()
        )
    }
    /// Returns `true` if the [`Optical`] represents a detector which can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Mark this [`Optical`] as inverted.
    fn set_inverted(&mut self, _inverted: bool) {
        // self.ports.set_inverted(inverted);
        // self.node.set_inverted(inverted);
    }
    /// Returns `true` if this [`Optical`] is inverted.
    fn inverted(&self) -> bool {
        false
    }
    fn as_group(&self) -> Result<&NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
    }
    fn properties(&self) -> &Properties;
    fn set_property(&mut self, _name: &str, _prop: Property) -> Result<()> {
        Ok(())
    }
    fn set_properties(&mut self, properties: &Properties) -> Result<()> {
        let own_properties= self.properties().props.clone();

        for prop in properties.props.iter() {
            if own_properties.contains_key(prop.0) {
                self.set_property(prop.0, prop.1.clone())?;
            }
        }
        Ok(())
    }
}

impl Debug for dyn Optical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.node_type())
    }
}

#[derive(Debug, Default, Clone)]
pub struct OpticGraph(pub DiGraph<OpticRef, Light>);

impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let g = self.0.clone();
        let mut graph = serializer.serialize_struct("graph", 2)?;
        let nodes = g
            .node_weights()
            .map(|n| n.to_owned())
            .collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;
        let edgeidx = g
            .edge_indices()
            .map(|e| {
                (
                    g.edge_endpoints(e).unwrap().0,
                    g.edge_endpoints(e).unwrap().1,
                    g.edge_weight(e).unwrap().src_port(),
                    g.edge_weight(e).unwrap().target_port(),
                )
            })
            .collect::<Vec<(NodeIndex, NodeIndex, &str, &str)>>();
        graph.serialize_field("edges", &edgeidx)?;
        graph.end()
    }
}

#[derive(Debug, Clone)]
pub struct OpticRef(pub Rc<RefCell<dyn Optical>>);

impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut node = serializer.serialize_struct("node", 1)?;
        node.serialize_field("type", self.0.borrow().node_type())?;
        node.serialize_field("properties", &self.0.borrow().properties())?;
        node.end()
    }
}

impl<'de> Deserialize<'de> for OpticRef {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            NodeType,
            Properties,
        }
        const FIELDS: &'static [&'static str] = &["type", "properties"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("`type` or `properties`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "type" => Ok(Field::NodeType),
                            "properties" => Ok(Field::Properties),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticRefVisitor;

        impl<'de> Visitor<'de> for OpticRefVisitor {
            type Value = OpticRef;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a struct OpticRef")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<OpticRef, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let _node_type = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let _properties = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                Ok(OpticRef(Rc::new(RefCell::new(Dummy::default()))))
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticRef, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut node_type = None;
                let mut properties = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::NodeType => {
                            if node_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            node_type = Some(map.next_value()?);
                        }
                        Field::Properties => {
                            if properties.is_some() {
                                return Err(de::Error::duplicate_field("properties"));
                            }
                            properties = Some(map.next_value::<Properties>()?);
                        }
                    }
                }
                let node_type = node_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let properties =
                    properties.ok_or_else(|| de::Error::missing_field("properties"))?;
                let node =
                    create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;
                node.0
                    .borrow_mut()
                    .set_properties(&properties).map_err(|e| de::Error::custom(e.to_string()))?;
                Ok(node)
            }
        }
        deserializer.deserialize_struct("OpticRef", FIELDS, OpticRefVisitor)
    }
}
