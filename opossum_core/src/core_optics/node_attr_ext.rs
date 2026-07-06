use crate::{
    core_optics::{
        NodeAttr, OpticPorts, SceneryResources, hit_map::HitMap, node_attr::HasNodeAttr,
        optic_surface::OpticSurface,
    },
    error::OpmResult,
    light::Rays,
    properties::{Properties, Proptype},
    utils::geom_transformation::Isometry,
};
use nalgebra::Point2;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Extension trait providing standardized, non-overridable methods
/// for checking and manipulating core node attributes.
pub trait NodeAttrExt {
    // --- Immutable Methods (Getters) ---

    /// Get the node type of this [`OpticNode`]
    fn node_type(&self) -> &str;

    /// Get the name of this [`OpticNode`]
    fn name(&self) -> &str;

    /// Get the gui position of this [`OpticNode`].
    fn gui_position(&self) -> Option<Point2<f64>>;

    /// Return all properties of this [`OpticNode`].
    fn properties(&self) -> &Properties;

    /// Returns `true` if the node should be analyzed in reverse direction.
    fn inverted(&self) -> bool;

    /// Get the local alignment (decenter, tilt) of an optical node.
    ///
    /// This function returns `None` if no local alignment is defined for this node.
    fn alignment(&self) -> Option<Isometry>;

    /// Get a reference to a global configuration (if any).
    fn global_conf(&self) -> &Option<Arc<Mutex<SceneryResources>>>;

    /// Return a [`String`] in the form `'name' (type)` for display purposes.
    fn node_info(&self) -> String;

    /// Returns a reference to an [`OpticSurface`] of this [`OpticNode`] with the key `surf_name`
    /// # Attributes
    /// - `surf_name`: name of the optical surface, which is the key in the [`OpticPorts`] hashmap stat stores the surfaces
    fn get_optic_surface(&self, surf_name: &str) -> Option<&OpticSurface>;

    /// Return all hit maps (if any) of this [`OpticNode`].
    fn hit_maps(&self) -> HashMap<String, HitMap>;

    // --- Mutable Methods (Setters / Actions) ---

    /// Set a property of this [`OpticNode`].
    ///
    /// Set a property of an optical node. This property must already exist (e.g. defined in `new()` / `default()` functions of the node).
    ///
    /// # Errors
    /// This function will return an error if a non-defined property is set or the property has the wrong data type.
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()>;

    /// Return the available (input & output) ports of this [`OpticNode`] as mutables.
    fn ports_mut(&mut self) -> &mut OpticPorts;

    /// Returns a mutable reference to an [`OpticSurface`] of this [`OpticNode`] with the key `surf_name`
    /// # Attributes
    /// - `surf_name`: name of the optical surface, which is the key in the [`OpticPorts`] hashmap stat stores the surfaces
    fn get_optic_surface_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface>;

    /// Resets the data-holding fields of all [`OpticSurface`]s of this node
    /// This includes the forward and backward rays cache, as well as the hitmaps
    fn reset_optic_surfaces(&mut self);

    /// Update node attributes of this [`OpticNode`] from given [`NodeAttr`].
    ///
    /// # Errors
    /// Returns an error if validation fails.
    fn set_node_attr(&mut self, node_attributes: NodeAttr) -> OpmResult<()>;
}

/// Blanket implementation for any type that provides access to `NodeAttr`.
impl<T: ?Sized + HasNodeAttr> NodeAttrExt for T {
    fn node_type(&self) -> &str {
        self.node_attr().node_type()
    }

    fn name(&self) -> &str {
        self.node_attr().name()
    }

    fn gui_position(&self) -> Option<Point2<f64>> {
        self.node_attr().gui_position()
    }

    fn properties(&self) -> &Properties {
        self.node_attr().properties()
    }
    fn inverted(&self) -> bool {
        self.node_attr().inverted()
    }

    fn alignment(&self) -> Option<Isometry> {
        *self.node_attr().alignment()
    }

    fn global_conf(&self) -> &Option<Arc<Mutex<SceneryResources>>> {
        self.node_attr().global_conf()
    }

    fn node_info(&self) -> String {
        format!("'{}' ({})", self.name(), self.node_type())
    }
    fn get_optic_surface(&self, surf_name: &str) -> Option<&OpticSurface> {
        let runtime = self.node_attr().runtime_surfaces();
        runtime
            .inputs
            .get(surf_name)
            .or_else(|| runtime.outputs.get(surf_name))
    }

    fn hit_maps(&self) -> HashMap<String, HitMap> {
        let mut map: HashMap<String, HitMap> = HashMap::default();
        let runtime = self.node_attr().runtime_surfaces();

        for (port_name, optic_surf) in &runtime.inputs {
            if !optic_surf.hit_map().is_empty() {
                map.insert(port_name.clone(), optic_surf.hit_map().to_owned());
            }
        }
        for (port_name, optic_surf) in &runtime.outputs {
            if !optic_surf.hit_map().is_empty() {
                map.insert(port_name.clone(), optic_surf.hit_map().to_owned());
            }
        }
        map
    }

    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()> {
        self.node_attr_mut().set_property(name, proptype)
    }
    fn ports_mut(&mut self) -> &mut OpticPorts {
        let inverted = self.node_attr().inverted();
        let ports = self.node_attr_mut().raw_ports_mut();
        if inverted {
            ports.set_inverted(true);
        }
        ports
    }

    fn get_optic_surface_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        let runtime = self.node_attr_mut().runtime_surfaces_mut();
        runtime
            .inputs
            .get_mut(surf_name)
            .or_else(|| runtime.outputs.get_mut(surf_name))
    }

    fn reset_optic_surfaces(&mut self) {
        let runtime = self.node_attr_mut().runtime_surfaces_mut();
        for optic_surf in runtime.inputs.values_mut() {
            optic_surf.set_backwards_rays_cache(Vec::<Rays>::new());
            optic_surf.set_forward_rays_cache(Vec::<Rays>::new());
            optic_surf.reset_hit_map();
        }
        for optic_surf in runtime.outputs.values_mut() {
            optic_surf.set_backwards_rays_cache(Vec::<Rays>::new());
            optic_surf.set_forward_rays_cache(Vec::<Rays>::new());
            optic_surf.reset_hit_map();
        }
    }

    fn set_node_attr(&mut self, node_attributes: NodeAttr) -> OpmResult<()> {
        let node_attr_mut = self.node_attr_mut();
        if let Some(iso) = node_attributes.isometry() {
            node_attr_mut.set_isometry(iso);
        }
        if let Some(alignment) = node_attributes.alignment() {
            node_attr_mut.set_alignment(*alignment);
        }
        node_attr_mut.set_name(node_attributes.name());
        node_attr_mut.set_inverted(node_attributes.inverted());
        if let Some((node_idx, distance)) = node_attributes.get_align_like_node_at_distance() {
            node_attr_mut.set_align_like_node_at_distance(*node_idx, *distance);
        }
        node_attr_mut.update_properties(node_attributes.properties().clone());
        node_attr_mut.set_ports(node_attributes.raw_ports().clone());
        node_attr_mut.set_uuid(node_attributes.uuid());
        node_attr_mut.set_gui_position(node_attributes.gui_position());
        Ok(())
    }
}
