#![warn(missing_docs)]
//! The capability of an optical node to enclose a volume of material.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is where light refracts, a
//! [`Body`](crate::geometry::body::Body) is what lies between two of them. Nodes such as a lens, a
//! wedge or a cylindric lens *are* the medium in between, and everything happening inside that
//! medium — the path light takes through it and, later on, the gain of an active medium — has to
//! address it.
//!
//! [`Volumetric`] is what states that. It is deliberately a capability of the node rather than a
//! list of node type names kept somewhere else: nodes are handled as `dyn Analyzable` trait objects
//! throughout the graph, so a caller that only holds such an object cannot recover the concrete type
//! it once was. [`OpticNode::as_volume`] is the one question that survives that erasure — every node
//! answers it, and only those with a volume answer it with `Some`.

use crate::{
    analyzers::propagation_strategy::PropagationStrategy,
    apertures::{Aperture, ApertureType},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, optic_node_ext::single_io_port_names},
    error::{OpmResult, OpossumError},
    gain::GainModel,
    geometry::body::{CLEAR_APERTURE, SurfaceBoundedBody},
    light::{LightData, LightRays, LightResult, Rays},
    material::{MATERIAL, Material},
    properties::{Proptype, proptype::AssetRef},
    types::validated_type_definitions::ValidatedCrossSection,
    utils::geom_transformation::Isometry,
};

/// An [`OpticNode`] that encloses a volume of material between two of its surfaces.
///
/// Implementing this trait is what makes a node a volume node. Everything the volume machinery needs
/// is derived from what such a node already has — the two
/// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface)s its `update_surfaces()` places and its
/// [`CLEAR_APERTURE`] property — so a node type declares the capability with an empty
/// `impl Volumetric for ... {}`, plus the [`OpticNode::as_volume`] override that makes it visible
/// through a trait object.
///
/// The methods below live here rather than on [`OpticNodeExt`], which every node type gets: they
/// are meaningful only where there is a volume, and now the compiler says so instead of a runtime
/// error on a node that never had one.
pub trait Volumetric: OpticNode {
    /// Return the [`Material`] this node's volume is filled with.
    ///
    /// The whole material is handed out, not just its refractive index model: a caller that only
    /// refracts takes the index out of it, while later stages (thermal lensing, stress
    /// birefringence, gain) need the other material data from the very same object.
    ///
    /// What a component is made of belongs to the component, not to the analysis looking at it,
    /// which is why this sits on the capability: every analysis entering the volume asks for the
    /// same material, and none of them has to know which property carries it.
    ///
    /// # Returns
    ///
    /// A clone of the node's material.
    ///
    /// # Errors
    ///
    /// This function errors if the node does not carry an embedded [`Material`] under the
    /// [`MATERIAL`] property.
    fn material(&self) -> OpmResult<Material> {
        let Ok(Proptype::Material(AssetRef::Inline(material))) =
            self.node_attr().get_property(MATERIAL)
        else {
            return Err(OpossumError::Analysis("cannot read material".into()));
        };
        Ok(material.clone())
    }
    /// Return the volume enclosed by the two surfaces of this node as a
    /// [`Body`](crate::geometry::body::Body).
    ///
    /// This is the geometric counterpart of [`Volumetric::pass_through_volume_generic`]: that
    /// function guides rays *through* the volume, this one describes the volume itself. It is
    /// derived entirely from what the node already has — the two
    /// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface)s built by `update_surfaces()` from
    /// the node's curvature and thickness properties, and its [`CLEAR_APERTURE`] property as the
    /// transversal extent.
    ///
    /// The returned body refers to the surfaces the node holds at this moment. Changing the node's
    /// placement or any of its geometry properties runs `update_surfaces()`, which installs fresh
    /// surfaces, so the body has to be derived again afterwards.
    ///
    /// **Derive it once per node and keep it**, rather than per ray or per grid point: resolving
    /// the node's ports copies its whole `OpticPorts` (see [`OpticNode::ports`]), which is a
    /// handful of allocations, while the body itself only shares the surfaces it is handed. The
    /// body is what belongs in the loop, not this call.
    ///
    /// Both bounding surfaces are taken in their *physical* order, so an inverted node encloses the
    /// same volume as an upright one: inverting a node reverses the direction light travels, not
    /// the geometry it travels through.
    ///
    /// **Note**: The port [`Aperture`]s have no say in the extent of the body. They mask the light
    /// passing a surface, which is independent of how far the medium behind it reaches.
    ///
    /// # Returns
    ///
    /// The [`SurfaceBoundedBody`] enclosed by the node's entrance and exit surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly one input and one output
    /// port or if either of the two surfaces cannot be found.
    fn volume_body(&self) -> OpmResult<SurfaceBoundedBody> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
        // `OpticNode::ports` hands out the *logical* ports, which are swapped on an inverted node.
        // The body is a geometric object, so the physical order is restored here.
        let (entrance_name, exit_name) = if self.inverted() {
            (out_port_name, in_port_name)
        } else {
            (in_port_name, out_port_name)
        };
        let surface_by_name = |surf_name: &str| {
            self.get_optic_surface(surf_name).ok_or_else(|| {
                OpossumError::Other(format!(
                    "no surface with name {surf_name} defined for node '{}'",
                    self.name()
                ))
            })
        };
        let entrance_surface = surface_by_name(&entrance_name)?;
        let exit_surface = surface_by_name(&exit_name)?;
        Ok(SurfaceBoundedBody::new(
            entrance_surface.geo_surface(),
            exit_surface.geo_surface(),
            cross_section(self)?,
            self.effective_node_iso().unwrap_or_else(Isometry::identity),
        ))
    }
    /// Guides a ray bundle through the volume of a node: in through its entry surface, out through
    /// its exit surface.
    ///
    /// This is the counterpart of [`OpticNodeExt::pass_through_surface_generic`] for the nodes that
    /// enclose a volume of material (lens, wedge, cylindric lens, ...). All of them perform the very
    /// same two-step sequence, which is collected here so that the step in between — what happens
    /// *inside* the medium — exists in exactly one place. Today that step is the amplification of an
    /// active medium ([`Volumetric::amplify_inside`]); the segmentation of the inner path follows
    /// once a model needs it.
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
    /// This function errors if the node's [`Material`] cannot be read, if one of the two surfaces
    /// cannot be found or if the geometric propagation through either of them fails.
    fn pass_through_volume_generic(
        &mut self,
        entry_surf_name: &str,
        exit_surf_name: &str,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        // Behind the exit surface the node's ambient index applies, inside it the material's. The
        // whole material is read rather than just its refractive index because what happens
        // *inside* the volume depends on more than refraction — absorption, thermal and stress
        // data, and later the gain.
        let material_inside = self.material()?;
        let backward = self.inverted();
        // The rays are meant to be refracted at both surfaces of the volume, not just traced to them.
        let refraction_intended = true;
        // Refraction at a surface is governed by the index alone, so only that is handed down.
        self.pass_through_surface_generic(
            entry_surf_name,
            Some(material_inside.refractive_index().clone()),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )?;
        self.amplify_inside(rays_bundle, strategy)?;
        self.pass_through_surface_generic(
            exit_surf_name,
            Some(self.ambient_idx()),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )
    }
    /// Amplify a ray bundle travelling inside this node's medium.
    ///
    /// This is the step *between* the two surface passes: the rays are inside the material here, so
    /// this is where an active medium adds energy to them. Which model applies is not asked of the
    /// node but of the analysis, because whether a component is pumped belongs to the operating
    /// point being analyzed - see
    /// [`PropagationStrategy::gain_model`](crate::analyzers::propagation_strategy::PropagationStrategy::gain_model).
    ///
    /// # Parameters
    ///
    /// * `rays_bundle`: the ray bundle inside the medium, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`], which knows the operating point.
    ///
    /// # Errors
    ///
    /// This function errors if the resulting ray energies would not be finite.
    fn amplify_inside(
        &mut self,
        rays_bundle: &mut [Rays],
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        // Matched exhaustively on purpose: a model that cannot be evaluated from "the ray was in
        // here" alone - anything saturating or path dependent - has to be handled here explicitly
        // rather than falling into a catch-all arm that would silently do nothing.
        match strategy.gain_model(self.node_attr().uuid()) {
            GainModel::None => Ok(()),
            GainModel::Const(const_gain) => {
                // A constant gain is by definition independent of the path through the medium, so
                // every ray of the bundle is multiplied by the same factor, once per pass.
                for rays in rays_bundle.iter_mut() {
                    rays.scale_energy(const_gain.gain())?;
                }
                Ok(())
            }
        }
    }
    /// Amplify the spectral energy passing through this node's medium.
    ///
    /// The energy counterpart of [`Volumetric::amplify_inside`]. An energy flow analysis knows no
    /// rays and no path lengths, so it can only evaluate models that do not need them - which is
    /// exactly what the match below states: a model that depends on the path a beam takes has to
    /// decide here what an energy analysis is supposed to do with it, and until that decision is
    /// made the code does not compile.
    ///
    /// # Parameters
    ///
    /// * `data`: the light data arriving at the node, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`], which knows the operating point.
    ///
    /// # Errors
    ///
    /// This function errors if the amplified spectrum cannot be scaled.
    fn amplify_energy_data(
        &self,
        data: &mut LightData,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        match strategy.gain_model(self.node_attr().uuid()) {
            GainModel::None => Ok(()),
            GainModel::Const(const_gain) => {
                if let LightData::Energy(spectrum) = data {
                    spectrum.scale_vertical(&const_gain.gain())?;
                }
                // Any other kind of light data does not belong to an energy analysis and is left
                // untouched here rather than being reinterpreted.
                Ok(())
            }
        }
    }
    /// A unified helper function to analyze optical nodes that enclose a volume of material.
    ///
    /// This is the volume counterpart of [`OpticNodeExt::unified_analyze_single_surface_node`]: it
    /// resolves the node's single input and output port, unwraps the incoming ray data, guides it
    /// through the volume via [`Volumetric::pass_through_volume_generic`] and packs the result
    /// back onto the output port. All nodes with two surfaces enclosing a medium (lens, wedge,
    /// cylindric lens, ...) share this body, so their `AnalysisRayTrace::analyze` reduces to reading
    /// the medium's material and delegating here.
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
    /// The ghost focus variant of [`Volumetric::unified_analyze_volume_node`].
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
}

/// Determine the transversal boundary of a node's volume from its [`CLEAR_APERTURE`] property.
///
/// The port [`Aperture`]s are deliberately not consulted: an aperture states how much light a
/// surface transmits where and may soften or invert that transmission, which says nothing about how
/// far the material reaches. Masking a component down does not make it smaller.
///
/// # Arguments
///
/// * `node` - the node whose volume is to be bounded.
///
/// # Returns
///
/// The cross section of the node's volume.
///
/// # Errors
///
/// This function returns an error if the node does not declare a clear aperture at all or if that
/// clear aperture does not delimit a region, which leaves the extent of the medium undefined.
fn cross_section<T: ?Sized + Volumetric>(node: &T) -> OpmResult<ValidatedCrossSection> {
    let Ok(Proptype::Aperture(clear_aperture)) = node.node_attr().get_property(CLEAR_APERTURE)
    else {
        return Err(OpossumError::Other(format!(
            "node '{}' has no '{CLEAR_APERTURE}' property, so the extent of its volume is unknown",
            node.name()
        )));
    };
    let aperture = Aperture::new(clear_aperture.clone(), ApertureType::Hole, None, None)?;
    ValidatedCrossSection::try_new(aperture).map_err(|e| {
        OpossumError::Other(format!(
            "the {CLEAR_APERTURE} of node '{}' cannot bound its volume: {e}",
            node.name()
        ))
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            Analyzer, GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy,
            energy::EnergyAnalyzer, energy::EnergyConfig, ghostfocus::AnalysisGhostFocus,
            ghostfocus::GhostFocusAnalyzer, raytrace::AnalysisRayTrace,
            raytrace::RayTracingAnalyzer,
        },
        coatings::CoatingConstantR,
        core_optics::{Alignable, PortType, node_attr::HasNodeAttr},
        degree,
        gain::{ConstGain, PumpScenario},
        joule,
        light::{
            Rays,
            lightdata::energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
            spectrum_helper::create_he_ne_spec,
        },
        millimeter, nanometer,
        nodes::{
            EnergyMeter, Lens, NodeGroup, NodeReference, SourcePort, SpotDiagram, ThinMirror,
            create_node_ref, node_types, round_collimated_ray_builder,
        },
        percent,
        refractive_index::RefrIndexConst,
        utils::{LockExt, test_helper::test_helper::metered_energy},
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use uuid::Uuid;

    /// A lens sitting at the origin, ready to be traced through.
    ///
    /// # Errors
    ///
    /// Returns an error if the lens cannot be placed.
    fn placed_lens() -> OpmResult<Lens> {
        let mut lens = Lens::default();
        lens.set_isometry(Isometry::identity())?;
        Ok(lens)
    }
    /// An operating point in which the node with the given [`Uuid`] amplifies by a constant factor.
    ///
    /// # Errors
    ///
    /// Returns an error if the gain factor is rejected.
    fn scenario_with_gain(node_id: Uuid, gain: f64) -> OpmResult<PumpScenario> {
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(node_id, GainModel::Const(ConstGain::new(gain)?));
        Ok(scenario)
    }
    /// Trace a ray bundle through the given lens and return by how much its energy grew.
    ///
    /// # Errors
    ///
    /// Returns an error if the analysis fails or does not yield ray data.
    fn traced_energy_ratio(lens: &mut Lens, config: &RayTraceConfig) -> OpmResult<f64> {
        let rays = Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &crate::distributions::position::Hexapolar::new(millimeter!(1.0), 1)?,
        )?;
        let energy_before = rays.total_energy();
        let incoming = LightResult::from([("input_1".into(), LightData::Geometric(rays))]);
        let outgoing = AnalysisRayTrace::analyze(lens, incoming, config)?;
        let Some(LightData::Geometric(rays)) = outgoing.get("output_1") else {
            return Err(OpossumError::Analysis(
                "expected ray data at the output port".into(),
            ));
        };
        Ok((rays.total_energy() / energy_before).value)
    }
    /// The point of the whole exercise: a component amplifies because the operating point says so.
    #[test]
    fn a_scenario_amplifies_the_rays_passing_the_medium() -> OpmResult<()> {
        let mut lens = placed_lens()?;
        let node_id = lens.node_attr().uuid();
        // Without an operating point the very same lens is passive, so the two runs differ by
        // nothing but the scenario.
        assert_abs_diff_eq!(
            traced_energy_ratio(&mut lens, &RayTraceConfig::default())?,
            1.0,
            epsilon = 1e-12
        );
        let mut config = RayTraceConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_gain(node_id, 2.5)?));
        assert_abs_diff_eq!(
            traced_energy_ratio(&mut lens, &config)?,
            2.5,
            epsilon = 1e-12
        );
        Ok(())
    }
    /// A scenario amplifies the nodes it names and nothing else.
    #[test]
    fn a_node_the_scenario_does_not_name_stays_passive() -> OpmResult<()> {
        let mut lens = placed_lens()?;
        let mut config = RayTraceConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_gain(Uuid::new_v4(), 2.5)?));
        assert_abs_diff_eq!(
            traced_energy_ratio(&mut lens, &config)?,
            1.0,
            epsilon = 1e-12
        );
        Ok(())
    }
    /// An energy flow analysis has to amplify too - a constant gain needs no ray to be evaluated,
    /// and leaving it out would silently report an amplifier chain as passive.
    #[test]
    fn a_scenario_amplifies_the_energy_flow() -> OpmResult<()> {
        let mut lens = placed_lens()?;
        let mut config = EnergyConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_gain(lens.node_attr().uuid(), 2.5)?));
        let spectrum = create_he_ne_spec(1.0)?;
        let energy_before = spectrum.total_energy();
        let incoming = LightResult::from([("input_1".into(), LightData::Energy(spectrum))]);
        let outgoing = AnalysisEnergy::analyze(&mut lens, incoming, &config)?;
        let Some(LightData::Energy(spectrum)) = outgoing.get("output_1") else {
            panic!("expected energy data at the output port");
        };
        assert_abs_diff_eq!(
            spectrum.total_energy() / energy_before,
            2.5,
            epsilon = 1e-12
        );
        Ok(())
    }
    /// Trace a collimated bundle of 1 J through the given model and return the energy its
    /// [`EnergyMeter`] recorded.
    ///
    /// The model is analyzed as a whole rather than node by node, which is what places the nodes at
    /// the distances they are connected with - the amplifying nodes therefore sit where a real
    /// layout would put them instead of all sharing one position.
    ///
    /// The model is cleared out before the run, as
    /// [`OpmDocument::analyze`](crate::opm_document::OpmDocument::analyze) does between two
    /// operating points, so the same model may be measured in several scenarios in a row and every
    /// run starts from the same state.
    ///
    /// # Errors
    ///
    /// Returns an error if the analysis fails or its report holds no energy reading.
    fn metered_energy_of(
        model: &mut NodeGroup,
        source: Uuid,
        gains: PumpScenario,
    ) -> OpmResult<f64> {
        model.clear_edges();
        model.reset_data();
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 3)?,
        );
        config.set_active_pump_scenario(Some(gains));
        let analyzer = RayTracingAnalyzer::new(config);
        analyzer.analyze(model)?;
        metered_energy(&analyzer.report(model)?)
    }
    /// The energy flow counterpart of [`metered_energy_of`]: 1 J into the model, the
    /// [`EnergyMeter`]'s reading out of it.
    ///
    /// # Errors
    ///
    /// Returns an error if the analysis fails or its report holds no energy reading.
    fn metered_energy_of_energy_flow(
        model: &mut NodeGroup,
        source: Uuid,
        gains: PumpScenario,
    ) -> OpmResult<f64> {
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        config.set_active_pump_scenario(Some(gains));
        let analyzer = EnergyAnalyzer::new(config);
        analyzer.analyze(model)?;
        metered_energy(&analyzer.report(model)?)
    }
    /// Several amplifiers in a row multiply, so a chain's overall gain is the product of its stages.
    ///
    /// This is the layout an amplifier chain is actually built as, and the first case where one
    /// scenario has to hand each of several nodes *its own* factor - a single lookup reused for the
    /// whole run would pass the single-stage test and fail here.
    #[test]
    fn a_chain_of_amplifiers_multiplies_their_factors() -> OpmResult<()> {
        let gains = [2.0, 3.0, 5.0];
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let mut scenario = PumpScenario::new("full power");
        let mut upstream = source;
        for gain in gains {
            let stage = model.add_node(Lens::default())?;
            model.connect_nodes(upstream, "output_1", stage, "input_1", millimeter!(20.0))?;
            scenario.set_gain_model(stage, GainModel::Const(ConstGain::new(gain)?));
            upstream = stage;
        }
        let meter = model.add_node(EnergyMeter::default())?;
        model.connect_nodes(upstream, "output_1", meter, "input_1", millimeter!(20.0))?;

        assert_relative_eq!(
            metered_energy_of(&mut model, source, scenario)?,
            gains.iter().product::<f64>(),
            epsilon = 1e-9
        );
        Ok(())
    }
    /// A multipass amplifier: the same head, passed several times, gains its factor once per pass.
    ///
    /// Built from [`NodeReference`]s, which need no amplification machinery of their own - what this
    /// pins down is that the scenario is looked up under the uuid of the node that *has* the medium,
    /// not under the uuid of the reference standing in for it. Were it the latter, a multipass
    /// amplifier would silently run passive on every pass but the first.
    ///
    /// Analyzed as an energy flow, like the reference node's own example
    /// (`examples/reference_test.rs`): a reference chained in a straight line is a statement about
    /// how often the light passes a component, not about where that component is - which is exactly
    /// what an energy analysis asks. Sending rays through it instead would need the beam folded back
    /// onto the head by real mirrors, since the head keeps the one position it was placed at.
    #[test]
    fn passing_the_same_amplifier_again_amplifies_again() -> OpmResult<()> {
        let passes = 3;
        let gain = 2.0;
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let head = model.add_node(Lens::default())?;
        model.connect_nodes(source, "output_1", head, "input_1", millimeter!(20.0))?;
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(head, GainModel::Const(ConstGain::new(gain)?));

        let mut upstream = head;
        for _ in 1..passes {
            let further_pass = model.add_node(NodeReference::from_node(&model.node(head)?)?)?;
            model.connect_nodes(
                upstream,
                "output_1",
                further_pass,
                "input_1",
                millimeter!(20.0),
            )?;
            upstream = further_pass;
        }
        let meter = model.add_node(EnergyMeter::default())?;
        model.connect_nodes(upstream, "output_1", meter, "input_1", millimeter!(20.0))?;

        assert_relative_eq!(
            metered_energy_of_energy_flow(&mut model, source, scenario)?,
            gain.powi(passes),
            epsilon = 1e-9
        );
        Ok(())
    }
    /// A folded double pass: a mirror sends the beam back through the very same head, and the rays
    /// leave with the gain applied twice.
    ///
    /// The ray trace counterpart of [`passing_the_same_amplifier_again_amplifies_again`], and the
    /// one that pins down the geometry as well as the bookkeeping: here the second pass really
    /// traverses the medium a second time, entering it through the surface the first pass left by.
    /// The head is therefore reached as an *inverted* [`NodeReference`], which is what turns its
    /// ports around - it is entered through the port it normally leaves by, exactly as
    /// `examples/lens_inverse.rs` builds the same fold.
    ///
    /// The passive run is measured first. Both passes are lossless without a scenario, so anything
    /// the second measurement shows above 1 J came from the operating point rather than from the
    /// fold - without that baseline a mirror quietly swallowing rays would be indistinguishable
    /// from an amplifier that only ran once.
    #[test]
    fn a_mirror_folding_the_beam_back_amplifies_on_both_passes() -> OpmResult<()> {
        let gain = 2.0;
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let head = model.add_node(Lens::default())?;
        // Tilted, so the returning beam is separated from the incoming one instead of running back
        // into the source - which is what a real double pass does too.
        let fold = model.add_node(ThinMirror::new("fold").with_tilt(degree!(2.0, 0.0, 0.0))?)?;
        let mut second_pass = NodeReference::from_node(&model.node(head)?)?;
        second_pass.set_inverted(true)?;
        let second_pass = model.add_node(second_pass)?;
        let meter = model.add_node(EnergyMeter::default())?;
        model.connect_nodes(source, "output_1", head, "input_1", millimeter!(30.0))?;
        model.connect_nodes(head, "output_1", fold, "input_1", millimeter!(50.0))?;
        model.connect_nodes(fold, "output_1", second_pass, "output_1", millimeter!(50.0))?;
        model.connect_nodes(second_pass, "input_1", meter, "input_1", millimeter!(30.0))?;

        assert_relative_eq!(
            metered_energy_of(&mut model, source, PumpScenario::new("cold"))?,
            1.0,
            epsilon = 1e-9
        );
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(head, GainModel::Const(ConstGain::new(gain)?));
        assert_relative_eq!(
            metered_energy_of(&mut model, source, scenario)?,
            gain * gain,
            epsilon = 1e-9
        );
        Ok(())
    }
    /// The reflectivity both faces of the amplifier head in the ghost focus test carry.
    ///
    /// An uncoated glass surface at normal incidence, which is what makes the ghost paths below
    /// exist in the first place.
    const GHOST_REFLECTIVITY: f64 = 0.04;
    /// Run a ghost focus analysis on an amplifier head reflecting [`GHOST_REFLECTIVITY`] at both
    /// faces, and return the energy accumulated at each bounce level.
    ///
    /// The head is plane on both sides, so every reflection runs straight back the way it came and
    /// the ghost paths are the ones written out in
    /// [`ghost_reflections_are_amplified_on_every_pass_through_the_medium`] - no focusing, no
    /// walk-off, nothing to obscure the energy bookkeeping.
    ///
    /// # Arguments
    ///
    /// * `gain` - the factor the head amplifies by. 1.0 makes it a passive lens.
    /// * `max_bounces` - how many reflections the analysis follows.
    ///
    /// # Returns
    ///
    /// The total energy of all ray bundles accumulated at bounce 0, 1, ... in joule.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be built or the analysis fails.
    fn ghost_energies_per_bounce(gain: f64, max_bounces: usize) -> OpmResult<Vec<f64>> {
        let plane = millimeter!(f64::INFINITY);
        let mut head = Lens::new(
            "head",
            plane,
            plane,
            millimeter!(10.0),
            RefrIndexConst::new(1.5)?,
        )?;
        for (port_type, port_name) in [(PortType::Input, "input_1"), (PortType::Output, "output_1")]
        {
            head.set_coating(
                &port_type,
                port_name,
                &CoatingConstantR::new(percent!(GHOST_REFLECTIVITY * 100.0))?.into(),
            )?;
        }
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let head = model.add_node(head)?;
        let screen = model.add_node(SpotDiagram::default())?;
        model.connect_nodes(source, "output_1", head, "input_1", millimeter!(30.0))?;
        model.connect_nodes(head, "output_1", screen, "input_1", millimeter!(30.0))?;

        let mut config = GhostFocusConfig::default();
        config.set_max_bounces(max_bounces);
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(head, GainModel::Const(ConstGain::new(gain)?));
        config.set_active_pump_scenario(Some(scenario));

        let analyzer = GhostFocusAnalyzer::new(config);
        analyzer.analyze(&mut model)?;
        Ok(model
            .accumulated_rays()
            .iter()
            .map(|bundles| {
                bundles
                    .values()
                    .map(|rays| rays.total_energy().value)
                    .sum::<f64>()
            })
            .collect())
    }
    /// Every traversal of a pumped medium multiplies by the gain - the ghost paths included.
    ///
    /// This is the case the ghost focus analysis exists for, and the one where getting the gain
    /// wrong is most consequential: a ghost is dangerous because of the fluence it carries, and a
    /// ghost that ran through an amplifier twice more than the main beam carries orders of magnitude
    /// more of it. Counting its passes wrong understates exactly the hazard being looked for.
    ///
    /// With both faces reflecting `R` and transmitting `T = 1 - R`, the paths up to the second
    /// bounce are, written out with one `G` per traversal of the medium:
    ///
    /// - **bounce 0** - straight through: `T·G·T`
    /// - **bounce 1** - the front-face reflection, which never enters the medium at all (`R`), plus
    ///   the ghost that entered, reflected off the rear face and came back out (`T·G·R·G·T`)
    /// - **bounce 2** - that ghost reflected once more off the front face from inside and sent
    ///   forward again: `T·G·R·G·R·G·T`
    ///
    /// The same formulas with `G = 1` describe the passive head, which is checked first: it pins
    /// down the reflection bookkeeping itself, so the pumped run can only differ by the gain.
    #[test]
    fn ghost_reflections_are_amplified_on_every_pass_through_the_medium() -> OpmResult<()> {
        let r = GHOST_REFLECTIVITY;
        let t = 1.0 - r;
        // The powers of `g` are the number of times each path crosses the medium - which is the
        // whole assertion here, so they are spelled out rather than folded into an exponent.
        let expected_energies = |g: f64| {
            let straight_through = t * g * t;
            let front_face_reflection = r;
            let ghost_off_the_rear_face = t * g * r * g * t;
            let ghost_reflected_once_more = t * g * r * g * r * g * t;
            [
                straight_through,
                front_face_reflection + ghost_off_the_rear_face,
                ghost_reflected_once_more,
            ]
        };
        for gain in [1.0, 2.0, 3.0] {
            let measured = ghost_energies_per_bounce(gain, 2)?;
            let expected = expected_energies(gain);
            assert_eq!(
                measured.len(),
                expected.len(),
                "expected one energy per bounce level up to the second"
            );
            for (bounce, (measured, expected)) in measured.iter().zip(expected.iter()).enumerate() {
                assert_relative_eq!(measured, expected, epsilon = 1e-9);
                assert!(
                    *measured > 0.0,
                    "bounce {bounce} carries no energy at all with a gain of {gain}"
                );
            }
        }
        Ok(())
    }
    /// A ghost focus analysis traverses the very same medium and has to amplify there too.
    ///
    /// It reaches the volume through an entry point of its own
    /// ([`Volumetric::unified_analyze_volume_node_ghost_focus`]), so "the ray trace amplifies" says
    /// nothing about it: a stray reflection running through a pumped head picks up its gain like any
    /// other pass, and reporting it as if it did not is exactly the error that analysis exists to
    /// catch.
    #[test]
    fn a_scenario_amplifies_a_ghost_focus_pass() -> OpmResult<()> {
        let mut lens = placed_lens()?;
        let mut config = GhostFocusConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_gain(lens.node_attr().uuid(), 2.5)?));
        let rays = Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &crate::distributions::position::Hexapolar::new(millimeter!(1.0), 1)?,
        )?;
        let energy_before = rays.total_energy();
        let incoming = LightRays::from([("input_1".into(), vec![rays])]);
        let outgoing =
            AnalysisGhostFocus::analyze(&mut lens, incoming, &config, &mut Vec::new(), 0)?;
        let energy_after: uom::si::f64::Energy = outgoing
            .get("output_1")
            .ok_or_else(|| OpossumError::Analysis("expected rays at the output port".into()))?
            .iter()
            .map(Rays::total_energy)
            .sum();
        assert_abs_diff_eq!((energy_after / energy_before).value, 2.5, epsilon = 1e-12);
        Ok(())
    }
    /// Only the nodes that really enclose a medium may present themselves as [`Volumetric`].
    ///
    /// "Node with a volume" is stated twice: by this capability and by the transversal extent of
    /// its medium ([`CLEAR_APERTURE`]), a property such a node carries. Both have to mean the same
    /// set of node types, otherwise a node ends up with a body whose extent is undefined.
    #[test]
    fn the_volume_capability_matches_the_volume_properties() -> OpmResult<()> {
        for (node_type, _) in node_types() {
            let optic_ref = create_node_ref(node_type)?;
            let node = optic_ref.optical_ref.lock_opm()?;
            let is_volumetric = node.as_volume().is_some();
            assert_eq!(
                node.node_attr().get_property(CLEAR_APERTURE).is_ok(),
                is_volumetric,
                "node type '{node_type}' declares '{CLEAR_APERTURE}' or presents itself as \
                 volumetric, but not both"
            );
        }
        Ok(())
    }
}
