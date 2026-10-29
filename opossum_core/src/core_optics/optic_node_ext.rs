use uom::si::f64::{Angle, Length};

use crate::{
    analyzers::propagation_strategy::PropagationStrategy,
    apertures::Outline,
    coatings::{CoatingType, SplittingConfigBuilder},
    core_optics::{
        NodeAttrExt, OpticNode, OpticPorts, PortType, SurfaceConfig, SurfacePorts,
        optic_surface::OpticSurface,
        surface_effect::{Refraction, SurfaceEffect},
    },
    error::{OpmResult, OpossumError},
    geometry::{
        FaceIndex, FacePatch, Geometry,
        body::{OPTIC_SHAPE, default_optic_shape},
    },
    light::{LightData, LightRays, LightResult, Rays, rays::ReflectedPart},
    material::{MATERIAL, Material},
    nodes::fluence_detector::Fluence,
    properties::{Proptype, proptype::AssetRef, validator::Validator},
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
    /// It is the frame of the vertex of the face the surface is, placed with the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if:
    /// - no effective node isometry is defined
    /// - the surface with the specified name cannot be found
    fn effective_surface_iso(&self, surf_name: &str) -> OpmResult<Isometry>;

    /// Return the transversal extent of this node, read from its [`OPTIC_SHAPE`] property.
    ///
    /// # Returns
    ///
    /// The outline of the optic shape, or `None` if the optic shape is open: the node is
    /// unbounded, as a detector that records all light.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not declare an optic shape at all.
    fn cross_section(&self) -> OpmResult<Option<Outline>>;

    /// Return the material this node's volume is filled with, read from its [`MATERIAL`] property.
    ///
    /// It is handed out as stored: embedded, or still referenced while a document is being loaded.
    /// The node puts it into the solid it describes (see [`OpticNode::create_geometry`]); whoever
    /// works with the volume asks that solid (see
    /// [`Body::material`](crate::geometry::body::Body::material)).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no [`MATERIAL`] property holding a material.
    fn material(&self) -> OpmResult<AssetRef<Material>>;

    /// Return triangle meshes of this node's geometry, each with the face it lies on, to draw it
    /// (see [`GeometryKind::mesh`](crate::geometry::GeometryKind::mesh)).
    ///
    /// The geometry is derived from the node's properties as they are now, so the meshes follow a
    /// changed property even if the node's surfaces have not been rebuilt since. They are stated
    /// in the node's own frame, so moving the node leaves them untouched. A face without an edge of
    /// its own is drawn over the default optic shape.
    ///
    /// # Arguments
    ///
    /// * `segments` - the number of points to sample the rim of an outline with.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no geometry, if it cannot be derived or if it
    /// cannot be meshed.
    fn geometry_mesh(&self, segments: usize) -> OpmResult<Vec<FacePatch>>;

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

    /// Return the name of one face of this node's geometry, which names the property holding its
    /// [`SurfaceConfig`] (see [`GeometryKind::face_name`](crate::geometry::GeometryKind::face_name)).
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry cannot be derived or has no such face.
    fn face_name(&self, face: FaceIndex) -> OpmResult<String>;

    /// Return the coating and the LIDT of one surface of this node.
    ///
    /// # Arguments
    ///
    /// * `face_name` - the name of the face (see [`OpticNodeExt::face_name`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no surface property of that name.
    fn surface_config(&self, face_name: &str) -> OpmResult<SurfaceConfig>;

    /// Create a [`SurfaceConfig`] property with the default coating and LIDT for every face of
    /// this node's geometry, named after the face.
    ///
    /// The property only takes coatings that fit its face (see [`Validator::SurfaceCoating`]): a
    /// splitting face (see [`SurfacePorts::splitting`]) only a separator, by default one that
    /// transmits half of the light; every other face any other coating, by default an ideal
    /// antireflective one. A node calls this once when it is created, after the properties its
    /// geometry is derived from. A node without a shape of its own gets none.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry cannot be derived or a property of that name
    /// exists already.
    fn create_default_surface_properties(&mut self) -> OpmResult<()>;

    /// Set the coating of one surface of this node, keeping its LIDT.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry has no such face or the surfaces cannot be
    /// rebuilt.
    fn set_coating(&mut self, face: FaceIndex, coating: &CoatingType) -> OpmResult<()>;

    /// Set the LIDT of one surface of this node, keeping its coating.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry has no such face, the LIDT is negative or
    /// NaN, or the surfaces cannot be rebuilt.
    fn set_lidt(&mut self, face: FaceIndex, lidt: Fluence) -> OpmResult<()>;

    /// Create this node's [`Geometry`](crate::geometry::Geometry) from its current properties (see
    /// [`OpticNode::create_geometry`]) and move it to where the node sits: its position followed by
    /// its alignment, or the identity for a node that has not been placed.
    ///
    /// # Returns
    ///
    /// The placed geometry, or `None` for a node without a shape of its own.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry cannot be derived from the properties.
    fn placed_geometry(&self) -> OpmResult<Option<Geometry>>;

    /// Move this node's [`Geometry`](crate::geometry::Geometry) to where the node sits, keep it as
    /// the node's runtime geometry and install a surface on one of its faces under each of the given
    /// ports, with the coating and the LIDT of that face (see [`OpticNodeExt::surface_config`]).
    /// A port the node does not have yet is added.
    ///
    /// The node decides which face of its geometry every port sits on; several ports may share a
    /// face. The volume a solid encloses is the node's medium; a single surface leaves the node
    /// without one. The installation replaces the one before, with everything an analysis left on
    /// it.
    ///
    /// # Arguments
    ///
    /// * `ports` - the ports of the node, each with the face it sits on (see
    ///   [`OpticNode::surface_ports`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no geometry, if the geometry cannot be
    /// derived or built, if a port names a face the geometry does not have, or if the node has no
    /// surface property for a face.
    fn install_geometry(&mut self, ports: &SurfacePorts) -> OpmResult<()>;

    /// Return the face of this node's geometry the port of the given (physical) name sits on (see
    /// [`OpticNode::surface_ports`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no such port.
    fn face_of_port(&self, port: &str) -> OpmResult<FaceIndex>;

    /// Return the optic surface the port of the given (physical) name sits on, or `None` if the
    /// node has no such port or no runtime geometry.
    fn get_optic_surface(&self, port: &str) -> Option<&OpticSurface>;

    /// Return the optic surface the port of the given (physical) name sits on, as
    /// [`OpticNodeExt::get_optic_surface`] does, mutably.
    fn get_optic_surface_mut(&mut self, port: &str) -> Option<&mut OpticSurface>;

    /// Lets a surface of this node act on the ray bundles of an analysis (see
    /// [`Geometry::propagate`]) and warns if rays were lost there.
    ///
    /// # Arguments
    ///
    /// * `optic_surf_name` - the (physical) name of the port whose surface acts.
    /// * `effect` - what the surface does to a ray that hits it.
    /// * `rays_bundle` - the bundles of the analysis.
    /// * `strategy` - the analysis.
    /// * `backward` - whether the light runs through the node backward.
    /// * `part` - what the reflected part is to each bundle.
    ///
    /// # Errors
    ///
    /// This function returns an error if the specified surface cannot be found, if geometric propagation fails,
    /// or if strategy-specific hooks (e.g., fluence evaluation) fail.
    fn pass_through_surface_generic(
        &mut self,
        optic_surf_name: &str,
        effect: &dyn SurfaceEffect,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
        backward: bool,
        part: ReflectedPart,
    ) -> OpmResult<()>;

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
    ///
    /// # Errors
    ///
    /// This function returns an error if the specified optical surface cannot be found or if geometric surface propagation fails.
    fn unified_analyze_single_surface_node(
        &mut self,
        incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
    ) -> OpmResult<LightResult>;

    /// Guides a ray bundle through the volume of a node: in through its entry surface, through the
    /// medium, out through its exit surface.
    ///
    /// This is the counterpart of [`OpticNodeExt::pass_through_surface_generic`] for the nodes whose
    /// geometry encloses a volume of material (lens, wedge, cylindric lens, ...). The rays are
    /// refracted into the material at the entry surface and out into the ambient medium at the exit
    /// surface; what is reflected there is a ghost. In between, the medium acts on them (see
    /// [`Body::propagate_inside`](crate::geometry::body::Body)) with the gain model the operating
    /// point runs this node with - except in a positioning run, which has no operating point.
    ///
    /// # Parameters
    ///
    /// * `entry_surf_name`: name of the surface the rays enter through (typically `"input_1"`).
    /// * `exit_surf_name`: name of the surface the rays leave through (typically `"output_1"`).
    /// * `rays_bundle`: the ray bundle, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function errors if the node's medium is not prepared or holds no single
    /// [`Material`], if one of the two surfaces cannot be found, or if the propagation through
    /// either of them or through the medium fails.
    fn pass_through_volume_generic(
        &mut self,
        entry_surf_name: &str,
        exit_surf_name: &str,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()>;

    /// A unified helper function to analyze optical nodes that enclose a volume of material.
    ///
    /// This is the volume counterpart of [`OpticNodeExt::unified_analyze_single_surface_node`]: it
    /// resolves the node's single input and output port, unwraps the incoming ray data, guides it
    /// through the volume via [`OpticNodeExt::pass_through_volume_generic`] and packs the result
    /// back onto the output port.
    ///
    /// Unlike the single-surface helper this does **not** call `set_light_data`: volume nodes are
    /// never detectors, and the hook would clone the whole ray bundle for nothing.
    ///
    /// # Parameters
    ///
    /// * `incoming_data`: the [`LightResult`] arriving at the node's input port.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly one input and one output
    /// port, if the incoming data is not geometric ray data, or if the propagation through the
    /// volume fails.
    fn unified_analyze_volume_node(
        &mut self,
        incoming_data: LightResult,
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
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly one input and one output
    /// port, or if the propagation through the volume fails.
    fn unified_analyze_volume_node_ghost_focus(
        &mut self,
        incoming_data: LightRays,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightRays>;

    /// Return whether this node's geometry encloses a volume of material.
    ///
    /// The geometry is the one answer to that: a node that describes a solid is a volume node, one
    /// that describes a single surface or no shape at all is not.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node's geometry cannot be derived.
    fn encloses_volume(&self) -> OpmResult<bool>;

    /// Cut copies of the ray bundles that passed a surface to what the surface records.
    ///
    /// A detector records only the light within the window of its geometry (see
    /// [`GeometryKind::window`](crate::geometry::GeometryKind::window)), while all of it passes on. If rays were not recorded, a positioning run warns that the
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
        if self.effective_node_iso().is_none() {
            return Err(OpossumError::Other("no effective node iso defined".into()));
        }
        let face = self.surface_ports().face_of(surf_name).ok_or_else(|| {
            OpossumError::Other(format!("no surface with name {surf_name} defined"))
        })?;
        self.node_attr()
            .runtime_geometry()
            .and_then(|geometry| geometry.face(face))
            .map(|face| *face.frame())
            .ok_or_else(|| {
                OpossumError::Other(format!("the surface {surf_name} is no face of the node"))
            })
    }

    fn cross_section(&self) -> OpmResult<Option<Outline>> {
        let Ok(Proptype::OpticShape(optic_shape)) = self.node_attr().get_property(OPTIC_SHAPE)
        else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no '{OPTIC_SHAPE}' property, so its extent is unknown",
                self.name()
            )));
        };
        Ok(optic_shape.outline().cloned())
    }

    fn material(&self) -> OpmResult<AssetRef<Material>> {
        let Ok(Proptype::Material(material)) = self.node_attr().get_property(MATERIAL) else {
            return Err(OpossumError::Analysis(format!(
                "node '{}' has no '{MATERIAL}' property",
                self.name()
            )));
        };
        Ok(material.clone())
    }

    fn geometry_mesh(&self, segments: usize) -> OpmResult<Vec<FacePatch>> {
        let Some(geometry) = self.create_geometry()? else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no geometry to draw",
                self.name()
            )));
        };
        let default_shape = default_optic_shape();
        let unbounded = default_shape.outline().ok_or_else(|| {
            OpossumError::Other("the default optic shape bounds no region".into())
        })?;
        geometry.kind().mesh(segments, unbounded)
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

    fn face_name(&self, face: FaceIndex) -> OpmResult<String> {
        self.create_geometry()?
            .and_then(|geometry| geometry.kind().face_name(face))
            .ok_or_else(|| {
                OpossumError::Other(format!("node '{}' has no face {face}", self.name()))
            })
    }

    fn surface_config(&self, face_name: &str) -> OpmResult<SurfaceConfig> {
        match self.node_attr().get_property(face_name) {
            Ok(Proptype::SurfaceConfig(config)) => Ok(config.clone()),
            _ => Err(OpossumError::Other(format!(
                "node '{}' has no surface property '{face_name}'",
                self.name()
            ))),
        }
    }

    fn create_default_surface_properties(&mut self) -> OpmResult<()> {
        let Some(geometry) = self.create_geometry()? else {
            return Ok(());
        };
        let splitting_faces = self.surface_ports().splitting;
        let kind = geometry.kind();
        for face in (0..kind.face_count()).map(FaceIndex::new) {
            let Some(name) = kind.face_name(face) else {
                continue;
            };
            let splitting = splitting_faces.contains(&face);
            let coating = if splitting {
                SplittingConfigBuilder::default().into()
            } else {
                CoatingType::default()
            };
            self.node_attr_mut().create_property_with_validator(
                &name,
                &format!("coating and laser induced damage threshold of the {name}"),
                Validator::SurfaceCoating { splitting },
                SurfaceConfig::from(coating).into(),
            )?;
        }
        Ok(())
    }

    fn set_coating(&mut self, face: FaceIndex, coating: &CoatingType) -> OpmResult<()> {
        let name = self.face_name(face)?;
        let config = SurfaceConfig::new(coating.clone(), self.surface_config(&name)?.lidt())?;
        self.node_attr_mut().set_property(&name, config.into())?;
        self.update_surfaces()
    }

    fn set_lidt(&mut self, face: FaceIndex, lidt: Fluence) -> OpmResult<()> {
        let name = self.face_name(face)?;
        let config = SurfaceConfig::new(self.surface_config(&name)?.coating().clone(), lidt)?;
        self.node_attr_mut().set_property(&name, config.into())?;
        self.update_surfaces()
    }

    fn placed_geometry(&self) -> OpmResult<Option<Geometry>> {
        let mut geometry = self.create_geometry()?;
        if let Some(geometry) = &mut geometry {
            geometry.move_by(&self.effective_node_iso().unwrap_or_else(Isometry::identity));
        }
        Ok(geometry)
    }

    fn install_geometry(&mut self, surface_ports: &SurfacePorts) -> OpmResult<()> {
        let Some(mut placed) = self.placed_geometry()? else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no geometry to install surfaces from",
                self.name()
            )));
        };
        let mut ports = OpticPorts::new();
        for (port_names, port_type) in [
            (surface_ports.inputs, PortType::Input),
            (surface_ports.outputs, PortType::Output),
        ] {
            for &(name, face) in port_names {
                if placed.kind().surface(face).is_none() {
                    return Err(OpossumError::Other(format!(
                        "surface '{name}' sits on face {face}, which does not exist (face count: {})",
                        placed.kind().face_count()
                    )));
                }
                ports.add(&port_type, name)?;
            }
        }
        for face in (0..placed.kind().face_count()).map(FaceIndex::new) {
            let Some(face_name) = placed.kind().face_name(face) else {
                continue;
            };
            let config = self.surface_config(&face_name)?;
            if let Some(surface) = placed.kind_mut().surface_mut(face) {
                surface.set_config(&config);
            }
        }
        self.node_attr_mut().set_ports(ports);
        self.node_attr_mut().set_runtime_geometry(placed);
        Ok(())
    }

    fn face_of_port(&self, port: &str) -> OpmResult<FaceIndex> {
        self.surface_ports().face_of(port).ok_or_else(|| {
            OpossumError::Analysis(format!(
                "Cannot find surface: \"{port}\" of node: \"{}\"",
                self.name()
            ))
        })
    }

    fn get_optic_surface(&self, port: &str) -> Option<&OpticSurface> {
        let face = self.surface_ports().face_of(port)?;
        self.node_attr().runtime_geometry()?.kind().surface(face)
    }

    fn get_optic_surface_mut(&mut self, port: &str) -> Option<&mut OpticSurface> {
        let face = self.surface_ports().face_of(port)?;
        self.node_attr_mut()
            .runtime_geometry_mut()?
            .kind_mut()
            .surface_mut(face)
    }

    fn pass_through_surface_generic(
        &mut self,
        optic_surf_name: &str,
        effect: &dyn SurfaceEffect,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
        backward: bool,
        part: ReflectedPart,
    ) -> OpmResult<()> {
        let uuid = self.node_attr().uuid();
        let node_name = self.name().to_string();
        let face = self.face_of_port(optic_surf_name)?;
        let Some(geometry) = self.node_attr_mut().runtime_geometry_mut() else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{node_name}\""
            )));
        };
        let rays_before: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        geometry.propagate_rays(face, rays_bundle, effect, part, backward, uuid, strategy)?;
        let rays_after: usize = rays_bundle.iter().map(|r| r.nr_of_rays(true)).sum();
        self.warn_about_lost_rays(rays_before, rays_after);
        Ok(())
    }

    fn unified_analyze_single_surface_node(
        &mut self,
        mut incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
        optic_surf_name: &str,
    ) -> OpmResult<LightResult> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
        let Some(data) = incoming_data.remove(&in_port_name) else {
            return Ok(LightResult::default());
        };
        let refraction = Refraction(None);

        match data {
            LightData::Geometric(rays) => {
                let mut rays_bundle = vec![rays];
                self.pass_through_surface_generic(
                    optic_surf_name,
                    &refraction,
                    &mut rays_bundle,
                    strategy,
                    false,
                    ReflectedPart::Ghost,
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
                    &refraction,
                    &mut rays_bundle,
                    strategy,
                    false,
                    ReflectedPart::Ghost,
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

    fn pass_through_volume_generic(
        &mut self,
        entry_surf_name: &str,
        exit_surf_name: &str,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let node_name = self.name().to_owned();
        // Behind the exit surface the node's ambient index applies, inside it the material's.
        let index_inside = self
            .node_attr()
            .runtime_geometry()
            .and_then(|geometry| geometry.kind().as_body()?.material())
            .map(|material| material.refractive_index_type().clone())
            .ok_or_else(|| {
                OpossumError::Analysis(format!(
                    "node '{node_name}': its medium holds no material to refract with"
                ))
            })?;
        let backward = self.inverted();
        self.pass_through_surface_generic(
            entry_surf_name,
            &Refraction(Some(&index_inside)),
            rays_bundle,
            strategy,
            backward,
            ReflectedPart::Ghost,
        )?;
        if !strategy.is_positioning_run() {
            // `gain_model` is bound so the model it hands out outlives the extraction borrowed from it.
            let gain_model = strategy.pump_config(self.node_attr().uuid()).gain_model();
            let body = self
                .node_attr_mut()
                .runtime_geometry_mut()
                .and_then(|geometry| geometry.kind_mut().as_body_mut())
                .ok_or_else(|| {
                    OpossumError::Analysis(format!(
                        "node '{node_name}': medium was not prepared before propagation"
                    ))
                })?;
            body.propagate_inside(gain_model.as_extraction(), rays_bundle)
                .map_err(|e| OpossumError::Analysis(format!("node '{node_name}': {e}")))?;
        }
        self.pass_through_surface_generic(
            exit_surf_name,
            &Refraction(Some(&strategy.ambient_refractive_index())),
            rays_bundle,
            strategy,
            backward,
            ReflectedPart::Ghost,
        )
    }

    fn unified_analyze_volume_node(
        &mut self,
        mut incoming_data: LightResult,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightResult> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
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
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightRays> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
        let mut rays_bundle = incoming_data
            .get(&in_port_name)
            .map_or_else(Vec::<Rays>::new, Clone::clone);
        self.pass_through_volume_generic(
            &in_port_name,
            &out_port_name,
            &mut rays_bundle,
            strategy,
        )?;
        Ok(LightRays::from([(out_port_name, rays_bundle)]))
    }

    fn encloses_volume(&self) -> OpmResult<bool> {
        Ok(self
            .create_geometry()?
            .is_some_and(|geometry| geometry.kind().as_body().is_some()))
    }

    fn record_within_window(
        &mut self,
        optic_surf_name: &str,
        bundles: &mut [Rays],
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        if self.get_optic_surface(optic_surf_name).is_none() {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{}\"",
                self.name()
            )));
        }
        let mut cut = false;
        if let Some(window) = self
            .node_attr()
            .runtime_geometry()
            .and_then(|geometry| geometry.kind().window())
        {
            for rays in bundles.iter_mut() {
                cut |= rays.invalidate_outside(window);
            }
        }
        if cut {
            let (name, node_type) = (self.name(), self.node_type());
            if strategy.is_positioning_run() {
                log::warn!(
                    "the optical axis misses the optic shape of detector '{name}' ({node_type}); its measurement may fail"
                );
            } else {
                log::warn!(
                    "rays passed '{name}' ({node_type}) outside its optic shape and were not recorded. Results might not be accurate."
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
                "Rays were lost at '{}' ({}): they ran outside its optic shape or fell below the energy threshold. Results might not be accurate.",
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
        apertures::{Bore, CircleShape, OpticShape},
        coatings::SplittingConfigBuilder,
        core_optics::{
            node_attr::{HasNodeAttr, NodePositioning},
            optic_node::LIDT,
        },
        degree,
        geometry::{Extruded, SurfaceGeometry, SurfaceMesh},
        joule,
        light::Ray,
        millimeter, nanometer,
        nodes::{BeamSplitter, Dummy, Lens, NodeGroup, ParabolicMirror, ThinMirror},
        properties::Properties,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::Point2;
    use std::f64::consts::PI;

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
        )?;
        assert!(out.is_empty());
        Ok(())
    }

    /// Setting the coating or LIDT of a surface of an inverted node keeps its ports stored
    /// physically: otherwise it stays inverted after the inversion is reverted, and every lookup of
    /// a physical port misses.
    #[test]
    fn surface_settings_keep_the_stored_ports_of_an_inverted_node_physical() -> OpmResult<()> {
        let inverted = || -> OpmResult<Dummy> {
            let mut node = Dummy::default();
            node.set_inverted(true)?;
            Ok(node)
        };
        let lidt = J_per_cm2!(2.0);
        let mut with_coating = inverted()?;
        with_coating.set_coating(SurfaceGeometry::FACE, &CoatingType::Fresnel)?;
        let mut with_lidt = inverted()?;
        with_lidt.set_lidt(SurfaceGeometry::FACE, lidt)?;
        for mut node in [with_coating, with_lidt, inverted()?.with_lidt(lidt)?] {
            assert_eq!(
                node.node_attr().raw_ports().names(&PortType::Input),
                vec!["input_1"]
            );
            node.set_inverted(false)?;
            assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        }
        Ok(())
    }
    /// Changing the coating or LIDT of a surface installs the surfaces anew, and every port on
    /// that face carries them.
    #[test]
    fn a_changed_surface_reaches_every_port_on_it() -> OpmResult<()> {
        let mut node = Dummy::default();
        node.set_coating(SurfaceGeometry::FACE, &CoatingType::Fresnel)?;
        node.set_lidt(SurfaceGeometry::FACE, J_per_cm2!(2.0))?;
        for port in ["input_1", "output_1"] {
            let surface = node
                .get_optic_surface(port)
                .ok_or_else(|| OpossumError::Other(format!("the dummy lost its {port} surface")))?;
            assert!(matches!(surface.coating(), CoatingType::Fresnel));
            assert_eq!(*surface.lidt(), J_per_cm2!(2.0));
        }
        Ok(())
    }
    /// The faces of a lens carry their own coatings: the front one reaches the surface under the
    /// input port, the rear one the surface under the output port.
    #[test]
    fn each_face_of_a_lens_carries_its_own_coating() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_coating(Extruded::FRONT, &CoatingType::Fresnel)?;
        let coating = |port: &str| {
            lens.get_optic_surface(port)
                .map(|surface| surface.coating().clone())
                .ok_or_else(|| OpossumError::Other(format!("the lens has no {port} surface")))
        };
        assert!(matches!(coating("input_1")?, CoatingType::Fresnel));
        assert!(matches!(coating("output_1")?, CoatingType::IdealAR));
        Ok(())
    }
    /// A mirror or a lens sends only one part of the light on at each surface, so none of their
    /// surfaces takes a separator.
    #[test]
    fn only_a_beam_splitter_takes_a_separator() {
        let separator: CoatingType = SplittingConfigBuilder::default().into();
        assert!(
            ThinMirror::default()
                .set_coating(SurfaceGeometry::FACE, &separator)
                .is_err()
        );
        for face in [Extruded::FRONT, Extruded::REAR] {
            assert!(Lens::default().set_coating(face, &separator).is_err());
        }
    }
    /// Pass a ray along the axis through the given port of a node, as a ray trace does.
    fn pass_along_the_axis<T: OpticNodeExt>(node: &mut T, port: &str) -> OpmResult<Rays> {
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, -10.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        let mut bundles = vec![rays.clone()];
        node.pass_through_surface_generic(
            port,
            &Refraction(None),
            &mut bundles,
            &RayTraceConfig::default(),
            false,
            ReflectedPart::Ghost,
        )?;
        Ok(rays)
    }
    /// A face is one surface, however many ports sit on it: light passing a single surface forward
    /// (through `input_1`) and backward (through `output_1`) is recorded in its one hit map, under
    /// the name of the face.
    #[test]
    fn both_ports_of_a_single_surface_record_on_its_one_face() -> OpmResult<()> {
        let mut node = Dummy::default();
        let forward = pass_along_the_axis(&mut node, "input_1")?;
        let backward = pass_along_the_axis(&mut node, "output_1")?;
        let surfaces = node.surfaces_with_hits();
        let [(name, surface)] = surfaces.as_slice() else {
            return Err(OpossumError::Other(format!(
                "expected one surface with hits, got {}",
                surfaces.len()
            )));
        };
        assert_eq!(name, "surface");
        for rays in [forward, backward] {
            assert!(surface.hit_map().get_rays_hit_map(0, rays.uuid()).is_some());
        }
        Ok(())
    }
    /// A port the node does not have has no surface to trace at.
    #[test]
    fn an_unknown_port_is_an_error() {
        let error = pass_along_the_axis(&mut Dummy::default(), "output_2")
            .expect_err("a port the dummy does not have was traced");
        assert!(
            error
                .to_string()
                .contains("Cannot find surface: \"output_2\""),
            "{error}"
        );
    }
    /// An LIDT given to a whole node reaches every one of its surfaces.
    #[test]
    fn a_node_lidt_reaches_every_surface() -> OpmResult<()> {
        let lens = Lens::default().with_lidt(J_per_cm2!(3.0))?;
        for port in ["input_1", "output_1"] {
            let surface = lens
                .get_optic_surface(port)
                .ok_or_else(|| OpossumError::Other(format!("the lens has no {port} surface")))?;
            assert_eq!(*surface.lidt(), J_per_cm2!(3.0));
        }
        Ok(())
    }
    /// A node without the surface property of a face cannot install its surfaces.
    #[test]
    fn a_missing_surface_property_is_an_error() -> OpmResult<()> {
        let mut node = Dummy::default();
        let mut shape_only = Properties::default();
        shape_only.create(OPTIC_SHAPE, "extent", OpticShape::Open.into())?;
        node.node_attr_mut().set_properties(shape_only);
        let error = node
            .update_surfaces()
            .expect_err("surfaces were installed without a surface property");
        assert!(
            error
                .to_string()
                .contains("has no surface property 'surface'"),
            "{error}"
        );
        Ok(())
    }
    /// A port on a face the geometry does not have is rejected, and the installation before stays.
    #[test]
    fn a_port_on_a_missing_face_is_rejected() {
        const BEHIND_THE_SURFACE: SurfacePorts = SurfacePorts {
            inputs: &[("input_1", SurfaceGeometry::FACE)],
            outputs: &[("output_2", FaceIndex::new(1))],
            splitting: &[],
        };
        let mut node = Dummy::default();
        let error = node
            .install_geometry(&BEHIND_THE_SURFACE)
            .expect_err("a port on face 1 of a single surface was accepted");
        assert!(
            error
                .to_string()
                .contains("surface 'output_2' sits on face 1, which does not exist"),
            "{error}"
        );
        assert!(node.get_optic_surface("output_2").is_none());
        assert!(node.get_optic_surface("output_1").is_some());
    }
    /// A node is meshed from its geometry: a group has none of its own and so nothing to mesh, and
    /// the unbounded plane of a dummy is drawn as far as the default optic shape reaches.
    #[test]
    fn a_node_is_meshed_from_its_geometry() -> OpmResult<()> {
        let error = NodeGroup::default().geometry_mesh(16).unwrap_err();
        assert!(error.to_string().contains("no geometry"), "{error}");
        let meshes = Dummy::default().geometry_mesh(16)?;
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].face, Some(SurfaceGeometry::FACE));
        let farthest = meshes[0]
            .mesh
            .points()
            .iter()
            .map(|point| point.x.value.hypot(point.y.value))
            .fold(0.0, f64::max);
        let reach = default_optic_shape()
            .outline()
            .map(|outline| outline.transversal_reach().value)
            .ok_or_else(|| OpossumError::Other("the default optic shape is unbounded".into()))?;
        assert_abs_diff_eq!(farthest, reach, epsilon = 1e-12);
        Ok(())
    }
    /// The one patch a surface node is drawn as.
    fn surface_mesh(node: &impl OpticNode, segments: usize) -> OpmResult<SurfaceMesh> {
        let mut meshes = node.geometry_mesh(segments)?;
        assert_eq!(meshes.len(), 1, "a surface is drawn as one patch");
        meshes
            .pop()
            .map(|patch| patch.mesh)
            .ok_or_else(|| OpossumError::Other("the surface was not meshed".into()))
    }
    /// A node in its default state — a flat mirror over the default optic shape — can be meshed at
    /// all, and the mesh actually covers ground rather than coming back empty.
    #[test]
    fn a_surface_node_can_be_meshed_in_its_default_state() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = surface_mesh(&mirror, 32)?;
        assert!(
            !mesh.triangles().is_empty(),
            "a mirror's default surface came out without any triangles"
        );
        assert_eq!(
            mesh.points().len(),
            mesh.normals().len(),
            "every meshed point needs a normal to shade or hit-test it"
        );
        Ok(())
    }
    /// The mesh is drawn from the node's properties as they are now, exactly like a volume node's
    /// body: a curvature set without rebuilding the surfaces already bends it. A sphere of radius R
    /// rises by R - sqrt(R² - r²) at the 12.5 mm rim of the default optic shape.
    #[test]
    fn the_mesh_follows_a_changed_property_without_a_rebuild() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let radius = 100.0_f64;
        mirror.set_property("curvature", Proptype::Curvature(millimeter!(radius)))?;
        let mesh = surface_mesh(&mirror, 32)?;
        let rise = mesh
            .points()
            .iter()
            .map(|point| point.z.value.abs())
            .fold(0.0, f64::max);
        assert_abs_diff_eq!(
            rise,
            millimeter!(radius - radius.mul_add(radius, -12.5 * 12.5).sqrt()).value,
            epsilon = 1e-9
        );
        Ok(())
    }
    /// The mesh lives in the node's own frame, exactly like a volume node's - moving the node must
    /// not bake a translation into the points themselves.
    #[test]
    fn the_mesh_does_not_depend_on_where_the_node_is_placed() -> OpmResult<()> {
        let at_origin = {
            let mut mirror = ThinMirror::default();
            mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
            surface_mesh(&mirror, 16)?
        };
        let far_away = {
            let mut mirror = ThinMirror::default();
            mirror.set_positioning(NodePositioning::Absolute(Isometry::new(
                millimeter!(0.0, 0.0, 500.0),
                degree!(0.0, 0.0, 0.0),
            )?))?;
            surface_mesh(&mirror, 16)?
        };
        assert_eq!(
            at_origin.points(),
            far_away.points(),
            "the surface mesh must be independent of the node's placement"
        );
        Ok(())
    }
    /// A mirror with a bore through it is meshed around the bore: what is drawn covers the ring it
    /// is, not the disc it would be without the bore.
    #[test]
    fn a_surface_with_a_bore_is_meshed_around_it() -> OpmResult<()> {
        let (radius, bore) = (millimeter!(12.5), millimeter!(4.0));
        let ring = OpticShape::Bounded(Outline::new(
            CircleShape::new(radius)?,
            Some(Bore::new(
                CircleShape::new(bore)?,
                millimeter!(0., 0.),
                degree!(0.),
            )),
        )?);
        let mut mirror = ThinMirror::default();
        mirror.set_property(OPTIC_SHAPE, ring.into())?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = surface_mesh(&mirror, 64)?;
        let covered = mesh
            .triangles()
            .iter()
            .map(|triangle| {
                let [a, b, c] =
                    triangle.map(|index| mesh.points()[index as usize].map(|c| c.value));
                (b - a).cross(&(c - a)).norm() / 2.
            })
            .sum::<f64>();
        let ring_area = PI * (radius.value.powi(2) - bore.value.powi(2));
        assert_relative_eq!(covered, ring_area, max_relative = 1e-2);
        Ok(())
    }
    /// The mesh of an off-axis parabola lies on the mirror, every normal standing on it, although
    /// the optic shape is measured across the axis of the parent parabola rather than the mirror's
    /// own.
    #[test]
    fn the_mesh_of_an_off_axis_parabola_lies_on_the_mirror() -> OpmResult<()> {
        let mut parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        let optic_shape =
            OpticShape::Bounded(Outline::new(CircleShape::new(millimeter!(25.0))?, None)?);
        parabola.set_property(OPTIC_SHAPE, optic_shape.into())?;
        let mesh = surface_mesh(&parabola, 32)?;
        let geometry = parabola
            .create_geometry()?
            .ok_or_else(|| OpossumError::Other("a mirror has a geometry".into()))?;
        let face = geometry
            .face(SurfaceGeometry::FACE)
            .ok_or_else(|| OpossumError::Other("a mirror has a face".into()))?;
        for (point, normal) in mesh.points().iter().zip(mesh.normals()) {
            assert!(
                face.is_on_vertex_sheet(point),
                "{point:?} does not lie on the mirror"
            );
            let on_surface = face.frame().inverse_transform_point(point);
            let surface_normal = face
                .surface()
                .local_normal_at(&Point2::new(on_surface.x, on_surface.y))
                .expect("the mirror reaches a point it was meshed at");
            let surface_normal = face.frame().transform_vector_f64(&surface_normal);
            assert_abs_diff_eq!(normal.dot(&surface_normal).abs(), 1.0, epsilon = 1e-9);
        }
        Ok(())
    }
}
