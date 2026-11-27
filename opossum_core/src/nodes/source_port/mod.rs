use std::{fmt::Display, sync::Arc};

use crate::{
    analyzers::propagation_strategy::PropagationStrategy,
    core_optics::{NodeAttr, OpticNodeExt, SurfacePorts, node_attr::NodePositioning},
    error::OpmResult,
    geometry::{Geometry, SurfaceGeometry},
    light::{RayMesh, Rays, lightdata::ray_data_builder::RayDataBuilder},
    nodes::NodeRegistration,
    prelude::{Isometry, OpticNode},
};
use log::warn;
use opm_macros_lib::OpmNode;

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

#[derive(OpmNode, Clone)]
#[opm_node("slateblue")]
/// A source port node marks the position of a light source.
///
/// A source port is a node that marks the logical position of a light source. It is used to define the position
/// and orientation of a light source in the scene. Note, that the source port does not contain any information about the light source itself.
/// During analysis it looks up its data in a source map given by the current analyzer. This allows for a decoupling of the actual light data
/// from its position and orientation in the scene. This way the same source port can be used for different light sources in different analyses,
/// e.g. for ray tracing and energy analysis.
pub struct SourcePort {
    node_attr: NodeAttr,
}

inventory::submit! {
    NodeRegistration::new::<SourcePort>("source port", "light source port")
}

impl Default for SourcePort {
    fn default() -> Self {
        let node_attr = NodeAttr::new("source port");

        let mut src = Self { node_attr };
        src.create_default_surface_properties().unwrap();
        src.set_positioning(NodePositioning::Absolute(Isometry::identity()))
            .unwrap();
        src.update_surfaces().unwrap();
        src
    }
}

impl SourcePort {
    /// Creates a new source port with the specified name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut node = Self::default();
        node.node_attr.set_name(name);
        node
    }

    /// Returns the light an analysis defines for this source, warning if it defines none: the
    /// source then sends out no light in that analysis.
    ///
    /// # Arguments
    ///
    /// * `light` - what the analysis's source map holds for this source.
    fn light_or_warn<'a, L>(&self, light: Option<&'a L>) -> Option<&'a L> {
        if light.is_none() {
            warn!("{self} has no light in this analysis, so it sends out none");
        }
        light
    }
    /// Builds the rays this source sends out into a run: their energy spread over the area each
    /// ray stands for if the run asks so and the source samples its energy (see
    /// [`Rays::spread_energy_over_cells`]), spanning the ray tubes the run needs, in its ambient
    /// medium, placed by the builder and at the output of the source. One mesh serves both, built
    /// before the rays are placed, where the axis of their source is z (see [`RayMesh::new`]).
    ///
    /// # Arguments
    ///
    /// * `builder` - the light the analysis defines for this source.
    /// * `strategy` - the analysis.
    ///
    /// # Errors
    ///
    /// This function returns an error if the rays cannot be built, triangulated or given the
    /// ambient refractive index, or if the source is not placed.
    fn sent_out_rays(
        &self,
        builder: &RayDataBuilder,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<Rays> {
        let mut rays = builder.source().clone().build()?;
        let spread = strategy.spreads_energy_over_cells() && builder.source().samples_its_energy();
        if spread || strategy.needs_ray_tubes() {
            let mesh = Arc::new(RayMesh::new(&rays)?);
            if spread {
                rays.spread_energy_over_cells(&mesh)?;
            }
            if strategy.needs_ray_tubes() {
                rays.attach(&mesh);
            }
        }
        rays.set_refractive_index(&strategy.ambient_refractive_index())?;
        Ok(builder
            .placed(rays)
            .transformed_by_iso(&self.effective_surface_iso("output_1")?))
    }
}

impl Display for SourcePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' (source port)", self.node_attr.name())
    }
}

impl OpticNode for SourcePort {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(None)))
    }
    /// A source port only has an output port.
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts {
            inputs: &[],
            outputs: &[("output_1", SurfaceGeometry::FACE)],
            splitting: &[],
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{Analyzer, raytrace::RayTracingAnalyzer},
        core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
        error::OpossumError,
        joule,
        light::RayMesh,
        millimeter,
    };
    use crate::{
        nodes::{
            NodeGroup, ParaxialSurface, RayPropagationVisualizer, round_collimated_ray_builder,
        },
        prelude::{PortType, RayTraceConfig},
    };
    use spade::Triangulation;

    #[test]
    fn default() {
        let node = SourcePort::default();
        assert_eq!(node.name(), "source port");
        assert_eq!(node.node_type(), "source port");
        assert_eq!(
            node.positioning(),
            &NodePositioning::Absolute(Isometry::identity())
        );
        assert!(!node.node_attr().inverted());
        assert_eq!(node.node_color(), "slateblue");
    }
    #[test]
    fn new() {
        let source = SourcePort::new("test");
        assert_eq!(source.name(), "test");
    }
    #[test]
    fn is_invertable() {
        let mut node = SourcePort::default();
        assert!(node.set_inverted(false).is_ok());
        assert!(node.set_inverted(true).is_ok());
    }
    #[test]
    fn ports() {
        let node = SourcePort::default();
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn integration_test() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let i_src = scenery.add_node(SourcePort::default())?;
        let i_l = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
        let i_sd = scenery.add_node(RayPropagationVisualizer::default())?;
        scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(i_l, "output_1", i_sd, "input_1", millimeter!(150.0))?;

        let ray_data_builder = round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 10)?;
        let mut ray_trace_config = RayTraceConfig::default();
        ray_trace_config.map_source(i_src, ray_data_builder);
        let analyzer = RayTracingAnalyzer::new(ray_trace_config);
        assert!(analyzer.analyze(&mut scenery).is_ok());
        Ok(())
    }
    /// A flat-top beam of 1 J in two hexapolar rings, of two laser lines of 1 and 3 parts.
    fn two_lines() -> OpmResult<RayDataBuilder> {
        use crate::{
            distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
            nanometer,
            prelude::{CollimatedSrc, RayDataSource},
        };
        Ok(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(5.0), 2)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(1064.0), 3.0)])?.into(),
        ))
        .into())
    }
    /// The energies of the given rays at the given wavelength in nm, in J, in the order of the
    /// bundle.
    fn energies_at(rays: &Rays, wavelength: f64) -> Vec<f64> {
        rays.iter()
            .filter(|ray| (ray.wavelength().value * 1e9 - wavelength).abs() < 1e-6)
            .map(|ray| ray.energy().value)
            .collect()
    }
    /// An analysis sends out each ray with the share of its source over the area it stands for,
    /// and each laser line keeps its share: the ray in the middle of a hexapolar pattern stands
    /// for less area than those of its first ring, and carries less.
    #[test]
    fn an_analysis_sends_out_the_energy_over_the_areas_of_the_rays() -> OpmResult<()> {
        let builder = two_lines()?;
        let rays = SourcePort::default()
            .sent_out_rays(&builder, &crate::analyzers::GhostFocusConfig::default())?;
        let areas = RayMesh::new(&builder.build()?)?;
        let sheet = areas
            .sheets()
            .first()
            .ok_or_else(|| OpossumError::Other("no sheet".into()))?;
        let area_of = |index: u32| {
            sheet
                .vertices()
                .find(|v| v.data().rays().contains(&index))
                .map_or(0.0, |v| v.data().cell_area())
        };
        for (wavelength, share) in [(1000.0, 0.25), (1064.0, 0.75)] {
            let energies = energies_at(&rays, wavelength);
            approx::assert_relative_eq!(energies.iter().sum::<f64>(), share, max_relative = 1e-12);
            // the middle ray and a ray of the first ring, in the order the sheet was built
            let (middle, ring) = (energies[0], energies[1]);
            approx::assert_relative_eq!(
                middle / ring,
                area_of(0) / area_of(2),
                max_relative = 1e-9
            );
            assert!(middle < 0.9 * ring, "{middle} J against {ring} J");
        }
        Ok(())
    }
    /// A point source its builder turns spans the tubes of an upright one: they are spanned before
    /// the rays are placed, where the axis of the source is z.
    #[test]
    fn a_turned_point_source_spans_the_tubes_of_an_upright_one() -> OpmResult<()> {
        use crate::{
            degree,
            distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
            nanometer,
            prelude::{PointSrc, RayDataSource},
            utils::geom_transformation::Isometry,
        };
        let upright = RayDataBuilder::from(RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(5.0), 3)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(100.0),
        )?));
        let mut turned = upright.clone();
        turned.set_isometry(Some(Isometry::new_rotation(degree!(10.0, 20.0, 0.0))?));
        let mut config = crate::analyzers::GhostFocusConfig::default();
        config.set_ray_tubes(true);
        let sent_out =
            |builder: &RayDataBuilder| SourcePort::default().sent_out_rays(builder, &config);
        let (upright, turned) = (sent_out(&upright)?, sent_out(&turned)?);
        let first = |rays: &Rays| {
            rays.iter()
                .next()
                .map(|ray| (ray.direction(), ray.source().cloned()))
        };
        let (
            Some((upright_direction, Some(upright_tubes))),
            Some((turned_direction, Some(turned_tubes))),
        ) = (first(&upright), first(&turned))
        else {
            panic!("the rays span no tubes");
        };
        assert!(
            (upright_direction - turned_direction).norm() > 0.1,
            "the builder turns the rays"
        );
        assert_eq!(upright_tubes.mesh(), turned_tubes.mesh());
        Ok(())
    }
    /// Light drawn in the 3D view is sent out as its source builds it: every ray of a flat top
    /// carries the same energy.
    #[test]
    fn light_drawn_is_sent_out_as_built() -> OpmResult<()> {
        let rays =
            SourcePort::default().sent_out_rays(&two_lines()?, &RayTraceConfig::default())?;
        for (wavelength, share) in [(1000.0, 0.25), (1064.0, 0.75)] {
            for energy in energies_at(&rays, wavelength) {
                approx::assert_relative_eq!(energy, share / 19.0, max_relative = 1e-12);
            }
        }
        Ok(())
    }
    /// Raw rays keep the energies they were given, also in an analysis.
    #[test]
    fn raw_rays_keep_their_energies() -> OpmResult<()> {
        use crate::{distributions::position::Hexapolar, nanometer, prelude::RayDataSource};
        let raw = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), 2)?,
        )?;
        let expected = energies_at(&raw, 1000.0);
        let builder: RayDataBuilder = RayDataSource::Raw(raw).into();
        let rays = SourcePort::default()
            .sent_out_rays(&builder, &crate::analyzers::GhostFocusConfig::default())?;
        assert_eq!(energies_at(&rays, 1000.0), expected);
        Ok(())
    }
}
