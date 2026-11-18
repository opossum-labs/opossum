#![warn(missing_docs)]
//! What the nodes have in common that split the light of two inputs into two outputs, such as the
//! [`BeamSplitter`](crate::nodes::BeamSplitter) and the
//! [`BoredMirror`](crate::nodes::BoredMirror).

use log::warn;
use uom::si::energy::joule;

use crate::{
    analyzers::{
        AnalyzerType, GhostFocusConfig, RayTraceConfig,
        propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    },
    coatings::SplittingConfig,
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType, surface_effect::Refraction},
    error::{OpmResult, OpossumError},
    joule,
    light::{LightData, LightRays, LightResult, Rays, rays::ReflectedPart},
};

/// A node that splits the light of two inputs into two outputs.
///
/// Its first and second input and output are the ports it declares in that order (see
/// [`OpticNode::surface_ports`]), such as `input_1`, `input_2`, `out1_trans1_refl2` and
/// `out2_trans2_refl1` of a [`BeamSplitter`](crate::nodes::BeamSplitter). Light arriving at an
/// input is split at the face of that input. The part the face transmits keeps its way, the part
/// it reflects leaves through the other output: the first output collects what the first input
/// transmits and the second reflects, the second output the rest. An inverted node swaps inputs
/// and outputs. How the light is split is up to the geometry and the coatings; this trait only
/// routes it. Light entering the body of a node is not traced through it yet: only light passing
/// a bore gets through a body.
pub trait Splitter: OpticNode {
    /// Split the rays arriving at both inputs on the surface and merge them per output, as a ray
    /// trace does (see [`AnalysisRayTrace::analyze`](crate::analyzers::raytrace::AnalysisRayTrace::analyze)).
    ///
    /// An alignment run reaches this through a reference (to the node or to a group holding it).
    /// It then places the following nodes by geometry alone, exactly like
    /// [`Splitter::split_axis`]: how the light is split must not decide where the optical axis
    /// runs.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the light arriving at the input ports.
    /// * `config` - the ray tracing configuration.
    ///
    /// # Returns
    ///
    /// The light leaving both outputs, or nothing if no light arrives.
    ///
    /// # Errors
    ///
    /// This function returns an error if the light is not geometric, if the surface cannot be
    /// found or if the split fails.
    fn split_rays(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        if config.is_positioning_run() {
            if incoming_data.is_empty() {
                return Ok(LightResult::default());
            }
            return self.split_axis(incoming_data, config);
        }
        let (input_1, input_2) = self.port_pair(PortType::Input)?;
        let (out1_data, out2_data) = self.split_both_inputs(
            incoming_data.get(&input_1),
            incoming_data.get(&input_2),
            &AnalyzerType::RayTrace(config.clone()),
        )?;
        if let Some(out1_data) = out1_data
            && let Some(out2_data) = out2_data
        {
            let (out_1, out_2) = self.port_pair(PortType::Output)?;
            Ok(LightResult::from([(out_1, out1_data), (out_2, out2_data)]))
        } else {
            Ok(LightResult::default())
        }
    }
    /// Calculate the outgoing axis rays of this node for positioning the following nodes (see
    /// [`AnalysisRayTrace::calc_node_positions`](crate::analyzers::raytrace::AnalysisRayTrace::calc_node_positions)).
    ///
    /// The axis rays arriving at each input are split on the surface. For the first input the
    /// transmitted rays go on behind the first output and the reflected rays behind the second; for
    /// the second input the roles are swapped, since its transmission exits the second output and
    /// its reflection the first. On both outputs the rays of the first input come first, if it has
    /// any: the first ray of a bundle is the axis the nodes behind it are placed along, and the node
    /// itself is placed by that input as well. The rays of the second input follow them, so that
    /// its arm is drawn on as far as it goes. All ports are swapped, if the node is inverted.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the axis rays arriving at the input ports.
    /// * `config` - the ray tracing configuration.
    ///
    /// # Returns
    ///
    /// The axis rays leaving both output ports.
    ///
    /// # Errors
    ///
    /// This function returns an error if there are no geometric axis rays at either input or the
    /// surface cannot be found.
    fn split_axis(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_1, input_2) = self.port_pair(PortType::Input)?;
        let (out_1, out_2) = self.port_pair(PortType::Output)?;
        // Transmission keeps the beam direction, reflection exits on the other output. So the first
        // input feeds (out_1, out_2) with its (transmitted, reflected) rays; the second input, whose
        // transmission continues into out_2, feeds them in swapped order.
        let first = incoming_data
            .get(&input_1)
            .map(|input| self.split_axis_rays(input, &input_1, &out_1, config))
            .transpose()?;
        let second = incoming_data
            .get(&input_2)
            .map(|input| self.split_axis_rays(input, &input_2, &out_2, config))
            .transpose()?
            .map(|(transmitted, reflected)| (reflected, transmitted));
        let mut parts = first.into_iter().chain(second);
        let Some((mut on_out_1, mut on_out_2)) = parts.next() else {
            return Err(OpossumError::Analysis(format!(
                "could not calc optical axis for {}",
                self.node_type()
            )));
        };
        for (to_out_1, to_out_2) in parts {
            on_out_1.merge(&to_out_1);
            on_out_2.merge(&to_out_2);
        }
        Ok(LightResult::from([
            (out_1, LightData::Geometric(on_out_1)),
            (out_2, LightData::Geometric(on_out_2)),
        ]))
    }
    /// Split the ray bundles arriving at both inputs one by one, as a ghost focus analysis does
    /// (see [`AnalysisGhostFocus::analyze`](crate::analyzers::ghostfocus::AnalysisGhostFocus::analyze)),
    /// and weigh the fluence of each arriving bundle against the LIDT of the surface.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the ray bundles arriving at the input ports.
    /// * `config` - the ghost focus configuration.
    ///
    /// # Returns
    ///
    /// The ray bundles leaving both outputs.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have two inputs and two outputs, if the
    /// surface of an input cannot be found, if a split fails or if the fluence cannot be weighed.
    fn split_ghost_bundles(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
    ) -> OpmResult<LightRays> {
        let (in1_port, in2_port) = self.port_pair(PortType::Input)?;
        let (out1_port, out2_port) = self.port_pair(PortType::Output)?;

        let mut rays1_bundle = incoming_data
            .get(&in1_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        let mut rays2_bundle = incoming_data
            .get(&in2_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        let mut light_data_out1: Vec<Rays> = vec![];
        let mut light_data_out2: Vec<Rays> = vec![];
        for rays in &mut rays1_bundle {
            let (out1_data, out2_data) = self.split_both_inputs(
                Some(&LightData::Geometric(rays.clone())),
                None,
                &AnalyzerType::GhostFocus(config.clone()),
            )?;
            if let Some(LightData::Geometric(rays)) = out1_data {
                light_data_out1.push(rays.clone());
            }
            if let Some(LightData::Geometric(rays)) = out2_data {
                light_data_out2.push(rays.clone());
            }
        }
        for rays in &mut rays2_bundle {
            let (out1_data, out2_data) = self.split_both_inputs(
                None,
                Some(&LightData::Geometric(rays.clone())),
                &AnalyzerType::GhostFocus(config.clone()),
            )?;
            if let Some(LightData::Geometric(rays)) = out1_data {
                light_data_out1.push(rays.clone());
            }
            if let Some(LightData::Geometric(rays)) = out2_data {
                light_data_out2.push(rays.clone());
            }
        }
        let Some(surf) = self.get_optic_surface_mut(&in1_port) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{in1_port}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut rays1_bundle {
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }
        let Some(surf) = self.get_optic_surface_mut(&in2_port) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{in2_port}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut rays2_bundle {
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }
        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out1_port, light_data_out1);
        out_light_rays.insert(out2_port, light_data_out2);
        Ok(out_light_rays)
    }
    /// Split the light arriving at both inputs on the surface and merge it per output: the first
    /// output gets what the first input transmits and the second reflects, the second output the
    /// rest. A ray trace drops rays below its energy threshold afterwards.
    ///
    /// # Arguments
    ///
    /// * `in1` - the light arriving at the first input, if any.
    /// * `in2` - the light arriving at the second input, if any.
    /// * `analyzer_type` - the analysis, which decides what happens to rays missing the surface.
    ///
    /// # Returns
    ///
    /// The light leaving the first and the second output, or nothing if no light arrives.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have two inputs, if the light is not
    /// geometric, if the surface cannot be found or if the split fails.
    fn split_both_inputs(
        &mut self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        }
        let (in1_port_name, in2_port_name) = self.port_pair(PortType::Input)?;
        let (out1_port_name, out2_port_name) = self.port_pair(PortType::Output)?;

        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy(_) => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => ray_trace_config.missed_surface_strategy(),
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };

        // What the first input transmits leaves the first output, what the second transmits the
        // second.
        let (mut main_rays1, split_rays1) = self.split_at_input(
            in1,
            &in1_port_name,
            &out1_port_name,
            *missed_surface_strategy,
        )?;
        let (mut main_rays2, split_rays2) = self.split_at_input(
            in2,
            &in2_port_name,
            &out2_port_name,
            *missed_surface_strategy,
        )?;

        main_rays1.merge(&split_rays2); // out1 = trans1 + refl2
        main_rays2.merge(&split_rays1); // out2 = trans2 + refl1
        if let AnalyzerType::RayTrace(config) = analyzer_type {
            main_rays1.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            main_rays2.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        }

        Ok((
            Some(LightData::Geometric(main_rays1)),
            Some(LightData::Geometric(main_rays2)),
        ))
    }
    /// Split the rays arriving at a single input port at the face of the port, by its coating: the
    /// transmitted part keeps its direction, the reflected part is mirrored about the surface
    /// normal. On a body, the part transmitted into its material is not traced through it yet: it
    /// stops at the face. Light in the bore has to leave through the bore as well, at the face of
    /// the transmitted output; otherwise it hits the wall of the bore and stops. Either loss is
    /// warned about if it carries energy.
    ///
    /// # Arguments
    ///
    /// * `input` - the light arriving at the port, if any.
    /// * `port_name` - the name of the input port.
    /// * `exit_port` - the output port the transmitted part leaves through.
    /// * `missed_surface_strategy` - what happens to rays that do not hit the surface.
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected rays, both empty if there is no input.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    ///   - the input is not [`LightData::Geometric`].
    ///   - the surface of the port cannot be found.
    ///   - the splitting of the rays fails.
    fn split_at_input(
        &mut self,
        input: Option<&LightData>,
        port_name: &str,
        exit_port: &str,
        missed_surface_strategy: MissedSurfaceStrategy,
    ) -> OpmResult<(Rays, Rays)> {
        let Some(light_data) = input else {
            return Ok((Rays::default(), Rays::default()));
        };
        let LightData::Geometric(rays) = light_data else {
            return Err(OpossumError::Analysis(format!(
                "Expected LightData::Geometric at port '{port_name}'"
            )));
        };
        let mut rays = rays.clone();
        let face = self.face_of_port(port_name)?;
        let exit_face = self.face_of_port(exit_port)?;
        let name = self.node_attr().name().to_string();
        let Some(geometry) = self.node_attr_mut().runtime_geometry_mut() else {
            return Err(OpossumError::OpticPort(format!(
                "Input optic surface not found for port '{port_name}'"
            )));
        };
        // Whether each ray meets the face on its material, in its bore, or not at all.
        let on_material = rays
            .iter()
            .map(|ray| {
                ray.valid()
                    .then(|| geometry.intersect(face, ray).map(|hit| hit.on_material()))
                    .flatten()
            })
            .collect::<Vec<_>>();
        let reflected = geometry.interact_with_rays(
            face,
            &mut rays,
            &Refraction(None),
            ReflectedPart::Branch,
            missed_surface_strategy,
        )?;
        let into_body = geometry.kind().as_body().is_some();
        let (mut in_body, mut at_wall) = (joule!(0.0), joule!(0.0));
        for (ray, on_material) in rays.iter_mut().zip(on_material) {
            let lost = match on_material {
                Some(true) if into_body => &mut in_body,
                Some(false)
                    if geometry
                        .intersect(exit_face, ray)
                        .is_none_or(|hit| hit.on_material()) =>
                {
                    &mut at_wall
                }
                Some(_) | None => continue,
            };
            *lost += ray.energy();
            ray.set_invalid();
        }
        for (energy, where_to) in [
            (in_body, "enters its body, which is not traced yet"),
            (
                at_wall,
                "passes into its bore but hits the wall of the bore",
            ),
        ] {
            if energy > joule!(0.0) {
                warn!(
                    "{:.3e} J of the light reaching '{name}' at port '{port_name}' {where_to}: \
                     that light is lost",
                    energy.get::<joule>()
                );
            }
        }
        Ok((rays, reflected))
    }
    /// Splits the axis rays arriving at `port_name` on the surface for a positioning run.
    ///
    /// Positioning is purely geometric: a fixed 50:50 split leaves energy in both branches, so
    /// neither is invalidated by an energy threshold downstream, and no alignment wavelength has to
    /// lie inside a splitting spectrum. The reflected axis leaves where the axis meets the surface,
    /// as if there were no bore. If the surface has a bore, the transmitted axis starts anew at
    /// the center of the bore on the surface instead, where the axis of the bore meets it, and runs
    /// on parallel to the arriving axis: the light the bore lets pass is centered there, and the
    /// way to the node is drawn along the reflected axis. Through a body it runs along that line to
    /// where it leaves through the face of the transmitted output. Without a bore the transmitted
    /// axis runs on where the axis meets the surface.
    ///
    /// # Arguments
    ///
    /// * `input` - the axis rays arriving at the port.
    /// * `port_name` - the input port (and its surface).
    /// * `exit_port` - the output port the transmitted part leaves through.
    /// * `config` - the ray tracing configuration (for the missed-surface strategy).
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected axis rays, in that order.
    ///
    /// # Errors
    ///
    /// This function returns an error if `input` is not [`LightData::Geometric`], the surface
    /// cannot be found, the split fails, or the line through the center of the bore misses the
    /// face it enters or leaves through.
    fn split_axis_rays(
        &mut self,
        input: &LightData,
        port_name: &str,
        exit_port: &str,
        config: &RayTraceConfig,
    ) -> OpmResult<(Rays, Rays)> {
        let LightData::Geometric(rays) = input else {
            return Err(OpossumError::Analysis(format!(
                "expected Rays value at `{port_name}` port"
            )));
        };
        let mut transmitted = rays.clone();
        let face = self.face_of_port(port_name)?;
        let exit_face = self.face_of_port(exit_port)?;
        let Some(geometry) = self.node_attr_mut().runtime_geometry_mut() else {
            return Err(OpossumError::OpticPort(
                "input optic surface not found".into(),
            ));
        };
        let reflected = geometry.interact_with_rays(
            face,
            &mut transmitted,
            &SplittingConfig::Ratio(0.5),
            ReflectedPart::Branch,
            *config.missed_surface_strategy(),
        )?;
        let missed = || {
            OpossumError::Analysis(
                "the optical axis through the center of the bore misses the node".into(),
            )
        };
        let center = match geometry.kind().bored_rim(face) {
            Some(rim) => rim.bore_center_on(geometry.face(face).ok_or_else(missed)?)?,
            None => None,
        };
        if let Some(start) = center {
            let exit = geometry.face(exit_face).ok_or_else(missed)?;
            for passed in transmitted.iter_mut().filter(|passed| passed.valid()) {
                passed.restart_at(start);
                if exit_face != face {
                    // The line along the arriving axis from the center, up to the exit face.
                    let (end, _) = exit.intersect(passed).ok_or_else(missed)?;
                    let through = end - start;
                    let along = passed.direction().normalize();
                    passed.propagate(
                        through.x * along.x + through.y * along.y + through.z * along.z,
                    )?;
                }
            }
        }
        Ok((transmitted, reflected))
    }
    /// Return the names of the two ports of the given type, first and second, in the order the
    /// node declares them (see [`OpticNode::surface_ports`]); an inverted node takes those of the
    /// other type.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly two such ports.
    fn port_pair(&self, port_type: PortType) -> OpmResult<(String, String)> {
        let physical = if self.inverted() {
            port_type.opposite()
        } else {
            port_type
        };
        let ports = self.surface_ports();
        let declared = match physical {
            PortType::Input => ports.inputs,
            PortType::Output => ports.outputs,
        };
        let [(first, _), (second, _)] = declared else {
            return Err(OpossumError::Analysis(format!(
                "{} needs two {port_type:?} ports, but has {:?}",
                self.node_type(),
                declared.iter().map(|(name, _)| name).collect::<Vec<_>>()
            )));
        };
        Ok(((*first).to_string(), (*second).to_string()))
    }
}
