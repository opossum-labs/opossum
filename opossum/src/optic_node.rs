#![warn(missing_docs)]
//! Contains the basic trait representing an optical element
use log::warn;
use nalgebra::{Point2, Point3, Vector3};
use uom::si::f64::{Angle, Length};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    aperture::Aperture,
    coatings::CoatingType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{NodeAttr, NodeGroup, NodeReference, fluence_detector::Fluence},
    optic_ports::{OpticPorts, PortType},
    optic_scenery_rsc::SceneryResources,
    properties::{Properties, Proptype},
    rays::Rays,
    refractive_index::RefractiveIndexType,
    reporting::node_report::NodeReport,
    surface::{Plane, geo_surface::GeoSurfaceRef, hit_map::HitMap, optic_surface::OpticSurface},
    utils::geom_transformation::Isometry,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait OpticNode: Dottable {
    ///Sets the apodization warning on nodes that have that attribute
    fn set_apodization_warning(&mut self, _apodized: bool) {
        warn!(
            "\"set_apodization_warning\" is not implemented for '{}' ({})",
            self.name(),
            self.node_type()
        );
    }

    /// Return all hit maps (if any) of this [`OpticNode`].
    fn hit_maps(&self) -> HashMap<String, HitMap> {
        let mut map: HashMap<String, HitMap> = HashMap::default();
        for (port_name, optic_surf) in self.ports().ports(&PortType::Input) {
            if !optic_surf.hit_map().is_empty() {
                map.insert(port_name.clone(), optic_surf.hit_map().to_owned());
            }
        }
        for (port_name, optic_surf) in self.ports().ports(&PortType::Output) {
            if !optic_surf.hit_map().is_empty() {
                map.insert(port_name.clone(), optic_surf.hit_map().to_owned());
            }
        }
        map
    }
    /// Reset internal data (e.g. internal state of detector nodes)
    fn reset_data(&mut self) {
        self.reset_optic_surfaces();
    }

    /// Update the surfaces of nodes with a single interacting surface. E.g. detectors
    /// # Errors
    /// This function errors if the function `add_optic_surface` fails
    fn update_flat_single_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));

        self.update_surface(
            &"input_1".to_string(),
            geosurface.clone(),
            Isometry::identity(),
            &PortType::Input,
        )?;
        self.update_surface(
            &"output_1".to_string(),
            geosurface,
            Isometry::identity(),
            &PortType::Output,
        )?;

        Ok(())
    }

    /// Resets the data-holding fields of all [`OpticSurface`]s of this node
    /// This includes the forward and backward rays cache, as well as the hitmaps
    fn reset_optic_surfaces(&mut self) {
        for optic_surf in self.ports_mut().ports_mut(&PortType::Input).values_mut() {
            optic_surf.set_backwards_rays_cache(Vec::<Rays>::new());
            optic_surf.set_forward_rays_cache(Vec::<Rays>::new());
            optic_surf.reset_hit_map();
        }
        for optic_surf in self.ports_mut().ports_mut(&PortType::Output).values_mut() {
            optic_surf.set_backwards_rays_cache(Vec::<Rays>::new());
            optic_surf.set_forward_rays_cache(Vec::<Rays>::new());
            optic_surf.reset_hit_map();
        }
    }
    /// Return the available (input & output) ports of this [`OpticNode`].
    fn ports(&self) -> OpticPorts {
        let mut ports = self.node_attr().ports().clone();
        if self.node_attr().inverted() {
            ports.set_inverted(true);
        }
        ports
    }

    /// Return the available (input & output) ports of this [`OpticNode`] as mutables.
    fn ports_mut(&mut self) -> &mut OpticPorts {
        let inverted = self.node_attr().inverted();
        let ports = self.node_attr_mut().ports_mut();
        if inverted {
            ports.set_inverted(true);
        }
        ports
    }
    /// Set an [`Aperture`] for a given port name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_aperture(port_type, port_name, aperture)?;
        self.node_attr_mut().set_ports(ports);
        Ok(())
    }
    /// Set a coating for a given port name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_coating(port_type, port_name, coating)?;
        self.node_attr_mut().set_ports(ports);
        Ok(())
    }
    /// define the up-direction of this lightdata's first ray which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process, and only for source nodes.
    ///
    /// # Errors
    /// This function errors if the the lightdata is not geometric
    fn define_up_direction(&self, ray_data: &LightData) -> OpmResult<Vector3<f64>> {
        if let LightData::Geometric(rays) = ray_data {
            rays.define_up_direction()
        } else {
            Err(OpossumError::Other(
                "Wrong light data for \"up-direction\" definition".into(),
            ))
        }
    }
    /// Modifies the current up-direction of a ray, stored in lightdata, which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process.
    ///
    /// # Errors
    /// This function errors if the the lightdata is not geometric
    fn calc_new_up_direction(
        &self,
        ray_data: &LightData,
        up_direction: &mut Vector3<f64>,
    ) -> OpmResult<()> {
        if let LightData::Geometric(rays) = ray_data {
            rays.calc_new_up_direction(up_direction)?;
        } else {
            return Err(OpossumError::Other(
                "Wrong light data for \"up-direction\" calculation".into(),
            ));
        }
        Ok(())
    }
    /// Export analysis data to file(s) within the given directory path.
    ///
    /// This function should be overridden by a node in order to export node-specific data into a file.
    /// The default implementation does nothing.
    ///
    /// # Errors
    /// This function might return an error depending on the particular implementation.
    // fn export_data(&self, _data_dir: &Path, _uuid: &str) -> OpmResult<()> {
    //     Ok(())
    // }
    /// Return a downcasted reference of a [`NodeGroup`].
    ///
    /// # Errors
    /// This function will return an error if the [`OpticNode`] does not have the `node_type` property "group".
    fn as_group_mut(&mut self) -> OpmResult<&mut NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
    }
    /// This function is called right after a node has been deserialized (e.g. read from a file). By default, this
    /// function does nothing and returns no error.
    ///
    /// Currently this function is needed for group nodes whose internal graph structure must be synchronized with the
    /// graph stored in their properties.
    ///
    /// # Errors
    /// This function will return an error if the overwritten function generates an error.
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.update_lidt()?;
        self.update_surfaces()?;
        Ok(())
    }
    /// Updates the surfaces of this node after deserialization
    ///
    /// # Errors
    ///
    /// This function might return an error in a non-default implementation
    fn update_surfaces(&mut self) -> OpmResult<()>;

    /// Updates a single surface of this node
    ///
    /// # Attributes
    /// `surf_name`: name of the surface,
    /// `geo_surface`: the geometric surface [`GeoSurfaceRef`],
    /// `anchor_point_iso`: the isometry of the geometrical anchor point,
    /// `port_type`: the port type of this surface
    ///
    /// # Errors
    /// This function errors if `add_optic_surface` fails
    fn update_surface(
        &mut self,
        surf_name: &String,
        geo_surface: GeoSurfaceRef,
        anchor_point_iso: Isometry,
        port_type: &PortType,
    ) -> OpmResult<()> {
        if let Some(optic_surf) = self.ports_mut().get_optic_surface_mut(surf_name) {
            optic_surf.set_geo_surface(geo_surface);
            optic_surf.set_anchor_point_iso(anchor_point_iso);
        } else {
            let mut optic_surf = OpticSurface::default();
            optic_surf.set_geo_surface(geo_surface);
            optic_surf.set_anchor_point_iso(anchor_point_iso);
            self.ports_mut()
                .add_optic_surface(port_type, surf_name, optic_surf)?;
        }
        Ok(())
    }
    /// Updates the LIDT of the optical surfaces after deserialization
    ///
    /// # Errors
    ///
    /// This funtion returns an error if the LIDTs to be deserialized are invalid.
    fn update_lidt(&mut self) -> OpmResult<()> {
        let lidt = *self.node_attr().lidt();
        for optic_surf in self.ports_mut().ports_mut(&PortType::Input).values_mut() {
            optic_surf.set_lidt(lidt)?;
        }
        for optic_surf in self.ports_mut().ports_mut(&PortType::Output).values_mut() {
            optic_surf.set_lidt(lidt)?;
        }
        Ok(())
    }
    /// Return a downcasted mutable reference of a [`NodeReference`].
    ///
    /// # Errors
    /// This function will return an error if the [`OpticNode`] does not have the `node_type` property "reference".
    fn as_refnode_mut(&mut self) -> OpmResult<&mut NodeReference> {
        Err(OpossumError::Other("cannot cast to reference node".into()))
    }
    /// Set a property of this [`OpticNode`].
    ///
    /// Set a property of an optical node. This property must already exist (e.g. defined in `new()` / `default()` functions of the node).
    ///
    /// # Errors
    /// This function will return an error if a non-defined property is set or the property has the wrong data type.
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()> {
        self.node_attr_mut().set_property(name, proptype)
    }
    /// Set this [`OpticNode`] as inverted.
    ///
    /// This flag signifies that the [`OpticNode`] should be propagated in reverse order. This function normally simply sets the
    /// `inverted` property. For [`NodeGroup`] it also sets the `inverted` flag of the underlying `OpticGraph`.
    ///
    /// # Errors
    /// This function returns an error, if the node cannot be inverted. This is the case, if
    ///   - it is a source node
    ///   - it is a group node containing a non-invertable node (e.g. a source)
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }
    /// Returns `true` if the node should be analyzed in reverse direction.
    fn inverted(&self) -> bool {
        self.node_attr().inverted()
    }
    /// Return [`NodeReport`] of the current state of this [`OpticNode`].
    ///
    /// This function must be overridden for generating output in the analysis report. Mainly
    /// detector nodes use this feature.
    fn node_report(&self, _uuid: &str) -> Option<NodeReport> {
        None
    }
    /// Get the [`NodeAttr`] (common attributes) of an [`OpticNode`].
    fn node_attr(&self) -> &NodeAttr;
    /// Get the mutable[`NodeAttr`] (common attributes) of an [`OpticNode`].
    fn node_attr_mut(&mut self) -> &mut NodeAttr;
    /// Update node attributes of this [`OpticNode`] from given [`NodeAttr`].
    fn set_node_attr(&mut self, node_attributes: NodeAttr) {
        let node_attr_mut = self.node_attr_mut();
        if let Some(iso) = node_attributes.isometry() {
            let () = node_attr_mut.set_isometry(iso);
        }
        if let Some(alignment) = node_attributes.alignment() {
            node_attr_mut.set_alignment(*alignment);
        }
        node_attr_mut.set_name(&node_attributes.name());
        node_attr_mut.set_inverted(node_attributes.inverted());
        if let Some((node_idx, distance)) = node_attributes.get_align_like_node_at_distance() {
            node_attr_mut.set_align_like_node_at_distance(*node_idx, *distance);
        }
        node_attr_mut.update_properties(node_attributes.properties().clone());

        node_attr_mut.set_ports(node_attributes.ports().clone());

        node_attr_mut.set_uuid(node_attributes.uuid());
        node_attr_mut.set_lidt(node_attributes.lidt());
        node_attr_mut.set_gui_position(node_attributes.gui_position());
    }
    /// Get the node type of this [`OpticNode`]
    fn node_type(&self) -> String {
        self.node_attr().node_type()
    }
    /// Get the name of this [`OpticNode`]
    fn name(&self) -> String {
        self.node_attr().name()
    }
    /// Get the gui position of this [`OpticNode`].
    fn gui_position(&self) -> Option<Point2<f64>> {
        self.node_attr().gui_position()
    }
    /// Return all properties of this [`OpticNode`].
    fn properties(&self) -> &Properties {
        self.node_attr().properties()
    }
    /// Return the (base) [`Isometry`] of this optical node.
    fn isometry(&self) -> Option<Isometry> {
        self.node_attr().isometry()
    }
    /// Set the (base) [`Isometry`] (position and angle) of this optical node.
    ///
    /// # Errors
    /// This function errors if the `update_surfaces` function fails
    fn set_isometry(&mut self, isometry: Isometry) -> OpmResult<()> {
        self.node_attr_mut().set_isometry(isometry);
        self.update_surfaces()
    }
    /// Return the effective input isometry of this optical node.
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any).
    fn effective_node_iso(&self) -> Option<Isometry> {
        self.isometry().as_ref().and_then(|iso| {
            self.node_attr()
                .alignment()
                .as_ref()
                .map_or_else(|| Some(*iso), |local_iso| Some(iso.append(local_iso)))
        })
    }
    /// Return the effective input isometry of an [`OpticSurface`].
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any) and the anchor point isometry.  
    ///
    /// # Errors
    /// This function errors if
    /// - no effective node isometry is defined  
    /// - the surface with the specified name cannot be found
    fn effective_surface_iso(&self, surf_name: &str) -> OpmResult<Isometry> {
        let Some(eff_node_iso) = self.effective_node_iso() else {
            return Err(OpossumError::Other("no effective node iso defined".into()));
        };
        let Some(surf) = self.get_optic_surface(surf_name) else {
            return Err(OpossumError::Other(format!(
                "no surface with name {surf_name} defined"
            )));
        };
        Ok(eff_node_iso.append(surf.anchor_point_iso()))
    }
    /// Set local alignment (decenter, tilt) of an optical node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `alignment` property cannot be set.
    fn set_alignment(&mut self, decenter: Point3<Length>, tilt: Point3<Angle>) -> OpmResult<()> {
        let align = Isometry::new(decenter, tilt)?;
        self.node_attr_mut().set_alignment(align);
        self.update_surfaces()
    }
    /// Get a refrecne to a global configuration (if any).
    fn global_conf(&self) -> &Option<Arc<Mutex<SceneryResources>>> {
        self.node_attr().global_conf()
    }
    /// Set the global configuration for this [`OpticNode`].
    /// **Note**: This function should normally only be used by [`OpticRef`](crate::optic_ref::OpticRef).
    fn set_global_conf(&mut self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
        let node_attr = self.node_attr_mut();
        node_attr.set_global_conf(global_conf);
    }
    /// Get the ambient refractive index.
    ///
    /// This value is determined by the global configuration. A warning is issued and a default value is returned
    /// if the global config could not be found.
    fn ambient_idx(&self) -> RefractiveIndexType {
        self.global_conf().as_ref().map_or_else(
            || {
                warn!(
                    "could not get ambient medium since global config not found ... using default"
                );
                SceneryResources::default().ambient_refr_index
            },
            |conf| {
                conf.lock()
                    .expect("Mutex lock failed")
                    .ambient_refr_index
                    .clone()
            },
        )
    }

    /// Returns a mutable reference to an [`OpticSurface`] of this [`OpticNode`] with the key `surf_name`
    /// # Attributes
    /// - `surf_name`: name of the optical surface, which is the key in the [`OpticPorts`] hashmap stat stores the surfaces
    fn get_optic_surface_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        self.node_attr_mut()
            .ports_mut()
            .get_optic_surface_mut(&surf_name.to_owned())
    }
    /// Returns a reference to an [`OpticSurface`] of this [`OpticNode`] with the key `surf_name`
    /// # Attributes
    /// - `surf_name`: name of the optical surface, which is the key in the [`OpticPorts`] hashmap stat stores the surfaces
    fn get_optic_surface(&self, surf_name: &str) -> Option<&OpticSurface> {
        self.node_attr()
            .ports()
            .get_optic_surface(&surf_name.to_owned())
    }
}
/// Helper trait for optical elements that can be locally aligned
pub trait Alignable: OpticNode + Sized {
    /// Locally decenter an optical element.
    ///
    /// # Errors
    /// This function will return an error if the given `decenter` values are not finite.
    fn with_decenter(mut self, decenter: Point3<Length>) -> OpmResult<Self> {
        let old_rotation = self
            .isometry()
            .as_ref()
            .map_or_else(Point3::origin, Isometry::rotation);
        let translation_iso = Isometry::new(decenter, old_rotation)?;
        self.node_attr_mut().set_alignment(translation_iso);
        Ok(self)
    }
    /// Locally tilt an optical element.
    ///
    /// # Errors
    /// This function will return an error if the given `decenter` values are not finite.
    fn with_tilt(mut self, tilt: Point3<Angle>) -> OpmResult<Self> {
        let old_translation = self
            .isometry()
            .as_ref()
            .map_or_else(Point3::origin, Isometry::translation);
        let rotation_iso = Isometry::new(old_translation, tilt)?;
        self.node_attr_mut().set_alignment(rotation_iso);
        Ok(self)
    }
    /// Aligns this optical element with respect to another optical element.
    /// Specifically, the center (optical) axes of these to nodes are set on top of each other and the anchor points are separated by a given distance
    /// This helper function allows, e.g., to build a folded telescope (lens + 0° mirror) when the alignment beams propagate off-center through the lens.
    /// Remark: if this function is used, the distance specified at the `connect_nodes` function is ignored
    /// # Returns
    /// This function returns the original Node with updated alignment settings.
    #[must_use]
    fn align_like_node_at_distance(mut self, node_id: Uuid, distance: Length) -> Self {
        self.node_attr_mut()
            .set_align_like_node_at_distance(node_id, distance);
        self
    }
}

///trait to define an LIDT for a node
pub trait LIDT: OpticNode + Analyzable + Sized {
    /// Sets an LIDT value for all surfaces of this node
    ///
    /// # Errors
    ///
    /// This function returns an error if the given LIDT is negative or NaN.
    fn with_lidt(mut self, lidt: Fluence) -> OpmResult<Self> {
        let in_ports = self.ports().names(&PortType::Input);
        let out_ports = self.ports().names(&PortType::Output);

        for port_name in &in_ports {
            if let Some(surf) = self.get_optic_surface_mut(port_name) {
                surf.set_lidt(lidt)?;
            }
        }
        for port_name in &out_ports {
            if let Some(surf) = self.get_optic_surface_mut(port_name) {
                surf.set_lidt(lidt)?;
            }
        }
        self.node_attr_mut().set_lidt(&lidt);
        Ok(self)
    }
}
#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;
    use crate::{degree, millimeter, nodes::Dummy};

    #[test]
    fn set_alignment() {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.1, 0.2, 0.3);
        assert!(node.set_alignment(decenter, tilt).is_ok());
        let alignment = node.node_attr().alignment().clone().unwrap();
        assert_abs_diff_eq!(alignment.translation().x.value, decenter.x.value);
        assert_abs_diff_eq!(alignment.translation().y.value, decenter.y.value);
        assert_abs_diff_eq!(alignment.translation().z.value, decenter.z.value);
        assert_abs_diff_eq!(alignment.rotation().x.value, tilt.x.value);
        assert_abs_diff_eq!(alignment.rotation().y.value, tilt.y.value);
        assert_abs_diff_eq!(alignment.rotation().z.value, tilt.z.value);
    }
    #[test]
    fn effective_node_iso() {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.0, 0.0, 0.0);
        let iso = Isometry::new(decenter, tilt).unwrap();
        node.set_isometry(iso).unwrap();
        let local_trans = millimeter!(4.0, 5.0, 6.0);
        node.set_alignment(local_trans, degree!(0.0, 0.0, 0.0))
            .unwrap();
        let iso = node.effective_node_iso().unwrap();
        assert_abs_diff_eq!(
            iso.translation().x.value,
            decenter.x.value + local_trans.x.value
        );
        assert_abs_diff_eq!(
            iso.translation().y.value,
            decenter.y.value + local_trans.y.value
        );
        assert_abs_diff_eq!(
            iso.translation().z.value,
            decenter.z.value + local_trans.z.value
        );
    }
    #[test]
    fn effective_surface_iso() {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.1, 0.2, 0.3);
        node.set_alignment(decenter, tilt).unwrap();
        let msg = node.effective_surface_iso("input_1").unwrap_err();
        assert_eq!(
            msg.to_string(),
            "Opossum Error:Other:no effective node iso defined"
        );
        node.set_isometry(Isometry::identity()).unwrap();
        let msg = node.effective_surface_iso("wrong").unwrap_err();
        assert_eq!(
            msg.to_string(),
            "Opossum Error:Other:no surface with name wrong defined"
        );
        let iso = node.effective_surface_iso("input_1").unwrap();
        assert_abs_diff_eq!(iso.translation().x.value, decenter.x.value);
        assert_abs_diff_eq!(iso.translation().y.value, decenter.y.value);
        assert_abs_diff_eq!(iso.translation().z.value, decenter.z.value);
    }
}
