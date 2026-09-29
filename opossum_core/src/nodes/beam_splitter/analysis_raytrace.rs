use crate::{
    analyzers::{AnalyzerType, RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
};

use super::{BeamSplitter, SplittingConfig};

impl AnalysisRayTrace for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_port1, input_port2) = if self.inverted() {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input_1", "input_2")
        };
        let in1 = incoming_data.get(input_port1);
        let in2 = incoming_data.get(input_port2);
        let (out1_data, out2_data) =
            self.analyze_raytrace(in1, in2, &AnalyzerType::RayTrace(config.clone()))?;
        if let Some(out1_data) = out1_data
            && let Some(out2_data) = out2_data
        {
            let (target1, target2) = if self.inverted() {
                ("input_1", "input_2")
            } else {
                ("out1_trans1_refl2", "out2_trans2_refl1")
            };
            Ok(LightResult::from([
                (target1.into(), out1_data),
                (target2.into(), out2_data),
            ]))
        } else {
            Ok(LightResult::default())
        }
    }
    /// Calculates the outgoing axis rays of this [`BeamSplitter`] for positioning the following nodes.
    ///
    /// The axis rays arriving at the first input are split on the splitting surface. The transmitted
    /// rays position the nodes behind the first output, the reflected rays the nodes behind the second
    /// output (swapped, if the beam splitter is inverted).
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
    /// This function returns an error if there are no geometric axis rays at the first input or
    /// the splitting surface cannot be found.
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_port1, _input_port2) = if self.inverted() {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input_1", "input_2")
        };
        // Positioning is purely geometric. A fixed 50:50 split leaves energy in both branches, so
        // neither is invalidated by an energy threshold downstream, and no alignment wavelength has
        // to lie inside a splitting spectrum.
        let positioning_config = SplittingConfig::Ratio(0.5);
        let in1 = incoming_data.get(input_port1);
        // todo: do this also for in2 and check for position inconsistencies....
        let (transmitted_rays, reflected_rays) = if let Some(input_1) = in1 {
            match input_1 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    let reflected = if let Some(surf) = self.get_optic_surface_mut(input_port1) {
                        rays.split_on_surface(
                            surf,
                            &positioning_config,
                            config.missed_surface_strategy(),
                        )?
                    } else {
                        return Err(OpossumError::OpticPort(
                            "input optic surface not found".into(),
                        ));
                    };
                    (rays, reflected)
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input_1` port".into(),
                    ));
                }
            }
        } else {
            return Err(OpossumError::Analysis(
                "could not calc optical axis for beam splitter".into(),
            ));
        };
        let (target1, target2) = if self.inverted() {
            ("input_1", "input_2")
        } else {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        };
        let light_result = LightResult::from([
            (target1.into(), LightData::Geometric(transmitted_rays)),
            (target2.into(), LightData::Geometric(reflected_rays)),
        ]);
        Ok(light_result)
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;

    use crate::{
        analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
        core_optics::{OpticNode, OpticNodeExt, node_attr::NodePositioning},
        degree,
        error::OpmResult,
        joule,
        light::{LightData, LightResult, Ray, Rays},
        millimeter, nanometer,
        nodes::{
            BeamSplitter, NodeGroup, SourcePort, SplittingConfigBuilder,
            round_collimated_ray_builder,
        },
        prelude::{AnalyzerType, OpmDocument},
        utils::geom_transformation::Isometry,
    };

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_one_input() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., 0.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let result = output.clone().get("out1_trans1_refl2").unwrap().clone();
        let energy = if let LightData::Geometric(r) = result {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.6);
        let result = output.clone().get("out2_trans2_refl1").unwrap().clone();
        let energy = if let LightData::Geometric(r) = result {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.4);
        Ok(())
    }
    #[test]
    fn analyze_two_input() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("input_1".into(), LightData::Geometric(rays));
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(0.5),
        )?);
        input.insert("input_2".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let energy_output1 = if let LightData::Geometric(r) =
            output.clone().get("out1_trans1_refl2").unwrap().clone()
        {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output1, &0.8);
        let energy_output2 = if let LightData::Geometric(r) =
            output.clone().get("out2_trans2_refl1").unwrap().clone()
        {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output2, &0.7);
        Ok(())
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("out1_trans1_refl2".into(), LightData::Geometric(rays));
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(0.5),
        )?);
        input.insert("out2_trans2_refl1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let energy_output1 =
            if let LightData::Geometric(r) = output.clone().get("input_1").unwrap().clone() {
                r.total_energy().get::<uom::si::energy::joule>()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output1, &0.8);
        let energy_output2 =
            if let LightData::Geometric(r) = output.clone().get("input_2").unwrap().clone() {
                r.total_energy().get::<uom::si::energy::joule>()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output2, &0.7);
        Ok(())
    }
    /// A 60:40 beam splitter at the origin, tilted by 45° about the y axis.
    fn tilted_beam_splitter() -> OpmResult<BeamSplitter> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        node.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        Ok(node)
    }
    /// A single collimated ray along the z axis, arriving at the given port.
    fn ray_along_z_at(port: &str) -> OpmResult<LightResult> {
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        Ok(LightResult::from([(
            port.into(),
            LightData::Geometric(rays),
        )]))
    }
    fn output_rays(output: &LightResult, port: &str) -> Rays {
        let Some(LightData::Geometric(rays)) = output.get(port) else {
            panic!("expected rays at port {port}");
        };
        rays.clone()
    }
    /// Asserts that the rays leaving `transmitted_port` still travel along z, while the rays leaving
    /// `reflected_port` travel perpendicular to it, as reflected by a surface tilted by 45°.
    fn assert_separated(output: &LightResult, transmitted_port: &str, reflected_port: &str) {
        let transmitted = output_rays(output, transmitted_port);
        let reflected = output_rays(output, reflected_port);
        let transmitted_dir = transmitted.iter().next().unwrap().direction();
        let reflected_dir = reflected.iter().next().unwrap().direction();
        assert_abs_diff_eq!(transmitted_dir, Vector3::z(), epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_dir.x.abs(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_dir.z, 0.0, epsilon = 1e-12);
    }
    #[test]
    fn analyze_tilted_separates_outputs() -> OpmResult<()> {
        let mut node = tilted_beam_splitter()?;
        let output = AnalysisRayTrace::analyze(
            &mut node,
            ray_along_z_at("input_1")?,
            &RayTraceConfig::default(),
        )?;
        assert_separated(&output, "out1_trans1_refl2", "out2_trans2_refl1");
        let energy = |port| {
            output_rays(&output, port)
                .total_energy()
                .get::<uom::si::energy::joule>()
        };
        assert_abs_diff_eq!(energy("out1_trans1_refl2"), 0.6);
        assert_abs_diff_eq!(energy("out2_trans2_refl1"), 0.4);
        Ok(())
    }
    #[test]
    fn calc_node_positions_tilted_reflects_out2() -> OpmResult<()> {
        let mut node = tilted_beam_splitter()?;
        let output = AnalysisRayTrace::calc_node_positions(
            &mut node,
            ray_along_z_at("input_1")?,
            &RayTraceConfig::default(),
        )?;
        assert_separated(&output, "out1_trans1_refl2", "out2_trans2_refl1");
        Ok(())
    }
    #[test]
    fn calc_node_positions_tilted_inverted() -> OpmResult<()> {
        let mut node = tilted_beam_splitter()?;
        node.set_inverted(true)?;
        let output = AnalysisRayTrace::calc_node_positions(
            &mut node,
            ray_along_z_at("out1_trans1_refl2")?,
            &RayTraceConfig::default(),
        )?;
        assert_separated(&output, "input_1", "input_2");
        Ok(())
    }
    #[test]
    fn integration_not_connected_beam_splitter() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        scenery.add_node(BeamSplitter::default())?; // add unconnected beamsplitter
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));
        document.analyze()?;
        Ok(())
    }
}
