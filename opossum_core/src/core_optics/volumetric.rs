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
            RayTraceConfig, energy::AnalysisEnergy, energy::EnergyConfig,
            raytrace::AnalysisRayTrace,
        },
        core_optics::node_attr::HasNodeAttr,
        gain::{ConstGain, PumpScenario},
        joule,
        light::{Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::{Lens, create_node_ref, node_types},
        utils::LockExt,
    };
    use approx::assert_abs_diff_eq;
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
