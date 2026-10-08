use nalgebra::Vector3;
use uom::si::f64::{Angle, Length};

use crate::{
    analyzers::propagation_strategy::PropagationStrategy,
    apertures::{Aperture, ApertureShape, ApertureType},
    coatings::CoatingType,
    core_optics::{NodeAttrExt, OpticNode, PortType},
    error::{OpmResult, OpossumError},
    geometry::{Rim, body::CLEAR_APERTURE, geo_surface::GeoSurfaceRef},
    light::{LightData, LightResult, Rays},
    nodes::fluence_detector::Fluence,
    properties::Proptype,
    refractive_index::RefractiveIndexType,
    types::validated_type_definitions::ValidatedCrossSection,
    utils::geom_transformation::Isometry,
};

/// Extension trait providing advanced physical propagation routines, coordinate
/// transformations, and property distribution that are uniform across all nodes.
pub trait OpticNodeExt {
    /// Return the effective input isometry of this optical node.
    ///
    /// The effective input isometry is the effective base positioning (either absolute or cached automatic)
    /// modified by the local alignment isometry (if present). Returns `None` if the node has not yet been placed.
    fn effective_node_iso(&self) -> Option<Isometry>;

    /// Return the effective input isometry of an [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any) and the anchor point isometry.  
    ///
    /// # Errors
    ///
    /// This function returns an error if:
    /// - no effective node isometry is defined
    /// - the surface with the specified name cannot be found
    fn effective_surface_iso(&self, surf_name: &str) -> OpmResult<Isometry>;

    /// Return the transversal extent of this node, read from its [`CLEAR_APERTURE`] property.
    ///
    /// The port [`Aperture`]s are deliberately not consulted: an aperture states how much light a
    /// surface transmits where and may soften or invert that transmission, which says nothing about
    /// how far the component reaches. Masking a component down does not make it smaller.
    ///
    /// # Returns
    ///
    /// The extent, or `None` if the clear aperture is open: the node is unbounded, as a detector
    /// that records all light.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not declare a clear aperture at all or if
    /// that clear aperture neither is open nor delimits a region, which leaves the extent
    /// undefined.
    fn clear_aperture(&self) -> OpmResult<Option<ValidatedCrossSection>>;

    /// Set local alignment (decenter, tilt) of an optical node and update its optical surfaces.
    ///
    /// # Errors
    ///
    /// This function returns an error if constructing the alignment isometry fails or if `update_surfaces` fails.
    fn set_alignment(
        &mut self,
        decenter: nalgebra::Point3<Length>,
        tilt: nalgebra::Point3<Angle>,
    ) -> OpmResult<()>;

    /// Clears any local alignment (decenter, tilt) and updates optical surfaces.
    ///
    /// # Errors
    ///
    /// This function returns an error if `update_surfaces` fails.
    fn clear_alignment(&mut self) -> OpmResult<()>;

    /// Set an [`Aperture`] for a given port name.
    ///
    /// # Errors
    ///
    /// This function returns an error if the port name does not exist.
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
    /// This function returns an error if the port name does not exist.
    fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()>;

    /// Set the LIDT for a given port name.
    ///
    /// # Errors
    ///
    /// This function returns an error if the port name does not exist.
    fn set_lidt(&mut self, port_type: &PortType, port_name: &str, lidt: Fluence) -> OpmResult<()>;

    /// Build the surfaces of this node's [`Geometry`](crate::geometry::Geometry) and install them
    /// under the given ports.
    ///
    /// The entrance surface is installed under every input port, the exit surface under every
    /// output port. A single surface is both, so all ports then share it.
    ///
    /// # Arguments
    ///
    /// * `inputs` - the input ports light enters through, by their (physical) names.
    /// * `outputs` - the output ports light leaves through, by their (physical) names.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no geometry, if the geometry cannot be
    /// derived or built, or if a surface cannot be installed.
    fn install_geometry(&mut self, inputs: &[&str], outputs: &[&str]) -> OpmResult<()>;

    /// Updates a single surface of this node, adding its port if the node does not have it yet.
    ///
    /// # Arguments
    ///
    /// * `surf_name` - name of the surface, which is the physical name of its port.
    /// * `geo_surface` - the geometric surface reference [`GeoSurfaceRef`].
    /// * `anchor_point_iso` - the isometry of the geometrical anchor point.
    /// * `rim` - the lateral boundary of the component, or `None` for an unbounded surface.
    /// * `port_type` - the physical port type (`Input` or `Output`) of this surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if surface creation or registration fails.
    fn update_surface(
        &mut self,
        surf_name: &str,
        geo_surface: GeoSurfaceRef,
        anchor_point_iso: Isometry,
        rim: Option<Rim>,
        port_type: &PortType,
    ) -> OpmResult<()>;

    /// Defines the up-direction of this light data's first ray, needed to create an isometry from this ray.
    ///
    /// This function should only be used during the node positioning process, and only for source nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if `ray_data` is not geometric light data or contains no rays.
    fn define_up_direction(&self, ray_data: &LightData) -> OpmResult<Vector3<f64>>;

    /// Modifies the current up-direction of a ray, stored in light data, which is needed to create an isometry from this ray.
    ///
    /// This function should only be used during the node positioning process.
    ///
    /// # Errors
    ///
    /// This function returns an error if the light data is not geometric.
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
    /// # Returns
    ///
    /// For every bundle in `rays_bundle` after the call, whether each of its rays hit the surface
    /// (see [`OpticSurface::propagate_rays`](crate::core_optics::optic_surface::OpticSurface::propagate_rays)).
    /// A node that processes the light further after the surface does so only for those rays.
    ///
    /// # Errors
    ///
    /// This function returns an error if the specified surface cannot be found, if geometric propagation fails,
    /// or if strategy-specific hooks (e.g., fluence evaluation) fail.
    fn pass_through_surface_generic(
        &mut self,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
        backward: bool,
        refraction_intended: bool,
    ) -> OpmResult<Vec<Vec<bool>>>;

    /// A unified helper function to analyze optical nodes that feature a single interacting surface.
    ///
    /// This function simplifies the implementation of the analysis traits (`Energy`, `RayTrace`, `GhostFocus`)
    /// for simple transmissive nodes like detectors, monitors, or dummy nodes. It automatically:
    /// 1. Extracts incoming light from the single input port.
    /// 2. Propagates light through the specified surface using the given [`PropagationStrategy`].
    /// 3. Triggers the internal `set_light_data` hook so detector nodes can store results for reporting.
    /// 4. Packages the processed light data and maps it to the single output port.
    ///
    /// # Parameters
    /// * `incoming_data`: The [`LightResult`] arriving at the node's input port.
    /// * `strategy`: The analyzer-specific [`PropagationStrategy`] determining thresholds and physical rules.
    /// * `optic_surf_name`: The name of the interacting surface (typically `"input_1"`).
    /// * `refri_after_surf`: An optional refractive index after the surface. Usually `None` for non-refracting detectors.
    ///
    /// # Errors
    ///
    /// This function returns an error if the specified optical surface cannot be found or if geometric surface propagation fails.
    fn unified_analyze_single_surface_node(
        &mut self,
        incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
    ) -> OpmResult<LightResult>;

    /// Cut copies of the ray bundles that passed a surface to what the surface records.
    ///
    /// A detector records only the light within its window (see
    /// [`OpticSurface::clip_to_window`](crate::core_optics::optic_surface::OpticSurface::clip_to_window)),
    /// while all of it passes on. If rays were not recorded, a positioning run warns that the
    /// optical axis misses the detector; any other analysis warns that the result may be
    /// incomplete and sets the node's apodization warning.
    ///
    /// # Arguments
    ///
    /// * `optic_surf_name` - the name of the recording surface.
    /// * `bundles` - copies of the bundles that passed the surface; rays outside the window are
    ///   invalidated.
    /// * `strategy` - the strategy of the current analysis.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface cannot be found or a ray cannot be cut.
    fn record_within_window(
        &mut self,
        optic_surf_name: &str,
        bundles: &mut [Rays],
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()>;
    /// Warn if rays were lost while passing this node and set its apodization warning.
    ///
    /// # Arguments
    ///
    /// * `rays_before` - the number of valid rays that reached the node.
    /// * `rays_after` - the number of valid rays that left it.
    fn warn_about_lost_rays(&mut self, rays_before: usize, rays_after: usize);
}

/// Return the names of the one input and the one output port of `node`.
///
/// Every helper that guides light straight through a node needs this pair, and all of them are
/// written for components with exactly one of each.
///
/// A node with several inputs or outputs has to decide for itself which port feeds which - there is
/// no general answer, and picking one silently would be wrong rather than merely imprecise. Note
/// that "first" would not even mean "first declared": [`OpticPorts`](crate::core_optics::OpticPorts)
/// stores its ports in a `BTreeMap`, so any such pick would follow the alphabetical order of the port names.
/// Multi-port nodes therefore implement `analyze` themselves and address their ports by name - see
/// [`BeamSplitter`](crate::nodes::BeamSplitter), which additionally swaps them when inverted. This
/// function refuses those nodes instead of guessing.
///
/// # Arguments
///
/// * `node` - the node whose ports are looked up.
///
/// # Returns
///
/// The input port name and the output port name, in that order.
///
/// # Errors
///
/// This function returns an [`OpossumError::Analysis`] if the node does not have exactly one input
/// and exactly one output port.
pub(crate) fn single_io_port_names<T: ?Sized + OpticNode>(node: &T) -> OpmResult<(String, String)> {
    let ports = node.ports();
    let single_port = |port_type: &PortType| -> OpmResult<String> {
        let names = ports.names(port_type);
        if let [name] = names.as_slice() {
            return Ok(name.clone());
        }
        Err(OpossumError::Analysis(format!(
            "node '{}' ({}) has {} {port_type} ports, but this analysis path is only defined for \
             exactly one - a node with several ports must implement `analyze` itself and address \
             its ports by name",
            node.name(),
            node.node_type(),
            names.len(),
        )))
    };
    Ok((
        single_port(&PortType::Input)?,
        single_port(&PortType::Output)?,
    ))
}

impl<T: ?Sized + crate::core_optics::node_attr::HasNodeAttr + OpticNode> OpticNodeExt for T {
    fn effective_node_iso(&self) -> Option<Isometry> {
        self.effective_position().map(|iso| {
            self.node_attr()
                .alignment()
                .as_ref()
                .map_or(*iso, |local_iso| iso.append(local_iso))
        })
    }

    fn effective_surface_iso(&self, surf_name: &str) -> OpmResult<Isometry> {
        let Some(eff_node_iso) = self.effective_node_iso() else {
            return Err(OpossumError::Other("no effective node iso defined".into()));
        };
        let surf = self.get_optic_surface(surf_name).ok_or_else(|| {
            OpossumError::Other(format!("no surface with name {surf_name} defined"))
        })?;
        Ok(eff_node_iso.append(surf.anchor_point_iso()))
    }

    fn clear_aperture(&self) -> OpmResult<Option<ValidatedCrossSection>> {
        let Ok(Proptype::Aperture(clear_aperture)) = self.node_attr().get_property(CLEAR_APERTURE)
        else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no '{CLEAR_APERTURE}' property, so its extent is unknown",
                self.name()
            )));
        };
        if matches!(clear_aperture, ApertureShape::Open) {
            return Ok(None);
        }
        let aperture = Aperture::new(clear_aperture.clone(), ApertureType::Hole, None, None)?;
        ValidatedCrossSection::try_new(aperture)
            .map(Some)
            .map_err(|e| {
                OpossumError::Other(format!(
                    "the {CLEAR_APERTURE} of node '{}' does not bound a region: {e}",
                    self.name()
                ))
            })
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

    fn clear_alignment(&mut self) -> OpmResult<()> {
        self.node_attr_mut().set_alignment_option(None);
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

    fn install_geometry(&mut self, inputs: &[&str], outputs: &[&str]) -> OpmResult<()> {
        let Some(geometry) = self.geometry()? else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no geometry to install surfaces from",
                self.name()
            )));
        };
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let ((entrance, entrance_anchor), (exit, exit_anchor)) =
            geometry.entrance_and_exit(&node_iso)?;
        let rim = geometry.rim()?;
        for name in inputs {
            self.update_surface(
                name,
                entrance.clone(),
                entrance_anchor,
                rim.clone(),
                &PortType::Input,
            )?;
        }
        for name in outputs {
            self.update_surface(
                name,
                exit.clone(),
                exit_anchor,
                rim.clone(),
                &PortType::Output,
            )?;
        }
        Ok(())
    }

    fn update_surface(
        &mut self,
        surf_name: &str,
        geo_surface: GeoSurfaceRef,
        anchor_point_iso: Isometry,
        rim: Option<Rim>,
        port_type: &PortType,
    ) -> OpmResult<()> {
        let config = {
            let mut ports = self.node_attr().raw_ports().clone();
            if ports.ports_raw(port_type).get(surf_name).is_none() {
                ports.add(port_type, surf_name)?;
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
            optic_surf.set_rim(rim);
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
            optic_surf.set_rim(rim);
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
    ) -> OpmResult<Vec<Vec<bool>>> {
        let uuid = self.node_attr().uuid();
        let iso = self.effective_surface_iso(optic_surf_name)?;
        let node_name = self.name().to_string();

        let Some(surf) = self.get_optic_surface_mut(optic_surf_name) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{node_name}\""
            )));
        };
        let rays_before: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        let hits = surf.propagate_rays(
            rays_bundle,
            uuid,
            &iso,
            refri_after_surf.as_ref(),
            backward,
            refraction_intended,
            strategy,
        )?;
        let rays_after: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        self.warn_about_lost_rays(rays_before, rays_after);
        Ok(hits)
    }

    fn unified_analyze_single_surface_node(
        &mut self,
        mut incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
        refri_after_surf: Option<RefractiveIndexType>,
    ) -> OpmResult<LightResult> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
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
                let mut recorded = rays_bundle.clone();
                self.record_within_window(optic_surf_name, &mut recorded, strategy)?;
                self.set_light_data(Some(LightData::Geometric(recorded.remove(0))));
                let out_data = LightData::Geometric(rays_bundle.remove(0));
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
                let mut recorded = rays_bundle.clone();
                self.record_within_window(optic_surf_name, &mut recorded, strategy)?;
                self.set_light_data(Some(LightData::GhostFocus(recorded)));
                let out_data = LightData::GhostFocus(rays_bundle);
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

    fn record_within_window(
        &mut self,
        optic_surf_name: &str,
        bundles: &mut [Rays],
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let Some(surface) = self.get_optic_surface(optic_surf_name) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{}\"",
                self.name()
            )));
        };
        let mut cut = false;
        for rays in bundles.iter_mut() {
            cut |= surface.clip_to_window(rays)?;
        }
        if cut {
            let (name, node_type) = (self.name(), self.node_type());
            if strategy.is_positioning_run() {
                log::warn!(
                    "the optical axis misses the clear aperture of detector '{name}' ({node_type}); its measurement may fail"
                );
            } else {
                log::warn!(
                    "rays passed '{name}' ({node_type}) outside its clear aperture and were not recorded. Results might not be accurate."
                );
                self.set_apodization_warning(true);
            }
        }
        Ok(())
    }

    fn warn_about_lost_rays(&mut self, rays_before: usize, rays_after: usize) {
        if rays_after < rays_before {
            self.set_apodization_warning(true);
            log::warn!(
                "Rays were lost at '{}' ({}): they ran outside its clear aperture, were cut by a port aperture or fell below the energy threshold. Results might not be accurate.",
                self.name(),
                self.node_type()
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        J_per_cm2,
        analyzers::RayTraceConfig,
        core_optics::{
            node_attr::{HasNodeAttr, NodePositioning},
            optic_node::LIDT,
        },
        degree, millimeter,
        nodes::{BeamSplitter, Dummy},
    };
    use approx::assert_abs_diff_eq;

    #[test]
    fn effective_node_iso_and_clear_alignment() -> OpmResult<()> {
        let mut node = Dummy::default();

        // Initially Automatic(None), so effective_node_iso must be None
        assert_eq!(node.effective_node_iso(), None);

        // Set absolute position
        let base_pos = millimeter!(10.0, 20.0, 30.0);
        let base_rot = degree!(0.0, 0.0, 0.0);
        let base_iso = Isometry::new(base_pos, base_rot)?;
        node.set_positioning(NodePositioning::Absolute(base_iso))?;

        assert_eq!(node.effective_node_iso(), Some(base_iso));

        // Add local alignment
        let align_trans = millimeter!(1.0, 2.0, 3.0);
        let align_rot = degree!(0.0, 0.0, 0.0);
        node.set_alignment(align_trans, align_rot)?;

        let eff_iso = node
            .effective_node_iso()
            .expect("must have effective isometry");
        assert_abs_diff_eq!(eff_iso.translation().x.value, 11.0e-3);
        assert_abs_diff_eq!(eff_iso.translation().y.value, 22.0e-3);
        assert_abs_diff_eq!(eff_iso.translation().z.value, 33.0e-3);

        // Clear local alignment
        node.clear_alignment()?;
        assert_eq!(node.effective_node_iso(), Some(base_iso));
        assert!(node.node_attr().alignment().is_none());

        Ok(())
    }

    /// A beam splitter has two inputs and two outputs, and which one feeds which is its own
    /// decision - that is why it implements `analyze` itself. Reaching one of the unified helpers
    /// with such a node is a programming error, and it has to say so instead of silently picking
    /// the alphabetically first port.
    #[test]
    fn unified_helpers_reject_a_multi_port_node() {
        let mut node = BeamSplitter::default();
        let err = node
            .unified_analyze_single_surface_node(
                LightResult::default(),
                &RayTraceConfig::default(),
                "input_1",
                None,
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("only defined for exactly one"),
            "expected a multi-port rejection, got: {err}"
        );
    }

    /// The counterpart: a node with exactly one input and one output resolves both ports and only
    /// then finds there is nothing on the input - so the guard above cannot be satisfied vacuously.
    #[test]
    fn unified_helpers_accept_a_single_port_node() -> OpmResult<()> {
        let mut node = Dummy::default();
        let out = node.unified_analyze_single_surface_node(
            LightResult::default(),
            &RayTraceConfig::default(),
            "input_1",
            None,
        )?;
        assert!(out.is_empty());
        Ok(())
    }

    /// Setting a port's aperture, coating or LIDT addresses an inverted node's ports by their
    /// logical names, but the node has to keep storing them physically: otherwise it stays
    /// inverted after the inversion is reverted, and every lookup of a physical port misses.
    #[test]
    fn port_settings_keep_the_stored_ports_of_an_inverted_node_physical() -> OpmResult<()> {
        let inverted = || -> OpmResult<Dummy> {
            let mut node = Dummy::default();
            node.set_inverted(true)?;
            Ok(node)
        };
        let lidt = J_per_cm2!(2.0);
        let mut with_aperture = inverted()?;
        with_aperture.set_aperture(&PortType::Input, "output_1", &Aperture::default())?;
        let mut with_coating = inverted()?;
        with_coating.set_coating(&PortType::Input, "output_1", &CoatingType::Fresnel)?;
        let mut with_lidt = inverted()?;
        with_lidt.set_lidt(&PortType::Input, "output_1", lidt)?;
        for mut node in [
            with_aperture,
            with_coating,
            with_lidt,
            inverted()?.with_lidt(lidt)?,
        ] {
            assert_eq!(
                node.node_attr().raw_ports().names(&PortType::Input),
                vec!["input_1"]
            );
            node.set_inverted(false)?;
            assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        }
        Ok(())
    }
}
