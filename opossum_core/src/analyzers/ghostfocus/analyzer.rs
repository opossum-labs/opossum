use log::info;
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

use super::{GhostFocusConfig, GhostFocusHistory};
use crate::{
    analyzers::{Analyzer, raytrace::AnalysisRayTrace},
    error::OpmResult,
    light_result::{
        LightRays, LightResult, light_rays_to_light_result, light_result_to_light_rays,
    },
    nodes::NodeGroup,
    prelude::{OpticNode, Properties, Proptype, RayTraceConfig},
    properties::proptype::{count_str, format_value_with_prefix},
    rays::Rays,
    reporting::{analysis_report::AnalysisReport, node_report::NodeReport},
    utils::LockExt,
};

/// Analyzer for ghost focus simulation
#[derive(Default, Debug)]
pub struct GhostFocusAnalyzer {
    config: GhostFocusConfig,
}
impl GhostFocusAnalyzer {
    /// Creates a new [`GhostFocusAnalyzer`].
    #[must_use]
    pub const fn new(config: GhostFocusConfig) -> Self {
        Self { config }
    }
    /// Returns a reference to the config of this [`GhostFocusAnalyzer`].
    #[must_use]
    pub const fn config(&self) -> &GhostFocusConfig {
        &self.config
    }

    fn node_group_report(
        &self,
        group: &NodeGroup,
        analysis_report: &mut AnalysisReport,
    ) -> OpmResult<()> {
        for node_ref in group.graph().nodes() {
            let node = node_ref.optical_ref.lock_opm()?;
            if let Ok(g) = node.as_group() {
                self.node_group_report(g, analysis_report)?;
            } else {
                let node_name = &node.name();
                let hit_maps = node.hit_maps();
                drop(node);
                for hit_map in &hit_maps {
                    let critical_positions = hit_map.1.critical_fluences();
                    let node = node_ref.optical_ref.lock_opm()?;
                    let lidt = *node
                        .get_optic_surface(hit_map.0)
                        .expect("OpticSurface not found!")
                        .lidt();
                    drop(node);
                    if !critical_positions.is_empty() {
                        for (i, (rays_uuid, (fluence, hist_idx, bounce))) in
                            critical_positions.iter().enumerate()
                        {
                            let critical_ghost_hist = GhostFocusHistory::from((
                                group.accumulated_rays(),
                                *rays_uuid,
                                *hist_idx,
                            ));
                            let origin_str =
                                critical_ghost_hist.rays_origin_report_str(group.graph());
                            let mut hit_map_props = Properties::default();
                            hit_map_props.create(
                                "Origin",
                                "Surface bounces that enabled this fluence",
                                origin_str.clone().into(),
                            )?;
                            let fluence_data = hit_map
                                .1
                                .get_rays_hit_map(*bounce, *rays_uuid)
                                .unwrap()
                                .calc_fluence_map(
                                    (101, 101),
                                    self.config().fluence_estimator(),
                                    None,
                                    None,
                                )?;

                            hit_map_props.create(
                                &format!("Peak fluence ({})", fluence_data.estimator()),
                                "Peak fluence on this surface using Voronoi estimator",
                                format!(
                                    "{}J/cm², (LIDT of surface: {}J/cm²)",
                                    format_value_with_prefix(
                                        fluence.get::<joule_per_square_centimeter>()
                                    ),
                                    format_value_with_prefix(
                                        lidt.get::<joule_per_square_centimeter>()
                                    )
                                )
                                .into(),
                            )?;
                            hit_map_props.create(
                                "Ray propagation",
                                "ray propagation",
                                Proptype::from(critical_ghost_hist),
                            )?;
                            hit_map_props.create(
                                "Fluence",
                                "2D spatial energy distribution",
                                fluence_data.into(),
                            )?;
                            let hit_map_report = NodeReport::new(
                                "surface",
                                &format!(
                                    "{} critical fluence on surface '{}' of node '{}'",
                                    count_str(i + 1),
                                    hit_map.0,
                                    node_name
                                ),
                                &Uuid::new_v4().as_simple().to_string(),
                                hit_map_props,
                            );
                            analysis_report.add_node_report(hit_map_report);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
impl Analyzer for GhostFocusAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Calculate node positions of scenery{scenery_name}.");

        // copy source map to RayTraceConfig to be able to use it in the unified analyze function of AnalysisRayTrace
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_source_map(self.config.source_map().clone());
        AnalysisRayTrace::calc_node_positions(scenery, LightResult::default(), &raytrace_config)?;
        info!(
            "Performing ghost focus analysis of scenery{scenery_name} up to {} ray bounces.",
            self.config.max_bounces()
        );
        scenery.clear_edges();
        for bounce in 0..=self.config.max_bounces() {
            let mut ray_collection = Vec::<Rays>::new();
            if bounce % 2 == 0 {
                scenery.set_inverted(false)?;
                info!("Analyzing pass {bounce} (forward) ...");
            } else {
                scenery.set_inverted(true)?;
                info!("Analyzing pass {bounce} (backward) ...");
            }
            AnalysisGhostFocus::analyze(
                scenery,
                LightRays::default(),
                self.config(),
                &mut ray_collection,
                bounce,
            )?;
            scenery.set_inverted(false)?;
            scenery.clear_edges();
            for rays in &ray_collection {
                scenery.add_to_accumulated_rays(rays, bounce);
            }
        }
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::default();
        analysis_report.add_scenery(scenery);
        let mut props = Properties::default();
        let ghost_focus_history = GhostFocusHistory::from(scenery.accumulated_rays().clone());

        let proptype = Proptype::from(ghost_focus_history);
        props.create("propagation", "ray propagation", proptype)?;

        let mut node_report =
            NodeReport::new("ray propagation", "Global ray propagation", "global", props);
        node_report.set_show_item(true);
        analysis_report.add_node_report(node_report);

        self.node_group_report(scenery, &mut analysis_report)?;
        analysis_report.set_analysis_type("Ghost Focus Analysis");
        Ok(analysis_report)
    }
}

/// Trait for implementing ghost focus analysis.
///
/// This trait extends the [`AnalysisRayTrace`] trait and provides a default implementation
/// of the `analyze` function that performs a ghost focus analysis of an [`OpticNode`]. The
/// `analyze` function takes into account possible reflected [`Rays`] and returns the resulting [`LightRays`].
pub trait AnalysisGhostFocus: OpticNode + AnalysisRayTrace {
    /// Perform a ghost focus analysis of an [`OpticNode`].
    ///
    /// This function is similar to the corresponding [`AnalysisRayTrace`] function but also
    /// considers possible reflected [`Rays`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the analysis fails for any reason, such as if
    /// the input data is invalid or if the node cannot be analyzed.
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let incoming_result = light_rays_to_light_result(incoming_data);
        let out_result =
            self.unified_analyze_single_surface_node(incoming_result, config, "input_1", None)?;
        light_result_to_light_rays(out_result)
    }
}
#[cfg(test)]
mod test_ghost_focus_analyzer {
    use super::*;
    use crate::{
        analyzers::Analyzer,
        coatings::CoatingType,
        degree, joule,
        light_result::LightResult,
        millimeter,
        nodes::{
            Lens, NodeGroup, SourcePort, SpotDiagram, ThinMirror, round_collimated_ray_builder,
        },
        optic_node::{Alignable, OpticNode},
        optic_ports::PortType,
    };
    #[test]
    fn empty_report() {
        let analyzer = GhostFocusAnalyzer::default();
        let scenery = NodeGroup::new("");
        analyzer.report(&scenery).unwrap();
    }
    #[test]
    #[ignore]
    fn report() {
        let mut scenery = NodeGroup::default();
        let i_src = scenery.add_node(SourcePort::default()).unwrap();
        let mut lens = Lens::default();
        lens.set_coating(
            &PortType::Input,
            "input_1",
            &CoatingType::ConstantR { reflectivity: 0.2 },
        )
        .unwrap();
        lens.set_coating(
            &PortType::Output,
            "output_1",
            &CoatingType::ConstantR { reflectivity: 0.2 },
        )
        .unwrap();
        let i_l = scenery.add_node(lens).unwrap();
        let mir1 = scenery
            .add_node(
                ThinMirror::new("mir 1")
                    .with_tilt(degree!(45., 0., 0.))
                    .unwrap(),
            )
            .unwrap();
        scenery
            .connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(120.0))
            .unwrap();
        scenery
            .connect_nodes(i_l, "output_1", mir1, "input_1", millimeter!(60.0))
            .unwrap();

        let mut config = GhostFocusConfig::default();
        config.set_max_bounces(2);
        config.map_source(
            i_src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(2.), 5).unwrap(),
        );
        let analyzer = GhostFocusAnalyzer::new(config);
        analyzer.analyze(&mut scenery).unwrap();
        analyzer.report(&scenery).unwrap();
    }

    #[test]
    fn analyze_single_surface_node() {
        let mut sd = SpotDiagram::default();
        let config = GhostFocusConfig::default();
        let out_result = sd
            .unified_analyze_single_surface_node(LightResult::default(), &config, "input_1", None)
            .unwrap();
        let output_data = out_result.get("output_1");

        match output_data {
            Some(crate::lightdata::LightData::GhostFocus(rays)) => assert_eq!(rays.len(), 0),
            Some(crate::lightdata::LightData::Geometric(rays)) => {
                assert_eq!(rays.nr_of_rays(false), 0)
            }
            None => assert!(out_result.is_empty()),
            _ => panic!("Unerwarteter Datentyp auf dem Output Port"),
        }
    }
}
#[cfg(test)]
mod test_ghost_analysis_nested_groups_inversion {
    use crate::{
        analyzers::ghostfocus::config::GhostFocusConfig,
        coatings::CoatingType,
        energy_distributions::General2DGaussian,
        joule, millimeter, nanometer,
        nodes::{Lens, NodeGroup, SourcePort},
        position_distributions::Hexapolar,
        prelude::{
            AnalyzerType, CollimatedSrc, OpmDocument, OpticNode, PortType, RayDataSource,
            RefrIndexConst,
        },
        radian,
        spectral_distribution::LaserLines,
        utils::LockExt,
    };
    use uuid::Uuid;

    fn create_doc(bounces: usize) -> OpmDocument {
        let mut scenery = NodeGroup::new("Ghost focus nested group test");
        scenery.set_expand_view(true).unwrap();

        let inf = millimeter!(f64::INFINITY);

        let mut lens_01 = Lens::new(
            "Lens 0_1",
            inf,
            inf,
            millimeter!(1.),
            RefrIndexConst::new(1.4).unwrap(),
        )
        .unwrap();
        lens_01
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        lens_01
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();

        let mut lens_02 = Lens::new(
            "Lens 0_2",
            inf,
            inf,
            millimeter!(1.),
            RefrIndexConst::new(1.4).unwrap(),
        )
        .unwrap();
        lens_02
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        lens_02
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();

        let src = scenery
            .add_node(SourcePort::new("Collimated Source"))
            .unwrap();
        let l0_1 = scenery.add_node(lens_01).unwrap();
        let l0_2 = scenery.add_node(lens_02).unwrap();

        let mut lens_1 = Lens::new(
            "Lens 1",
            inf,
            inf,
            millimeter!(1.),
            RefrIndexConst::new(1.4).unwrap(),
        )
        .unwrap();
        lens_1
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        lens_1
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();

        let mut group_1 = NodeGroup::new("Group 1");
        group_1.set_expand_view(true).unwrap();
        let l1 = group_1.add_node(lens_1).unwrap();

        let mut lens_2 = Lens::new(
            "Lens 2",
            inf,
            inf,
            millimeter!(1.),
            RefrIndexConst::new(1.4).unwrap(),
        )
        .unwrap();
        lens_2
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        lens_2
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        let mut group_2 = NodeGroup::new("Group 2");
        group_2.set_expand_view(true).unwrap();
        let l2 = group_2.add_node(lens_2).unwrap();

        let mut lens_3 = Lens::new(
            "Lens 3",
            inf,
            inf,
            millimeter!(1.),
            RefrIndexConst::new(1.4).unwrap(),
        )
        .unwrap();
        lens_3
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        lens_3
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 0.01 },
            )
            .unwrap();
        let mut group_3 = NodeGroup::new("Group 3");
        group_3.set_expand_view(true).unwrap();
        let l3 = group_3.add_node(lens_3).unwrap();
        group_3.map_input_port(l3, "input_1", "input_1").unwrap();
        group_3.map_output_port(l3, "output_1", "output_1").unwrap();

        let g3 = group_2.add_node(group_3).unwrap();
        group_2
            .connect_nodes(l2, "output_1", g3, "input_1", millimeter!(10.))
            .unwrap();
        group_2.map_input_port(l2, "input_1", "input_1").unwrap();
        group_2.map_output_port(g3, "output_1", "output_1").unwrap();

        let g2 = group_1.add_node(group_2).unwrap();
        group_1
            .connect_nodes(l1, "output_1", g2, "input_1", millimeter!(10.))
            .unwrap();
        group_1.map_input_port(l1, "input_1", "input_1").unwrap();
        group_1.map_output_port(g2, "output_1", "output_1").unwrap();

        let g1 = scenery.add_node(group_1).unwrap();

        scenery
            .connect_nodes(src, "output_1", l0_1, "input_1", millimeter!(10.))
            .unwrap();
        scenery
            .connect_nodes(l0_1, "output_1", g1, "input_1", millimeter!(10.))
            .unwrap();
        scenery
            .connect_nodes(g1, "output_1", l0_2, "input_1", millimeter!(10.))
            .unwrap();

        //analyzers are added in the tests
        let mut doc: OpmDocument = OpmDocument::new(scenery);
        let config = get_ghost_focus_config_and_map_to_source(src, bounces);
        doc.add_analyzer(AnalyzerType::GhostFocus(config));
        doc
    }

    fn get_ghost_focus_config_and_map_to_source(src_id: Uuid, bounces: usize) -> GhostFocusConfig {
        // collimated source definition
        let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(10.), 0).unwrap().into(),
            General2DGaussian::new(
                joule!(5.0),
                millimeter!(0., 0.),
                millimeter!(2., 2.),
                5.,
                radian!(0.),
                false,
            )
            .unwrap()
            .into(),
            LaserLines::new(vec![(nanometer!(1053.0), 1.0)])
                .unwrap()
                .into(),
        ));
        let mut config = GhostFocusConfig::default();
        config.map_source(src_id, ray_data_source.into());
        config.set_max_bounces(bounces);
        config
    }

    fn check_not_inverted(group: &NodeGroup) -> bool {
        for opt_ref in group.graph().nodes() {
            let node = opt_ref.optical_ref.lock_opm().unwrap();
            if let Ok(g) = node.as_group() {
                if !check_not_inverted(g) {
                    return false;
                }
            }
            if node.inverted() {
                return false;
            }
        }
        true
    }

    #[test]
    fn bounce_0() {
        let bounce = 0;
        let mut document = create_doc(bounce);

        let _ = document.analyze().unwrap();

        let scenery = document.scenery();
        assert!(check_not_inverted(scenery))
    }
    #[test]
    fn bounce_1() {
        let bounce = 1;
        let mut document = create_doc(bounce);

        let _ = document.analyze().unwrap();

        let scenery = document.scenery();
        assert!(check_not_inverted(scenery))
    }
    #[test]
    fn bounce_2() {
        let bounce = 2;
        let mut document = create_doc(bounce);

        let _ = document.analyze().unwrap();

        let scenery = document.scenery();
        assert!(check_not_inverted(scenery))
    }
    #[test]
    fn bounce_3() {
        let bounce = 3;
        let mut document = create_doc(bounce);

        let _ = document.analyze().unwrap();

        let scenery = document.scenery();
        assert!(check_not_inverted(scenery))
    }
    #[test]
    fn bounce_4() {
        let bounce = 4;
        let mut document = create_doc(bounce);

        let _ = document.analyze().unwrap();

        let scenery = document.scenery();
        assert!(check_not_inverted(scenery))
    }
}
