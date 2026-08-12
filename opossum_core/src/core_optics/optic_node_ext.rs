use crate::{
    analyzers::propagation_strategy::PropagationStrategy,
    apertures::Aperture,
    coatings::CoatingType,
    core_optics::{NodeAttrExt, OpticNode, PortType, SceneryResources},
    error::{OpmResult, OpossumError},
    geometry::{Plane, geo_surface::GeoSurfaceRef},
    light::{LightData, LightRays, LightResult, Rays},
    nodes::fluence_detector::Fluence,
    refractive_index::RefractiveIndexType,
    utils::{LockExt, geom_transformation::Isometry},
};
use nalgebra::Vector3;
use std::sync::{Arc, Mutex};
use uom::si::f64::{Angle, Length};

/// Extension trait providing advanced physical propagation routines, coordinate
/// transformations, and property distribution that are uniform across all nodes.
pub trait OpticNodeExt {
    /// Return the effective input isometry of this optical node.
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any).
    fn effective_node_iso(&self) -> Option<Isometry>;
    /// Return the effective input isometry of an [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any) and the anchor point isometry.  
    ///
    /// # Errors
    /// This function errors if
    /// - no effective node isometry is defined  
    /// - the surface with the specified name cannot be found
    fn effective_surface_iso(&self, surf_name: &str) -> OpmResult<Isometry>;
    /// Get the ambient refractive index.
    ///
    /// This value is determined by the global configuration. A warning is issued and a default value is returned
    /// if the global config could not be found.
    fn ambient_idx(&self) -> RefractiveIndexType;
    /// Set local alignment (decenter, tilt) of an optical node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `alignment` property cannot be set.
    fn set_alignment(
        &mut self,
        decenter: nalgebra::Point3<Length>,
        tilt: nalgebra::Point3<Angle>,
    ) -> OpmResult<()>;
    /// Set an [`Aperture`] for a given port name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()>;
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
    ) -> OpmResult<()>;
    /// Set the LIDT for a given port name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    fn set_lidt(&mut self, port_type: &PortType, port_name: &str, lidt: Fluence) -> OpmResult<()>;

    /// Update the surfaces of nodes with a single interacting surface. E.g. detectors
    /// # Errors
    /// This function errors if the function `OpticNode::update_surface` fails
    fn update_flat_single_surfaces(&mut self) -> OpmResult<()>;

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
        surf_name: &str,
        geo_surface: GeoSurfaceRef,
        anchor_point_iso: Isometry,
        port_type: &PortType,
    ) -> OpmResult<()>;
    /// define the up-direction of this lightdata's first ray which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process, and only for source nodes.
    ///
    /// # Errors
    ///
    /// which one?
    fn define_up_direction(&self, ray_data: &LightData) -> OpmResult<Vector3<f64>>;
    /// Modifies the current up-direction of a ray, stored in lightdata, which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process.
    ///
    /// # Errors
    /// This function errors if the the lightdata is not geometric
    fn calc_new_up_direction(
        &self,
        ray_data: &LightData,
        up_direction: &mut Vector3<f64>,
    ) -> OpmResult<()>;
    /// Finds a surface by its name and guides the ray bundle through it.
    ///
    /// This function handles the boilerplate of retrieving the correct surface, calculating
    /// the ray count before and after propagation to detect apodization, and logging a warning
    /// if rays were blocked by the surface's aperture.
    ///
    /// # Errors
    ///
    /// This function errors if the specified surface cannot be found, if the geometric propagation fails,
    /// or if the strategy-specific hooks (e.g., fluence evaluation) fail.
    fn pass_through_surface_generic(
        &mut self,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
        backward: bool,
        refraction_intended: bool,
    ) -> OpmResult<()>;
    /// Guides a ray bundle through the volume of a node: in through its entry surface, out through
    /// its exit surface.
    ///
    /// This is the counterpart of [`OpticNodeExt::pass_through_surface_generic`] for the nodes that
    /// enclose a volume of material (lens, wedge, cylindric lens, ...). All of them performed the
    /// very same two-step sequence, which is collected here so that the step in between — the
    /// propagation *inside* the medium — exists in exactly one place. Today nothing happens in
    /// between and the behaviour is identical to calling the two surface passes directly; the
    /// segmentation and the amplification of active media will be added here.
    ///
    /// # Parameters
    ///
    /// * `entry_surf_name`: name of the surface the rays enter through (typically `"input_1"`).
    /// * `exit_surf_name`: name of the surface the rays leave through (typically `"output_1"`).
    /// * `refri_inside`: refractive index of the medium enclosed by the two surfaces. Behind the
    ///   exit surface the node's ambient index is used.
    /// * `rays_bundle`: the ray bundle, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function errors if one of the two surfaces cannot be found or if the geometric
    /// propagation through either of them fails.
    fn pass_through_volume_generic(
        &mut self,
        entry_surf_name: &str,
        exit_surf_name: &str,
        refri_inside: RefractiveIndexType,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()>;
    /// A unified helper function to analyze optical nodes that enclose a volume of material.
    ///
    /// This is the volume counterpart of [`OpticNodeExt::unified_analyze_single_surface_node`]: it
    /// resolves the node's first input and output port, unwraps the incoming ray data, guides it
    /// through the volume via [`OpticNodeExt::pass_through_volume_generic`] and packs the result
    /// back onto the output port. All nodes with two surfaces enclosing a medium (lens, wedge,
    /// cylindric lens, ...) share this body, so their `AnalysisRayTrace::analyze` reduces to reading
    /// the medium's refractive index and delegating here.
    ///
    /// Unlike the single-surface helper this does **not** call `set_light_data`: volume nodes are
    /// never detectors, and the hook would clone the whole ray bundle for nothing.
    ///
    /// # Parameters
    ///
    /// * `incoming_data`: the [`LightResult`] arriving at the node's input port.
    /// * `refri_inside`: refractive index of the enclosed medium.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no input or output port, if the incoming data
    /// is not geometric ray data, or if the propagation through either surface fails.
    fn unified_analyze_volume_node(
        &mut self,
        incoming_data: LightResult,
        refri_inside: RefractiveIndexType,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightResult>;
    /// The ghost focus variant of [`OpticNodeExt::unified_analyze_volume_node`].
    ///
    /// It is a separate function rather than a branch of the ray trace variant because the two
    /// differ in how they treat an unconnected input port: ray tracing yields no output at all,
    /// while a ghost focus analysis still reports the (then empty) bundle on the output port.
    ///
    /// # Parameters
    ///
    /// * `incoming_data`: the [`LightRays`] arriving at the node's input port.
    /// * `refri_inside`: refractive index of the enclosed medium.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no input or output port, or if the
    /// propagation through either surface fails.
    fn unified_analyze_volume_node_ghost_focus(
        &mut self,
        incoming_data: LightRays,
        refri_inside: RefractiveIndexType,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightRays>;
    /// A unified helper function to analyze optical nodes that feature a single interacting surface.
    ///
    /// This function simplifies the implementation of the analysis traits (`Energy`, `RayTrace`, `GhostFocus`)
    /// for simple transmissive nodes like detectors, monitors, or dummy nodes. It automatically:
    /// 1. Extracts the incoming light from the first input port.
    /// 2. Propagates the light through the specified surface using the given [`PropagationStrategy`].
    /// 3. Triggers the internal `set_light_data` hook so detector nodes can store the results for reporting.
    /// 4. Packages the processed light data and maps it to the first output port.
    ///
    /// # Parameters
    /// * `incoming_data`: The [`LightResult`] arriving at the node's input port.
    /// * `strategy`: The analyzer-specific [`PropagationStrategy`] determining thresholds and physical rules.
    /// * `optic_surf_name`: The name of the interacting surface (typically `"input_1"`).
    /// * `refri_after_surf`: An optional refractive index after the surface. Usually `None` for non-refracting detectors.
    ///
    /// # Errors
    /// This function returns an error if the specified optical surface cannot be found or if the underlying
    /// geometric surface propagation fails.
    fn unified_analyze_single_surface_node(
        &mut self,
        incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
    ) -> OpmResult<LightResult>;
}

/// Return the names of the first input and the first output port of `node`.
///
/// Every helper that guides light straight through a node needs this pair, and all of them treat a
/// node without one of the two as a programming error rather than as "nothing to do".
///
/// # Arguments
///
/// * `node` - the node whose ports are looked up.
///
/// # Returns
///
/// The first input port name and the first output port name, in that order.
///
/// # Errors
///
/// This function returns an [`OpossumError::Analysis`] if the node has no input or no output port.
fn first_io_port_names<T: ?Sized + OpticNode>(node: &T) -> OpmResult<(String, String)> {
    let ports = node.ports();
    let in_port = ports
        .names(&PortType::Input)
        .first()
        .cloned()
        .ok_or_else(|| {
            OpossumError::Analysis(format!("No input port found on node '{}'", node.name()))
        })?;
    let out_port = ports
        .names(&PortType::Output)
        .first()
        .cloned()
        .ok_or_else(|| {
            OpossumError::Analysis(format!("No output port found on node '{}'", node.name()))
        })?;
    Ok((in_port, out_port))
}

impl<T: ?Sized + crate::core_optics::node_attr::HasNodeAttr + OpticNode> OpticNodeExt for T {
    fn effective_node_iso(&self) -> Option<Isometry> {
        self.isometry().as_ref().and_then(|iso| {
            self.node_attr()
                .alignment()
                .as_ref()
                .map_or_else(|| Some(*iso), |local_iso| Some(iso.append(local_iso)))
        })
    }

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

    fn ambient_idx(&self) -> RefractiveIndexType {
        self.global_conf().as_ref().map_or_else(
            || {
                log::warn!(
                    "could not get ambient medium since global config not found ... using default"
                );
                SceneryResources::default().ambient_refr_index
            },
            |conf| conf.lock_opm().unwrap().ambient_refr_index.clone(),
        )
    }

    fn set_alignment(
        &mut self,
        decenter: nalgebra::Point3<Length>,
        tilt: nalgebra::Point3<Angle>,
    ) -> OpmResult<()> {
        let align = Isometry::new(decenter, tilt)?;
        self.node_attr_mut().set_alignment(align);
        self.update_surfaces()
    }

    fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_aperture(port_type, port_name, aperture)?;
        self.node_attr_mut().set_ports(ports);
        self.update_surfaces()
    }

    fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_coating(port_type, port_name, coating)?;
        self.node_attr_mut().set_ports(ports);
        self.update_surfaces()
    }

    fn set_lidt(&mut self, port_type: &PortType, port_name: &str, lidt: Fluence) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_lidt(port_type, port_name, lidt)?;
        self.node_attr_mut().set_ports(ports);
        self.update_surfaces()
    }

    fn update_flat_single_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));
        self.update_surface(
            "input_1",
            geosurface.clone(),
            Isometry::identity(),
            &PortType::Input,
        )?;
        self.update_surface(
            "output_1",
            geosurface,
            Isometry::identity(),
            &PortType::Output,
        )?;
        Ok(())
    }

    fn update_surface(
        &mut self,
        surf_name: &str,
        geo_surface: GeoSurfaceRef,
        anchor_point_iso: Isometry,
        port_type: &PortType,
    ) -> OpmResult<()> {
        let config = {
            let mut ports = self.ports();
            if ports.ports(port_type).get(surf_name).is_none() {
                let _ = ports.add(port_type, surf_name);
                self.node_attr_mut().set_ports(ports.clone());
            }
            ports
                .ports_raw(port_type)
                .get(surf_name)
                .cloned()
                .ok_or_else(|| {
                    OpossumError::Other(format!(
                        "Port config for surface {port_type}/{surf_name} of node '{}' not found.",
                        self.name()
                    ))
                })?
        };

        if let Some(optic_surf) = self.get_optic_surface_mut(surf_name) {
            optic_surf.set_geo_surface(geo_surface);
            optic_surf.set_anchor_point_iso(anchor_point_iso);
            optic_surf.set_aperture(config.aperture);
            optic_surf.set_coating(config.coating);
            optic_surf.set_lidt(*config.lidt.get())?;
        } else {
            let mut optic_surf = crate::core_optics::optic_surface::OpticSurface::new(
                geo_surface,
                config.coating,
                config.aperture,
                *config.lidt.get(),
            )?;
            optic_surf.set_anchor_point_iso(anchor_point_iso);
            let runtime = self.node_attr_mut().runtime_surfaces_mut();
            match port_type {
                PortType::Input => runtime.inputs.insert(surf_name.to_string(), optic_surf),
                PortType::Output => runtime.outputs.insert(surf_name.to_string(), optic_surf),
            };
        }
        Ok(())
    }

    fn define_up_direction(&self, ray_data: &LightData) -> OpmResult<Vector3<f64>> {
        if let LightData::Geometric(rays) = ray_data {
            rays.define_up_direction()
        } else {
            Err(OpossumError::Other(
                "Wrong light data for \"up-direction\" definition".into(),
            ))
        }
    }

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

    fn pass_through_surface_generic(
        &mut self,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
        backward: bool,
        refraction_intended: bool,
    ) -> OpmResult<()> {
        let uuid = self.node_attr().uuid();
        let iso = self.effective_surface_iso(optic_surf_name)?;
        let node_name = self.name().to_string();
        let node_type = self.node_type().to_string();

        let Some(surf) = self.get_optic_surface_mut(optic_surf_name) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{node_name}\""
            )));
        };
        let rays_before: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        surf.propagate_rays(
            rays_bundle,
            uuid,
            &iso,
            refri_after_surf.as_ref(),
            backward,
            refraction_intended,
            strategy,
        )?;
        let rays_after: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        if rays_after < rays_before {
            self.set_apodization_warning(true);
            log::warn!(
                "Rays have been apodized at input aperture of '{node_name}' ({node_type}). Results might not be accurate."
            );
        }
        Ok(())
    }

    fn pass_through_volume_generic(
        &mut self,
        entry_surf_name: &str,
        exit_surf_name: &str,
        refri_inside: RefractiveIndexType,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let backward = self.inverted();
        // The rays are meant to be refracted at both surfaces of the volume, not just traced to them.
        let refraction_intended = true;
        self.pass_through_surface_generic(
            entry_surf_name,
            Some(refri_inside),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )?;
        // Inside the medium nothing happens yet. This is where the segmentation of the inner path
        // and the evaluation of an active medium's gain model will be inserted.
        self.pass_through_surface_generic(
            exit_surf_name,
            Some(self.ambient_idx()),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )
    }

    fn unified_analyze_volume_node(
        &mut self,
        mut incoming_data: LightResult,
        refri_inside: RefractiveIndexType,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightResult> {
        let (in_port_name, out_port_name) = first_io_port_names(self)?;
        let Some(data) = incoming_data.remove(&in_port_name) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(rays) = data else {
            return Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ));
        };
        let mut rays_bundle = vec![rays];
        self.pass_through_volume_generic(
            &in_port_name,
            &out_port_name,
            refri_inside,
            &mut rays_bundle,
            strategy,
        )?;
        Ok(LightResult::from([(
            out_port_name,
            LightData::Geometric(rays_bundle.remove(0)),
        )]))
    }

    fn unified_analyze_volume_node_ghost_focus(
        &mut self,
        incoming_data: LightRays,
        refri_inside: RefractiveIndexType,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightRays> {
        let (in_port_name, out_port_name) = first_io_port_names(self)?;
        let mut rays_bundle = incoming_data
            .get(&in_port_name)
            .map_or_else(Vec::<Rays>::new, Clone::clone);
        self.pass_through_volume_generic(
            &in_port_name,
            &out_port_name,
            refri_inside,
            &mut rays_bundle,
            strategy,
        )?;
        Ok(LightRays::from([(out_port_name, rays_bundle)]))
    }

    fn unified_analyze_single_surface_node(
        &mut self,
        mut incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
    ) -> OpmResult<LightResult> {
        let (in_port_name, out_port_name) = first_io_port_names(self)?;
        let Some(data) = incoming_data.remove(&in_port_name) else {
            return Ok(LightResult::default());
        };

        match data {
            LightData::Geometric(rays) => {
                let mut rays_bundle = vec![rays];
                self.pass_through_surface_generic(
                    optic_surf_name,
                    refri_after_surf,
                    &mut rays_bundle,
                    strategy,
                    false,
                    true,
                )?;
                let out_data = LightData::Geometric(rays_bundle.remove(0));
                self.set_light_data(Some(out_data.clone()));
                Ok(LightResult::from([(out_port_name, out_data)]))
            }
            LightData::GhostFocus(mut rays_bundle) => {
                self.pass_through_surface_generic(
                    optic_surf_name,
                    refri_after_surf,
                    &mut rays_bundle,
                    strategy,
                    false,
                    true,
                )?;
                let out_data = LightData::GhostFocus(rays_bundle);
                self.set_light_data(Some(out_data.clone()));
                Ok(LightResult::from([(out_port_name, out_data)]))
            }
            LightData::Energy(energy) => {
                let out_data = LightData::Energy(energy);
                self.set_light_data(Some(out_data.clone()));
                Ok(LightResult::from([(out_port_name, out_data)]))
            }
            LightData::Fourier => Ok(LightResult::default()),
        }
    }
}
