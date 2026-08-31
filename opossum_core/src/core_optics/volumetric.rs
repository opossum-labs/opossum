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
    core_optics::{
        NodeAttrExt, OpticNode, OpticNodeExt, node_attr::RuntimeMedium,
        optic_node_ext::single_io_port_names,
    },
    error::{OpmResult, OpossumError},
    gain::Extraction,
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
    /// *inside* the medium — exists in exactly one place. That step is
    /// [`Volumetric::propagate_inside_medium`]: for a passive node it returns immediately; for an
    /// active one it walks the chord in substeps and scales each ray's energy accordingly. The rays
    /// are **not** moved by it: they are still carried from one surface to the other by the exit pass
    /// below, exactly as they are through a passive component.
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
        // data, and potentially gain.
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
        let ambient_refr_idx = strategy.ambient_refractive_index();

        // The positioning-run guard lives inside `propagate_inside_medium`; no outer check here.
        self.propagate_inside_medium(rays_bundle, strategy)?;
        self.pass_through_surface_generic(
            exit_surf_name,
            Some(ambient_refr_idx),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )
    }
    /// Apply whatever the medium does to a ray bundle travelling through it.
    ///
    /// This is the step *between* the two surface passes: the rays are inside the material here.
    /// What that means depends on the operating point — for a passive node (no gain model in the
    /// current [`PropagationStrategy`]) the call returns immediately without touching the rays. For
    /// an active node it queries the gain model from
    /// [`PropagationStrategy::pump_config`](crate::analyzers::propagation_strategy::PropagationStrategy::pump_config)
    /// and amplifies along the chord through the medium.
    ///
    /// Returns immediately if [`PropagationStrategy::is_positioning_run`] is `true` — no medium
    /// has been prepared yet and the step is skipped entirely. After the positioning run, a missing
    /// medium on an active node is a programming error and causes an [`OpossumError::Analysis`].
    ///
    /// # Parameters
    ///
    /// * `rays_bundle`: the ray bundle inside the medium, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`], which knows the operating point.
    ///
    /// # Errors
    ///
    /// This function errors if the medium was not prepared before this call on an active node
    /// (outside a positioning run), or if the resulting ray energies would not be finite.
    fn propagate_inside_medium(
        &mut self,
        rays_bundle: &mut [Rays],
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let config = strategy.pump_config(self.node_attr().uuid());
        let gain_model = config.gain_model();
        // A passive node — no gain model in the current operating point — leaves the rays untouched.
        let Some(extraction) = gain_model.as_extraction() else {
            return Ok(());
        };
        if strategy.is_positioning_run() {
            return Ok(());
        }
        // Capture the name before the mutable borrow of `self` through `node_attr_mut`.
        let node_name = self.name().to_owned();
        let Some(medium) = self.node_attr_mut().runtime_medium_mut() else {
            return Err(OpossumError::Analysis(format!(
                "node '{node_name}': medium was not prepared before propagation"
            )));
        };
        march_extraction_along_chords(&node_name, medium, extraction, rays_bundle)
    }
    /// Amplify the spectral energy passing through this node's medium.
    ///
    /// The energy counterpart of [`Volumetric::propagate_inside_medium`]. An energy flow analysis
    /// knows no rays and no path lengths — for a passive node (no gain model) it returns
    /// immediately. For an active node, the gain model's
    /// [`Extraction::amplify_spectrum`](crate::gain::Extraction::amplify_spectrum) decides what to
    /// do without a beam path: state a nominal path length, or refuse.
    ///
    /// Reads from the medium prepared by
    /// [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume) rather than
    /// rebuilding the body and inversion on every call. Returns immediately if no medium has been
    /// prepared yet (positioning run).
    ///
    /// # Parameters
    ///
    /// * `data`: the light data arriving at the node, modified in place.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`], which knows the operating point.
    ///
    /// # Errors
    ///
    /// This function errors if the spectrum cannot be scaled, or if the active gain model refuses
    /// energy analysis (e.g. [`MonochromaticSmallSignalGain`](crate::gain::MonochromaticSmallSignalGain)).
    fn propagate_energy_inside_medium(
        &self,
        data: &mut LightData,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let gain_model = strategy.pump_config(self.node_attr().uuid()).gain_model();
        let Some(extraction) = gain_model.as_extraction() else {
            return Ok(());
        };
        // Any other kind of light data does not belong to an energy analysis and is left untouched
        // here rather than being reinterpreted.
        let LightData::Energy(spectrum) = data else {
            return Ok(());
        };
        if strategy.is_positioning_run() {
            return Ok(());
        }
        let Some(medium) = self.node_attr().runtime_medium() else {
            return Err(OpossumError::Analysis(format!(
                "node '{}': medium was not prepared before amplification",
                self.name()
            )));
        };
        extraction
            .amplify_spectrum(medium.body(), medium.inversion(), spectrum)
            .map_err(|e| OpossumError::Analysis(format!("node '{}': {e}", self.name())))
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

/// Apply a gain extraction model to each ray that travels through the medium.
///
/// This is the mechanism half of [`Volumetric::propagate_inside_medium`]: it applies the gain
/// without asking *whether* to run — that decision belongs to the caller, which has already
/// confirmed the operating point is active and a medium is available.
///
/// For each valid ray with a positive chord through the body, the gain exponent is computed by
/// [`Extraction::path_exponent`] (which performs an exact Amanatides–Woo voxel walk for
/// grid-based models) and the ray energy is scaled by `exp(path_exponent)`.
///
/// # Arguments
///
/// * `node_name` - name of the owning node, used in error messages.
/// * `medium` - the prepared medium for this analysis run, split into body and inversion.
/// * `extraction` - the gain model to apply.
/// * `rays_bundle` - the ray bundle inside the medium, modified in place.
///
/// # Errors
///
/// This function errors if `path_length_inside` fails, if the exponent would produce a non-finite
/// scale factor, or if `scale_energy` rejects the resulting value.
fn march_extraction_along_chords(
    node_name: &str,
    medium: &mut RuntimeMedium,
    extraction: &dyn Extraction,
    rays_bundle: &mut [Rays],
) -> OpmResult<()> {
    // Split the medium so saturating models can read and deplete the inversion per traversed cell.
    let (body, inversion) = medium.parts_mut();
    for rays in rays_bundle.iter_mut() {
        for ray in rays.iter_mut() {
            if !ray.valid() {
                continue;
            }
            let Some(chord) = body
                .path_length_inside(ray)
                .map_err(|e| OpossumError::Analysis(format!("node '{node_name}': {e}")))?
            else {
                continue;
            };
            if chord.value <= 0.0 {
                continue;
            }
            let exponent = extraction.path_exponent(body, ray, inversion);
            let factor = exponent.exp();
            if !factor.is_finite() {
                return Err(OpossumError::Analysis(format!(
                    "node '{node_name}': would amplify by exp({exponent}) over a path of \
                     {} mm through the medium, which is not a finite factor",
                    chord.get::<uom::si::length::millimeter>()
                )));
            }
            ray.scale_energy(factor)
                .map_err(|e| OpossumError::Analysis(format!("node '{node_name}': {e}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            Analyzer, GhostFocusConfig, RayTraceConfig,
            energy::{AnalysisEnergy, EnergyAnalyzer, EnergyConfig},
            ghostfocus::{AnalysisGhostFocus, GhostFocusAnalyzer},
            raytrace::{AnalysisRayTrace, RayTracingAnalyzer},
        },
        coatings::CoatingConstantR,
        core_optics::{Alignable, PortType, node_attr::HasNodeAttr},
        degree,
        gain::{ConstGain, GainModel, MonochromaticSmallSignalGain, PumpScenario, PumpSource},
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
        percent, reciprocal_centimeter,
        refractive_index::RefrIndexConst,
        utils::{LockExt, test_helper::test_helper::metered_energy},
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use uom::si::f64::ReciprocalLength;
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
        Ok(scenario_with_model(
            node_id,
            GainModel::Const(ConstGain::new(gain)?),
            PumpSource::None,
        ))
    }
    /// An operating point in which the node with the given [`Uuid`] runs the given configuration.
    ///
    /// The general form of [`scenario_with_gain`]: a model reading the medium's state needs the
    /// pumping alongside it, and both have to come out of the same scenario.
    fn scenario_with_model(node_id: Uuid, model: GainModel, pump: PumpSource) -> PumpScenario {
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(node_id, model);
        scenario.set_pump_source(node_id, pump);
        scenario
    }
    /// The thickness of the plane-parallel amplifier head the small signal tests trace through.
    const HEAD_THICKNESS: f64 = 10.0;
    /// The refractive index of that head's material.
    const HEAD_INDEX: f64 = 1.5;
    /// How far the fold mirror of the double pass test is tilted, in degrees.
    ///
    /// It has to be tilted at all so the returning beam is separated from the incoming one instead
    /// of running back into the source - which is what a real double pass does too. The consequence
    /// is that the second pass crosses the medium slightly obliquely, and the test has to account
    /// for it.
    const FOLD_TILT: f64 = 2.0;
    /// The gain coefficient the medium of that head is pumped to.
    fn head_gain_coefficient() -> ReciprocalLength {
        reciprocal_centimeter!(0.5)
    }
    /// The factor a single on-axis pass through that head amplifies by: `G = exp(g₀·d)`.
    ///
    /// Worked out from the formula rather than hard coded, so the expectation is a statement of the
    /// physics rather than a number somebody once measured.
    fn single_pass_gain() -> f64 {
        f64::exp((head_gain_coefficient() * millimeter!(HEAD_THICKNESS)).value)
    }
    /// A plane-parallel amplifier head of [`HEAD_THICKNESS`], which a collimated bundle crosses
    /// without being bent - so the chord of every ray really is the centre thickness.
    fn amplifier_head() -> OpmResult<Lens> {
        let plane = millimeter!(f64::INFINITY);
        Lens::new(
            "head",
            plane,
            plane,
            millimeter!(HEAD_THICKNESS),
            RefrIndexConst::new(HEAD_INDEX)?,
        )
    }
    /// The factor a pass crossing that head at the given external angle amplifies by.
    ///
    /// The generalisation of [`single_pass_gain`], which is this at normal incidence: refraction at
    /// the entrance face bends the beam to `asin(sin θ / n)` inside the medium, so it travels
    /// `d / cos` of that rather than `d`, and the gain follows the longer chord. This is precisely
    /// what a constant factor cannot express, so a test using it is asserting the new capability
    /// rather than repeating the old one.
    fn gain_at_angle(external_degrees: f64) -> f64 {
        let internal = (external_degrees.to_radians().sin() / HEAD_INDEX).asin();
        f64::exp((head_gain_coefficient() * millimeter!(HEAD_THICKNESS)).value / internal.cos())
    }
    /// An operating point running the given node as a uniformly pumped small signal amplifier.
    ///
    /// The magnitude lives on the model now (`peak_gain_coefficient`), and the const pump just says
    /// "uniformly inverted" — so the head amplifies over the exact chord without a voxel grid.
    fn scenario_with_small_signal(node_id: Uuid) -> OpmResult<PumpScenario> {
        Ok(scenario_with_model(
            node_id,
            GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
                head_gain_coefficient(),
            )?),
            PumpSource::Const,
        ))
    }
    fn retraced_energy_value(lens: &mut Lens, config: &RayTraceConfig) -> OpmResult<f64> {
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
    /// Trace a ray bundle through the given lens and return by how much its energy grew.
    ///
    /// # Errors
    ///
    /// Returns an error if the analysis fails or does not yield ray data.
    fn traced_energy_ratio(lens: &mut Lens, config: &RayTraceConfig) -> OpmResult<f64> {
        lens.prepare_volume(config)?;
        retraced_energy_value(lens, config)
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
    /// A passive volume node gets a prepared body even without a gain model.
    ///
    /// This is the regression test for the new `prepare_volume` contract: the body is geometry, so
    /// it is built for every volume node regardless of the operating point. Previously a passive lens
    /// (no active scenario) returned from `prepare_volume` immediately, leaving `runtime_medium`
    /// unset. Now it is always set — the inversion slot is `None`, but the body is there.
    ///
    /// The second assertion (energy ratio `1.0`) pins the "stays passive" guarantee: a body in the
    /// medium slot must not cause any energy change when no gain model is present.
    #[test]
    fn a_passive_volume_node_has_a_prepared_body_after_prepare_volume() -> OpmResult<()> {
        let mut lens = placed_lens()?;
        let config = RayTraceConfig::default(); // no active pump scenario
        lens.prepare_volume(&config)?;
        // The body is now available even though no gain model was set.
        assert!(
            lens.node_attr().runtime_medium().is_some(),
            "prepare_volume must build the body for a passive volume node"
        );
        // And the inversion slot is empty — the body alone does nothing to the rays.
        assert!(
            lens.node_attr()
                .runtime_medium()
                .and_then(|m| m.inversion())
                .is_none(),
            "a passive node must not have an inversion field"
        );
        // The passive contract: no gain model → no energy change, even with a prepared body.
        assert_abs_diff_eq!(
            retraced_energy_value(&mut lens, &config)?,
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
        lens.prepare_volume(&config)?;
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
    /// * `model` - how the head amplifies. [`GainModel::None`] makes it a passive lens.
    /// * `pump` - how its medium is pumped, which only a model reading the inversion cares about.
    /// * `max_bounces` - how many reflections the analysis follows.
    ///
    /// # Returns
    ///
    /// The total energy of all ray bundles accumulated at bounce 0, 1, ... in joule.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be built or the analysis fails.
    fn ghost_energies_per_bounce(
        gain_model: GainModel,
        pump: PumpSource,
        max_bounces: usize,
    ) -> OpmResult<Vec<f64>> {
        let mut head = amplifier_head()?;
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
        config.set_active_pump_scenario(Some(scenario_with_model(head, gain_model, pump)));

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
            let measured = ghost_energies_per_bounce(
                GainModel::Const(ConstGain::new(gain)?),
                PumpSource::None,
                2,
            )?;
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
        lens.prepare_volume(&config)?;
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
    /// The acceptance test of the small signal stage: a real amplifier head against a hand
    /// calculation.
    ///
    /// A plane-parallel head of 10 mm, uniformly pumped to g₀ = 0.5 / cm, amplifies a collimated
    /// bundle by `exp(0.5) ≈ 1.6487`. Nothing about the number is hard coded - it is worked out from
    /// the formula in [`single_pass_gain`], so the test states the physics rather than a
    /// measurement.
    #[test]
    fn a_small_signal_scenario_amplifies_by_exp_of_the_path() -> OpmResult<()> {
        let mut head = amplifier_head()?;
        head.set_isometry(Isometry::identity())?;
        let node_id = head.node_attr().uuid();
        // Passive without a scenario, so the two runs differ by nothing but the operating point.
        assert_relative_eq!(
            traced_energy_ratio(&mut head, &RayTraceConfig::default())?,
            1.0,
            epsilon = 1e-9
        );
        let mut config = RayTraceConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_small_signal(node_id)?));
        assert_relative_eq!(
            traced_energy_ratio(&mut head, &config)?,
            single_pass_gain(),
            epsilon = 1e-9
        );
        Ok(())
    }
    /// `prepare_volume` always uses the current node position, not a body cached at an earlier one.
    ///
    /// This is the regression test for the bug commit `b1f766b8` identified but incompletely fixed.
    /// `init_runtime_medium` (called from `after_deserialization_hook`) built and stored a body at
    /// the node's position at deserialization time — which is the origin, because `set_isometry`
    /// from `calc_node_positions` had not run yet — and `prepare_volume`'s first branch then reused
    /// that stale body for all subsequent runs. A head placed anywhere but the origin had its
    /// inversion field laid out at the wrong coordinates, so `gain_exponent_at` found no inversion
    /// for any real ray and returned exactly `0.0`, giving a gain factor of `1.0`.
    ///
    /// After the fix: `prepare_volume` always re-derives the body from the current geometry, so the
    /// inversion and the sampling frame are always consistent with the node's actual position.
    #[test]
    fn prepare_volume_uses_current_position_not_a_cached_body() -> OpmResult<()> {
        let mut head = amplifier_head()?;
        // Start with the head at identity.
        head.set_isometry(Isometry::identity())?;
        let node_id = head.node_attr().uuid();

        // Seed a body at the current (identity) position, simulating what
        // `init_runtime_medium` used to do inside `after_deserialization_hook`.
        let identity_body = head.as_volume().unwrap().volume_body()?;
        head.node_attr_mut().set_runtime_medium(identity_body, None);

        // Move the head 500 mm along z — what `calc_node_positions` does after deserialization.
        head.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 500.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        // Simulate `reset_data`: clear only the inversion, leave the (stale, identity) body.
        head.node_attr_mut().clear_runtime_inversion();

        let mut config = RayTraceConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_small_signal(node_id)?));

        // `traced_energy_ratio` calls `prepare_volume` and then analyzes.
        // Before the fix: `prepare_volume` saw `runtime_medium().is_some()` and reused the
        // identity body; `gain_exponent_at` sampled at the real position, found no inversion, and
        // returned `0.0` — giving factor `1.0`, a silently passive amplifier.
        // After the fix: `prepare_volume` always rebuilds the body from the 500 mm isometry,
        // so the inversion covers the real position and the result is the expected gain.
        assert_relative_eq!(
            traced_energy_ratio(&mut head, &config)?,
            single_pass_gain(),
            epsilon = 1e-9
        );
        Ok(())
    }
    /// The two halves of an operating point really are independent.
    ///
    /// A gain model that reads the medium finds nothing there if nobody pumped it, and a medium
    /// nobody pumped must come out exactly as passive as it went in - not "almost", and not with the
    /// model's own parameters leaking into the result.
    #[test]
    fn a_small_signal_head_is_passive_without_a_pump() -> OpmResult<()> {
        let mut head = amplifier_head()?;
        head.set_isometry(Isometry::identity())?;
        let mut config = RayTraceConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_model(
            head.node_attr().uuid(),
            // A model that would amplify given a pump: the point is that no pump leaves it passive.
            GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
                head_gain_coefficient(),
            )?),
            PumpSource::None,
        )));
        assert_relative_eq!(
            traced_energy_ratio(&mut head, &config)?,
            1.0,
            epsilon = 1e-12
        );
        Ok(())
    }
    /// An energy flow analysis has no rays and therefore no path to integrate the gain along.
    ///
    /// It says so instead of guessing: silently amplifying by nothing would report a pumped chain as
    /// passive, and silently picking some path length would invent a number nobody stated.
    #[test]
    fn an_energy_flow_refuses_a_path_dependent_model() -> OpmResult<()> {
        let mut head = amplifier_head()?;
        head.set_isometry(Isometry::identity())?;
        let mut config = EnergyConfig::default();
        config.set_active_pump_scenario(Some(scenario_with_small_signal(head.node_attr().uuid())?));
        head.prepare_volume(&config)?;
        let incoming =
            LightResult::from([("input_1".into(), LightData::Energy(create_he_ne_spec(1.0)?))]);
        let result = AnalysisEnergy::analyze(&mut head, incoming, &config);
        assert!(
            result.is_err(),
            "an energy flow analysis must not silently evaluate a path dependent gain"
        );
        Ok(())
    }
    /// A folded double pass through a pumped medium amplifies on both passes.
    ///
    /// The small signal counterpart of
    /// [`a_mirror_folding_the_beam_back_amplifies_on_both_passes`], and the test that pins down the
    /// march through an **inverted** node: the returning beam enters the head through the face the
    /// first pass left by. The body is geometry and keeps its physical orientation either way, so
    /// the march has to work in both directions.
    ///
    /// The two passes do **not** gain the same amount, and that is the point. The fold mirror is
    /// tilted by [`FOLD_TILT`], so it sends the beam back at twice that angle and the second pass
    /// crosses the medium obliquely - over a chord longer by `1/cos` of the refracted angle. A
    /// constant gain is blind to this; a path dependent one must not be, so the expectation is
    /// built from the two angles rather than from twice the same factor.
    #[test]
    fn a_folded_double_pass_amplifies_small_signal_on_both_passes() -> OpmResult<()> {
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let head = model.add_node(amplifier_head()?)?;
        let fold =
            model.add_node(ThinMirror::new("fold").with_tilt(degree!(FOLD_TILT, 0.0, 0.0))?)?;
        let mut second_pass = NodeReference::from_node(&model.node(head)?)?;
        second_pass.set_inverted(true)?;
        let second_pass = model.add_node(second_pass)?;
        let meter = model.add_node(EnergyMeter::default())?;
        model.connect_nodes(source, "output_1", head, "input_1", millimeter!(30.0))?;
        model.connect_nodes(head, "output_1", fold, "input_1", millimeter!(50.0))?;
        model.connect_nodes(fold, "output_1", second_pass, "output_1", millimeter!(50.0))?;
        model.connect_nodes(second_pass, "input_1", meter, "input_1", millimeter!(30.0))?;

        // The passive baseline first: both passes are lossless without a scenario, so anything the
        // pumped run shows above 1 J came from the operating point rather than from the fold.
        assert_relative_eq!(
            metered_energy_of(&mut model, source, PumpScenario::new("cold"))?,
            1.0,
            epsilon = 1e-9
        );
        assert_relative_eq!(
            metered_energy_of(&mut model, source, scenario_with_small_signal(head)?)?,
            single_pass_gain() * gain_at_angle(2.0 * FOLD_TILT),
            epsilon = 1e-9
        );
        Ok(())
    }
    /// Ghost paths through a pumped medium pick up the small signal gain on every traversal.
    ///
    /// The same path algebra as
    /// [`ghost_reflections_are_amplified_on_every_pass_through_the_medium`], with `G = exp(g₀·d)`
    /// instead of a stated factor. Worth checking separately: the ghost focus analysis reaches the
    /// volume through an entry point of its own, and a reflected ray crosses the medium in the
    /// opposite direction, which is exactly where a march that assumed a direction would break.
    #[test]
    fn small_signal_ghost_reflections_are_amplified_on_every_pass() -> OpmResult<()> {
        let r = GHOST_REFLECTIVITY;
        let t = 1.0 - r;
        let g = single_pass_gain();
        let expected = [t * g * t, r + t * g * r * g * t, t * g * r * g * r * g * t];
        let measured = ghost_energies_per_bounce(
            GainModel::MonochromaticSmallSignalGain(MonochromaticSmallSignalGain::new(
                head_gain_coefficient(),
            )?),
            PumpSource::Const,
            2,
        )?;
        assert_eq!(measured.len(), expected.len());
        for (bounce, (measured, expected)) in measured.iter().zip(expected.iter()).enumerate() {
            assert_relative_eq!(measured, expected, epsilon = 1e-6);
            assert!(*measured > 0.0, "bounce {bounce} carries no energy at all");
        }
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
    /// Phase A: every analyzer installs a prepared medium before the first ray is traced.
    ///
    /// The medium is installed on the node's `NodeAttr` slot rather than passed per call, so an
    /// amplifier that did not get its medium prepared before the first ray would find nothing to
    /// read from. This test pins down that all three analyzers honour the contract: ray trace,
    /// energy flow, and ghost focus each call `prepare_volume` on every volume node they traverse.
    #[test]
    fn every_analyzer_prepares_the_media_it_traverses() -> OpmResult<()> {
        let make_model = || -> OpmResult<(NodeGroup, Uuid, Uuid)> {
            let mut model = NodeGroup::default();
            let source = model.add_node(SourcePort::default())?;
            let head = model.add_node(amplifier_head()?)?;
            let screen = model.add_node(SpotDiagram::default())?;
            model.connect_nodes(source, "output_1", head, "input_1", millimeter!(30.0))?;
            model.connect_nodes(head, "output_1", screen, "input_1", millimeter!(30.0))?;
            Ok((model, source, head))
        };
        let medium_is_prepared = |model: &NodeGroup, head: Uuid| -> OpmResult<bool> {
            let head_ref = model.graph().node(head)?;
            let head_node = head_ref.optical_ref.lock_opm()?;
            Ok(head_node.node_attr().runtime_medium().is_some())
        };

        // Ray trace
        let (mut model, source, head) = make_model()?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        config.set_active_pump_scenario(Some(scenario_with_small_signal(head)?));
        RayTracingAnalyzer::new(config).analyze(&mut model)?;
        assert!(
            medium_is_prepared(&model, head)?,
            "ray trace did not prepare the medium"
        );

        // Energy flow — uses ConstGain because the energy flow analysis has no path length and
        // refuses path-dependent models; the important thing is that prepare_volume is called.
        let (mut model, source, head) = make_model()?;
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        config.set_active_pump_scenario(Some(scenario_with_gain(head, 2.0)?));
        EnergyAnalyzer::new(config).analyze(&mut model)?;
        assert!(
            medium_is_prepared(&model, head)?,
            "energy flow did not prepare the medium"
        );

        // Ghost focus
        let (mut model, source, head) = make_model()?;
        let mut config = GhostFocusConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        config.set_active_pump_scenario(Some(scenario_with_small_signal(head)?));
        GhostFocusAnalyzer::new(config).analyze(&mut model)?;
        assert!(
            medium_is_prepared(&model, head)?,
            "ghost focus did not prepare the medium"
        );

        Ok(())
    }
}
