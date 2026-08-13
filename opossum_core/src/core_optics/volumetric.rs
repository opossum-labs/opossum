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
    geometry::body::{CLEAR_APERTURE, SurfaceBoundedBody},
    light::{LightData, LightRays, LightResult, Rays},
    material::Material,
    properties::Proptype,
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
    /// * `material_inside`: the [`Material`] filling the volume enclosed by the two surfaces.
    ///   Behind the exit surface the node's ambient index is used. The whole material is taken
    ///   rather than just its refractive index because what happens *inside* the volume depends on
    ///   more than refraction — absorption, thermal and stress data, and later the gain. The two
    ///   surface passes themselves only ever need the index and are handed just that.
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
        material_inside: &Material,
        rays_bundle: &mut Vec<Rays>,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
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
        // Inside the medium nothing happens yet. This is where the segmentation of the inner path
        // and the evaluation of an active medium's gain model will be inserted — the reason the
        // whole `material_inside` is available here and not just its refractive index.
        self.pass_through_surface_generic(
            exit_surf_name,
            Some(self.ambient_idx()),
            rays_bundle,
            strategy,
            backward,
            refraction_intended,
        )
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
    /// * `material_inside`: the [`Material`] filling the enclosed volume.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly one input and one output
    /// port, if the incoming data is not geometric ray data, or if the propagation through either
    /// surface fails.
    fn unified_analyze_volume_node(
        &mut self,
        mut incoming_data: LightResult,
        material_inside: &Material,
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
            material_inside,
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
    /// * `material_inside`: the [`Material`] filling the enclosed volume.
    /// * `strategy`: the analyzer-specific [`PropagationStrategy`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the node does not have exactly one input and one output
    /// port, or if the propagation through either surface fails.
    fn unified_analyze_volume_node_ghost_focus(
        &mut self,
        incoming_data: LightRays,
        material_inside: &Material,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<LightRays> {
        let (in_port_name, out_port_name) = single_io_port_names(self)?;
        let mut rays_bundle = incoming_data
            .get(&in_port_name)
            .map_or_else(Vec::<Rays>::new, Clone::clone);
        self.pass_through_volume_generic(
            &in_port_name,
            &out_port_name,
            material_inside,
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
        gain::AMP_CONFIG,
        nodes::{create_node_ref, node_types},
        utils::LockExt,
    };

    /// Only the nodes that really enclose a medium may present themselves as [`Volumetric`].
    ///
    /// "Node with a volume" is stated twice: by this capability and by the properties such a node
    /// carries — the transversal extent of its medium ([`CLEAR_APERTURE`]) and its amplification
    /// model ([`AMP_CONFIG`]). Both have to mean the same set of node types, otherwise a node ends
    /// up with a medium nobody can address or with a body whose extent is undefined.
    #[test]
    fn the_volume_capability_matches_the_volume_properties() -> OpmResult<()> {
        for (node_type, _) in node_types() {
            let optic_ref = create_node_ref(node_type)?;
            let node = optic_ref.optical_ref.lock_opm()?;
            let is_volumetric = node.as_volume().is_some();
            for property_name in [CLEAR_APERTURE, AMP_CONFIG] {
                assert_eq!(
                    node.node_attr().get_property(property_name).is_ok(),
                    is_volumetric,
                    "node type '{node_type}' declares '{property_name}' or presents itself as \
                     volumetric, but not both"
                );
            }
        }
        Ok(())
    }
}
