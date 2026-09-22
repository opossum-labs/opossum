//! Common optical node attributes.
//!
//! This module provides common attributes and utilities for optical nodes, such as [`Properties`],
//! geometric placement ([`NodePositioning`]), alignment isometries, and 2D GUI coordinates.
//! These attributes are shared across different types of optical nodes in the system.

use nalgebra::Point2;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    core_optics::{OpticPorts, optic_surface::OpticSurface},
    error::{OpmResult, OpossumError},
    gain::Inversion,
    geometry::body::SurfaceBoundedBody,
    properties::{Properties, Proptype, validator::Validator},
    utils::{file_utils::sanitize_filename, geom_transformation::Isometry},
};

/// Container for runtime state of an optical node.
#[derive(Default, Debug, Clone)]
pub struct RuntimeSurfaces {
    /// Mapping of input port names to optical surfaces.
    pub inputs: BTreeMap<String, OpticSurface>,
    /// Mapping of output port names to optical surfaces.
    pub outputs: BTreeMap<String, OpticSurface>,
}

impl RuntimeSurfaces {
    /// Returns an iterator over all mutable optic surfaces (inputs and outputs).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.inputs.values_mut().chain(self.outputs.values_mut())
    }

    /// Returns an iterator over all optic surfaces (inputs and outputs) and their port names.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &OpticSurface)> {
        self.inputs.iter().chain(self.outputs.iter())
    }
}

/// The volume state a node was prepared with for the current analysis run.
///
/// The counterpart of [`RuntimeSurfaces`] for what lies *between* the surfaces. Built once per node
/// per run by [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume).
/// Between analysis passes [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data)
/// clears only the inversion field — the body is always re-derived by the next `prepare_volume`
/// call and does not need to be cached across resets.
#[derive(Debug, Clone)]
pub struct RuntimeMedium {
    body: SurfaceBoundedBody,
    inversion: Option<Inversion>,
}

impl RuntimeMedium {
    /// Return the volume body this node was prepared with.
    #[must_use]
    pub fn body(&self) -> &dyn crate::geometry::body::Body {
        &self.body
    }

    /// Return the inversion, if the model built one.
    #[must_use]
    pub const fn inversion(&self) -> Option<&Inversion> {
        self.inversion.as_ref()
    }

    /// Return the body and inversion as a split mutable borrow.
    ///
    /// Splits `&mut RuntimeMedium` into an immutable reference to the body and a mutable reference
    /// to the inversion. The two borrows are valid simultaneously because they target different
    /// fields of the struct.
    ///
    /// # Returns
    ///
    /// `(&dyn Body, &mut Option<Inversion>)` — the body for geometric queries and the inversion
    /// that saturating models may deplete between substeps.
    pub fn parts_mut(&mut self) -> (&dyn crate::geometry::body::Body, &mut Option<Inversion>) {
        (&self.body, &mut self.inversion)
    }
}

fn deserialize_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(sanitize_filename(&s))
}

/// Defines the 3D positioning strategy of an optical node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodePositioning {
    /// Fixed, user-defined absolute position in 3D space.
    Absolute(Isometry),
    /// Automatically positioned along the optical axis during an analysis run.
    ///
    /// The inner option stores the calculated geometry from a previous positioning pass.
    /// - `None`: Not yet calculated for the current configuration.
    /// - `Some(isometry)`: Calculated in an earlier alignment pass.
    Automatic(Option<Isometry>),
}

impl Default for NodePositioning {
    fn default() -> Self {
        Self::Automatic(None)
    }
}

impl From<Isometry> for NodePositioning {
    fn from(iso: Isometry) -> Self {
        Self::Absolute(iso)
    }
}

impl NodePositioning {
    /// Returns the effective isometry, whether absolute or calculated by an automatic run.
    #[must_use]
    pub const fn effective_position(&self) -> Option<&Isometry> {
        match self {
            Self::Absolute(iso) => Some(iso),
            Self::Automatic(cached) => cached.as_ref(),
        }
    }

    /// Returns `true` if this node uses automatic positioning.
    #[must_use]
    pub const fn is_automatic(&self) -> bool {
        matches!(self, Self::Automatic(_))
    }

    /// Returns `true` if this node has a user-defined absolute position.
    #[must_use]
    pub const fn is_absolute(&self) -> bool {
        matches!(self, Self::Absolute(_))
    }

    /// Returns `true` if the node is in automatic mode and has already cached a calculated position.
    #[must_use]
    pub const fn has_cached_position(&self) -> bool {
        matches!(self, Self::Automatic(Some(_)))
    }

    /// Returns a reference to the cached isometry if in automatic mode.
    #[must_use]
    pub const fn cached_position(&self) -> Option<&Isometry> {
        if let Self::Automatic(cached) = self {
            cached.as_ref()
        } else {
            None
        }
    }

    /// Sets the cached runtime isometry if the node is in automatic mode.
    ///
    /// Does nothing if the node is configured as absolute.
    pub const fn set_cached_isometry(&mut self, iso: Isometry) {
        if let Self::Automatic(cached) = self {
            *cached = Some(iso);
        }
    }

    /// Clears the cached runtime isometry, resetting it to `Automatic(None)`.
    ///
    /// Does nothing if the node is configured as absolute.
    pub const fn clear_cache(&mut self) {
        if let Self::Automatic(cached) = self {
            *cached = None;
        }
    }
}

// Internal proxy for serialization to ensure runtime cache is never written to disk.
#[derive(Serialize, Deserialize)]
enum PositioningSerdeProxy {
    Absolute(Isometry),
    Automatic,
}

impl Serialize for NodePositioning {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let proxy = match self {
            Self::Absolute(iso) => PositioningSerdeProxy::Absolute(*iso),
            Self::Automatic(_) => PositioningSerdeProxy::Automatic,
        };
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodePositioning {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match PositioningSerdeProxy::deserialize(deserializer)? {
            PositioningSerdeProxy::Absolute(iso) => Ok(Self::Absolute(iso)),
            PositioningSerdeProxy::Automatic => Ok(Self::Automatic(None)),
        }
    }
}

/// Struct for storing common attributes of optical nodes.
///
/// `NodeAttr` encapsulates metadata and configuration for an optical node, including its type,
/// name, ports, unique identifier, properties, geometric placement, alignment, and GUI position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAttr {
    /// The type of the node (e.g., "lens", "mirror").
    node_type: String,
    /// The name of the node.
    #[serde(deserialize_with = "deserialize_name")]
    name: String,
    #[serde(default, skip_serializing_if = "OpticPorts::is_all_default")]
    ports: OpticPorts,
    #[serde(skip)]
    runtime_surfaces: RuntimeSurfaces,
    #[serde(skip)]
    runtime_medium: Option<RuntimeMedium>,
    uuid: Uuid,
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    props: Properties,
    /// 3D placement mode (Absolute or Automatic).
    #[serde(default, skip_serializing_if = "NodePositioning::is_automatic")]
    positioning: NodePositioning,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    inverted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment: Option<Isometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    align_like_node_at_distance: Option<(Uuid, Length)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: Option<Point2<f64>>,
}

impl NodeAttr {
    /// Creates new node attributes ([`NodeAttr`]).
    ///
    /// This constructor initializes a node with standard default properties:
    /// - `name`: Set to the provided `node_type` string.
    /// - `node_type`: Set to the provided `node_type` string.
    /// - `ports`: Set to default empty [`OpticPorts`].
    /// - `positioning`: Set to [`NodePositioning::Automatic(None)`].
    /// - `inverted`: Set to `false`.
    /// - `alignment`: Set to `None`.
    /// - `align_like_node_at_distance`: Set to `None`.
    /// - `uuid`: Randomly generated [`Uuid`].
    /// - `gui_position`: Set to `None`.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type of the optical node (e.g., "lens", "mirror").
    #[must_use]
    pub fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.into(),
            name: node_type.into(),
            props: Properties::default(),
            ports: OpticPorts::default(),
            runtime_surfaces: RuntimeSurfaces::default(),
            runtime_medium: None,
            positioning: NodePositioning::Automatic(None),
            inverted: false,
            alignment: None,
            align_like_node_at_distance: None,
            uuid: Uuid::new_v4(),
            gui_position: None,
        }
    }

    /// Returns the name of this node.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type identifier of this node.
    #[must_use]
    pub fn node_type(&self) -> &str {
        &self.node_type
    }

    /// Returns whether this node is physically inverted in the optical chain.
    #[must_use]
    pub const fn inverted(&self) -> bool {
        self.inverted
    }

    /// Sets a property of this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// Returns an error if the property does not exist or has the wrong [`Proptype`].
    pub fn set_property(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        self.props.set(name, value)
    }

    /// Updates multiple properties of this [`NodeAttr`].
    pub fn update_properties(&mut self, new_props: Properties) {
        self.props.update(new_props);
    }

    /// Replaces the entire properties container of this [`NodeAttr`].
    pub fn set_properties(&mut self, props: Properties) {
        self.props = props;
    }

    /// Creates a property within this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// Returns an error if the property already exists.
    pub fn create_property(
        &mut self,
        name: &str,
        description: &str,
        value: Proptype,
    ) -> OpmResult<()> {
        self.props.create(name, description, value)
    }

    /// Creates a property with an attached validator within this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// Returns an error if the property already exists or validation fails for the initial value.
    pub fn create_property_with_validator(
        &mut self,
        name: &str,
        description: &str,
        validator: Validator,
        value: Proptype,
    ) -> OpmResult<()> {
        self.props
            .create_with_validator(name, description, validator, value)
    }

    /// Returns a reference to the properties of this [`NodeAttr`].
    #[must_use]
    pub const fn properties(&self) -> &Properties {
        &self.props
    }

    /// Returns a reference to a property value by name.
    ///
    /// # Errors
    ///
    /// Returns an error if no property with the given name exists.
    pub fn get_property(&self, name: &str) -> OpmResult<&Proptype> {
        self.props.get(name)
    }

    /// Returns the boolean value of a named property.
    ///
    /// # Errors
    ///
    /// Returns an error if the property does not exist or is not a boolean.
    pub fn get_property_bool(&self, name: &str) -> OpmResult<bool> {
        let bool_prop = self.props.get(name)?;
        if let Proptype::Bool(value) = bool_prop {
            Ok(*value)
        } else {
            Err(OpossumError::Other("not a bool property".into()))
        }
    }

    /// Returns the current positioning configuration.
    #[must_use]
    pub const fn positioning(&self) -> &NodePositioning {
        &self.positioning
    }

    /// Returns a mutable reference to the positioning configuration.
    pub const fn positioning_mut(&mut self) -> &mut NodePositioning {
        &mut self.positioning
    }

    /// Sets the positioning strategy.
    pub const fn set_positioning(&mut self, positioning: NodePositioning) {
        self.positioning = positioning;
    }

    /// Convenience getter for the effective isometry (absolute or cached automatic).
    #[must_use]
    pub const fn effective_position(&self) -> Option<&Isometry> {
        self.positioning.effective_position()
    }

    /// Caches a calculated isometry if this node is configured for automatic positioning.
    pub const fn set_cached_isometry(&mut self, iso: Isometry) {
        self.positioning.set_cached_isometry(iso);
    }

    /// Clears any cached runtime position, leaving the node at `Automatic(None)`.
    pub const fn clear_cached_position(&mut self) {
        self.positioning.clear_cache();
    }

    /// Returns the local alignment isometry of a node (if any).
    #[must_use]
    pub const fn alignment(&self) -> &Option<Isometry> {
        &self.alignment
    }

    /// Sets the local alignment isometry of this [`NodeAttr`].
    pub const fn set_alignment(&mut self, isometry: Isometry) {
        self.alignment = Some(isometry);
    }

    /// Sets or clears the local alignment isometry of this [`NodeAttr`].
    pub const fn set_alignment_option(&mut self, alignment_opt: Option<Isometry>) {
        self.alignment = alignment_opt;
    }

    /// Sets the sanitized name of this [`NodeAttr`].
    pub fn set_name(&mut self, name: &str) {
        self.name = sanitize_filename(name);
    }

    /// Sets the inversion flag for this node.
    pub const fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }

    /// Returns a reference to the stored optic ports of this [`NodeAttr`].
    ///
    /// **Warning**: Only returns raw internally stored ports. For virtual nodes like
    /// `NodeReference`, use the `ports()` method of the `OpticNode` trait instead.
    #[must_use]
    pub const fn raw_ports(&self) -> &OpticPorts {
        &self.ports
    }

    /// Returns a mutable reference to the optic ports of this [`NodeAttr`].
    #[must_use]
    pub const fn raw_ports_mut(&mut self) -> &mut OpticPorts {
        &mut self.ports
    }

    /// Sets the port configuration of this [`NodeAttr`].
    pub fn set_ports(&mut self, ports: OpticPorts) {
        self.ports = ports;
    }

    /// Returns a mutable reference to the runtime surfaces.
    pub const fn runtime_surfaces_mut(&mut self) -> &mut RuntimeSurfaces {
        &mut self.runtime_surfaces
    }

    /// Returns a reference to the runtime surfaces.
    #[must_use]
    pub const fn runtime_surfaces(&self) -> &RuntimeSurfaces {
        &self.runtime_surfaces
    }

    /// Return the prepared medium for this node, if any.
    #[must_use]
    pub const fn runtime_medium(&self) -> Option<&RuntimeMedium> {
        self.runtime_medium.as_ref()
    }

    /// Return a mutable reference to the prepared medium for this node, if any.
    pub const fn runtime_medium_mut(&mut self) -> Option<&mut RuntimeMedium> {
        self.runtime_medium.as_mut()
    }

    /// Stores the prepared medium for this node.
    pub fn set_runtime_medium(&mut self, body: SurfaceBoundedBody, inversion: Option<Inversion>) {
        self.runtime_medium = Some(RuntimeMedium { body, inversion });
    }

    /// Clears the inversion field of the prepared medium between analysis runs.
    pub fn clear_runtime_inversion(&mut self) {
        if let Some(medium) = self.runtime_medium.as_mut() {
            medium.inversion = None;
        }
    }

    /// Returns the unique identifier of this node.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Sets the unique identifier of this node.
    pub const fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    /// Sets the relative alignment to another node index and distance.
    pub fn set_align_like_node_at_distance(&mut self, node_id: Uuid, distance: Length) {
        self.align_like_node_at_distance = Some((node_id, distance));
    }

    /// Returns the target node and distance for relative alignment, if set.
    #[must_use]
    pub const fn get_align_like_node_at_distance(&self) -> &Option<(Uuid, Length)> {
        &self.align_like_node_at_distance
    }

    /// Returns the 2D position of the node on the frontend canvas.
    #[must_use]
    pub const fn gui_position(&self) -> Option<Point2<f64>> {
        self.gui_position
    }

    /// Sets the 2D position of the node on the frontend canvas.
    pub const fn set_gui_position(&mut self, gui_position: Option<Point2<f64>>) {
        self.gui_position = gui_position;
    }

    /// Replaces attributes with a copy of `node_attr`, preserving the original UUID.
    pub fn replace_from_node_attr(&mut self, node_attr: &Self) {
        let id = self.uuid;
        *self = node_attr.clone();
        self.uuid = id;
    }
}

/// Trait for basic optical node attribute access.
pub trait HasNodeAttr {
    /// Returns an immutable reference to the node attributes.
    fn node_attr(&self) -> &NodeAttr;
    /// Returns a mutable reference to the node attributes.
    fn node_attr_mut(&mut self) -> &mut NodeAttr;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        millimeter,
        types::validated_type_definitions::ValidatedCrossSection,
    };
    use std::sync::{Arc, Mutex};

    fn test_body() -> OpmResult<SurfaceBoundedBody> {
        Ok(SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        ))
    }

    #[test]
    fn node_positioning_defaults_and_effective_position() -> OpmResult<()> {
        let mut pos = NodePositioning::default();
        assert_eq!(pos, NodePositioning::Automatic(None));
        assert!(pos.is_automatic());
        assert!(!pos.is_absolute());
        assert!(!pos.has_cached_position());
        assert_eq!(pos.effective_position(), None);

        let iso = Isometry::new_along_z(millimeter!(100.0))?;
        pos.set_cached_isometry(iso);
        assert!(pos.has_cached_position());
        assert_eq!(pos.effective_position(), Some(&iso));
        assert_eq!(pos.cached_position(), Some(&iso));

        pos.clear_cache();
        assert_eq!(pos.effective_position(), None);
        assert!(!pos.has_cached_position());

        let abs_pos = NodePositioning::from(iso);
        assert!(abs_pos.is_absolute());
        assert_eq!(abs_pos.effective_position(), Some(&iso));
        Ok(())
    }

    #[test]
    fn node_positioning_serde_discards_cached_runtime_position() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(50.0))?;
        let pos_cached = NodePositioning::Automatic(Some(iso));

        // Serialize to RON
        let serialized =
            ron::to_string(&pos_cached).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(serialized, "Automatic");

        // Deserialize back: must be reset to Automatic(None)
        let deserialized: NodePositioning =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(deserialized, NodePositioning::Automatic(None));
        assert_eq!(deserialized.effective_position(), None);
        Ok(())
    }

    #[test]
    fn node_attr_skips_serializing_automatic_positioning() -> OpmResult<()> {
        let mut attr = NodeAttr::new("test");
        let iso = Isometry::new_along_z(millimeter!(25.0))?;

        // In default Automatic(None), positioning field is skipped entirely
        let serialized = ron::to_string(&attr).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!serialized.contains("positioning"));

        // Setting a cached value still counts as is_automatic and is skipped
        attr.set_cached_isometry(iso);
        let serialized_cached =
            ron::to_string(&attr).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!serialized_cached.contains("positioning"));

        // When explicitly set to Absolute, positioning is serialized.
        attr.set_positioning(NodePositioning::Absolute(iso));
        let serialized_abs =
            ron::to_string(&attr).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(serialized_abs.contains("positioning:Absolute"));

        // Roundtrip test for Absolute
        let deserialized: NodeAttr =
            ron::from_str(&serialized_abs).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(deserialized.positioning(), &NodePositioning::Absolute(iso));
        Ok(())
    }

    #[test]
    fn runtime_medium_starts_unset() {
        assert!(NodeAttr::new("test").runtime_medium().is_none());
    }

    #[test]
    fn set_and_clear_runtime_inversion() -> OpmResult<()> {
        let mut attr = NodeAttr::new("test");
        attr.set_runtime_medium(test_body()?, None);
        assert!(attr.runtime_medium().is_some());
        attr.clear_runtime_inversion();
        assert!(attr.runtime_medium().is_some());
        assert!(attr.runtime_medium().unwrap().inversion().is_none());
        Ok(())
    }

    #[test]
    fn runtime_medium_is_not_in_ron_roundtrip() -> OpmResult<()> {
        let mut attr = NodeAttr::new("test");
        attr.set_runtime_medium(test_body()?, None);
        let serialized = ron::to_string(&attr).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(
            !serialized.contains("runtime_medium"),
            "runtime_medium must not be serialized: {serialized}"
        );
        let back: NodeAttr =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(back.runtime_medium().is_none());
        Ok(())
    }

    #[test]
    fn set_name_sanitization() {
        let mut attr = NodeAttr::new("test");
        attr.set_name("../bad_name");
        assert_eq!(attr.name(), ".._bad_name");
    }

    #[test]
    fn deserialize_name_sanitization() -> OpmResult<()> {
        let ron_str = r#"
            (
                node_type: "test",
                name: "../malicious",
                uuid: "98248e7f-dc4c-4131-8710-f3d5be2ff087",
                ports: (
                    inputs: {},
                    outputs: {}
                )
            )
        "#;
        let attr: NodeAttr =
            ron::from_str(ron_str).map_err(|e| OpossumError::OpmDocument(e.to_string()))?;
        assert_eq!(attr.name(), ".._malicious");
        Ok(())
    }
}
