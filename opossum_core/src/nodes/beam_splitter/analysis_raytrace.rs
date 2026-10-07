use nalgebra::{Rotation3, Vector3};

use crate::{
    analyzers::{AnalyzerType, RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult, Rays},
    meter,
    utils::geom_transformation::Isometry,
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
    /// The axis rays arriving at an input are split on the splitting surface. The first input is used
    /// when it carries axis data, otherwise the second input. For the first input the transmitted rays
    /// position the nodes behind the first output and the reflected rays those behind the second; for
    /// the second input the roles are swapped, since its transmission exits the second output and its
    /// reflection the first. All output ports are swapped, if the beam splitter is inverted.
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
    /// splitting surface cannot be found.
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_1, input_2, out_1, out_2) = if self.inverted() {
            (
                "out1_trans1_refl2",
                "out2_trans2_refl1",
                "input_1",
                "input_2",
            )
        } else {
            (
                "input_1",
                "input_2",
                "out1_trans1_refl2",
                "out2_trans2_refl1",
            )
        };
        // Transmission keeps the beam direction, reflection exits on the other output. So the first
        // input feeds (out_1, out_2) with its (transmitted, reflected) rays; the second input, whose
        // transmission continues into out_2, feeds them in swapped order.
        let light_result = if let Some(input) = incoming_data.get(input_1) {
            let (transmitted, reflected) = self.split_axis_rays(input, input_1, config)?;
            LightResult::from([
                (out_1.into(), LightData::Geometric(transmitted)),
                (out_2.into(), LightData::Geometric(reflected)),
            ])
        } else if let Some(input) = incoming_data.get(input_2) {
            let (transmitted, reflected) = self.split_axis_rays(input, input_2, config)?;
            LightResult::from([
                (out_2.into(), LightData::Geometric(transmitted)),
                (out_1.into(), LightData::Geometric(reflected)),
            ])
        } else {
            return Err(OpossumError::Analysis(
                "could not calc optical axis for beam splitter".into(),
            ));
        };
        Ok(light_result)
    }
    /// Returns the local frame from which the optical axis enters the given input port.
    ///
    /// The first input keeps the default (identity) frame: its axis enters along local +z through
    /// the local origin. The second input arrives on the same splitting plane from the direction of
    /// the beam that leaves the second output, i.e. mirrored on that plane, so its frame is the
    /// mirror image of the identity frame. See `doc/book` (geometry: beam combiners) for the
    /// convention. The second input is `input_2`, or `out2_trans2_refl1` when inverted.
    ///
    /// # Arguments
    ///
    /// * `port_name` - the input port the axis enters.
    ///
    /// # Returns
    ///
    /// The identity frame for the first input, the mirrored frame for the second.
    fn axis_entrance_frame(&self, port_name: &str) -> Isometry {
        let second_input = if self.inverted() {
            "out2_trans2_refl1"
        } else {
            "input_2"
        };
        if port_name != second_input {
            return Isometry::identity();
        }
        // Splitting plane in node-local coordinates: unit normal `n` = alignment applied to +z,
        // through the alignment's translation `t`. The second input's axis is the +z axis mirrored
        // on that plane: entry point `q` (origin mirrored) and direction `d` (+z mirrored).
        let alignment = (*self.node_attr().alignment()).unwrap_or_else(Isometry::identity);
        let n = alignment.transform_vector_f64(&Vector3::z());
        let t = alignment.translation();
        let t_vec = Vector3::new(t.x.value, t.y.value, t.z.value);
        let q = 2.0 * n.dot(&t_vec) * n;
        let d = Vector3::z() - 2.0 * Vector3::z().dot(&n) * n;
        let up = Rotation3::rotation_between(&Vector3::z(), &d)
            .map_or_else(Vector3::y, |rot| rot * Vector3::y());
        Isometry::new_from_view(meter!(q.x, q.y, q.z), d, up)
    }
}

impl BeamSplitter {
    /// Splits the axis rays arriving at `port_name` on the splitting surface for a positioning run.
    ///
    /// Positioning is purely geometric: a fixed 50:50 split leaves energy in both branches, so
    /// neither is invalidated by an energy threshold downstream, and no alignment wavelength has to
    /// lie inside a splitting spectrum.
    ///
    /// # Arguments
    ///
    /// * `input` - the axis rays arriving at the port.
    /// * `port_name` - the input port (and its splitting surface).
    /// * `config` - the ray tracing configuration (for the missed-surface strategy).
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected axis rays, in that order.
    ///
    /// # Errors
    ///
    /// This function returns an error if `input` is not [`LightData::Geometric`], the splitting
    /// surface cannot be found, or the split fails.
    fn split_axis_rays(
        &mut self,
        input: &LightData,
        port_name: &str,
        config: &RayTraceConfig,
    ) -> OpmResult<(Rays, Rays)> {
        let LightData::Geometric(rays) = input else {
            return Err(OpossumError::Analysis(format!(
                "expected Rays value at `{port_name}` port"
            )));
        };
        let mut transmitted = rays.clone();
        let Some(surf) = self.get_optic_surface_mut(port_name) else {
            return Err(OpossumError::OpticPort(
                "input optic surface not found".into(),
            ));
        };
        let (reflected, _) = transmitted.split_on_surface(
            surf,
            &SplittingConfig::Ratio(0.5),
            config.missed_surface_strategy(),
        )?;
        Ok((transmitted, reflected))
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
            BeamSplitter, Dummy, NodeGroup, NodeReference, SourcePort, SplittingConfigBuilder,
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
    /// The world position (in meters) and facing direction (world image of local +z) of a placed node.
    fn placed(scenery: &NodeGroup, uuid: uuid::Uuid) -> (Vector3<f64>, Vector3<f64>) {
        let iso = scenery
            .node(uuid)
            .unwrap()
            .effective_position()
            .cloned()
            .expect("node was not placed");
        let t = iso.translation();
        (
            Vector3::new(t.x.value, t.y.value, t.z.value),
            iso.transform_vector_f64(&Vector3::z()),
        )
    }
    /// A world position given in millimeters, expressed as raw meters for comparison.
    fn mm(x: f64, y: f64, z: f64) -> Vector3<f64> {
        Vector3::new(
            millimeter!(x).value,
            millimeter!(y).value,
            millimeter!(z).value,
        )
    }
    /// Pins today's group-level positioning through a beam splitter's first input.
    ///
    /// A source emits along +z into a 45°-tilted beam splitter 100 mm away; a dummy sits 50 mm
    /// behind each output. This captures the positions and facing directions that placement
    /// produces via `input_1`, so a later change to the placement or the axis split shows up here.
    #[test]
    fn pin_group_positioning_via_input_1() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let mut bs = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?;
        bs.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        let bs = scenery.add_node(bs)?;
        let transmitted = scenery.add_node(Dummy::new("transmitted"))?;
        let reflected = scenery.add_node(Dummy::new("reflected"))?;
        scenery.connect_nodes(src, "output_1", bs, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(
            bs,
            "out1_trans1_refl2",
            transmitted,
            "input_1",
            millimeter!(50.0),
        )?;
        scenery.connect_nodes(
            bs,
            "out2_trans2_refl1",
            reflected,
            "input_1",
            millimeter!(50.0),
        )?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;

        let (bs_pos, bs_dir) = placed(&scenery, bs);
        assert_abs_diff_eq!(bs_pos, mm(0., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(bs_dir, Vector3::z(), epsilon = 1e-9);

        let (trans_pos, trans_dir) = placed(&scenery, transmitted);
        assert_abs_diff_eq!(trans_pos, mm(0., 0., 150.), epsilon = 1e-9);
        assert_abs_diff_eq!(trans_dir, Vector3::z(), epsilon = 1e-9);

        let (refl_pos, refl_dir) = placed(&scenery, reflected);
        assert_abs_diff_eq!(refl_pos, mm(-50., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(refl_dir, -Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
    /// A group positioning run through a beam splitter connected only on its second input.
    ///
    /// Regression for #1233: connecting only `input_2` used to fail the positioning run. The second
    /// input's transmission exits the second output and its reflection the first, so a 45°-tilted
    /// splitter sends the axis along +z out of `out2_trans2_refl1` and along +x out of
    /// `out1_trans1_refl2`, both from the splitter placed on the source axis at 100 mm.
    #[test]
    fn group_positioning_via_input_2_tilted() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let mut bs = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?;
        bs.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        let bs = scenery.add_node(bs)?;
        let out1 = scenery.add_node(Dummy::new("out1"))?;
        let out2 = scenery.add_node(Dummy::new("out2"))?;
        scenery.connect_nodes(src, "output_1", bs, "input_2", millimeter!(100.0))?;
        scenery.connect_nodes(bs, "out1_trans1_refl2", out1, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(bs, "out2_trans2_refl1", out2, "input_1", millimeter!(50.0))?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;

        let (bs_pos, _) = placed(&scenery, bs);
        assert_abs_diff_eq!(bs_pos, mm(0., 0., 100.), epsilon = 1e-9);

        // Transmission of input_2 leaves out2 along +z; reflection leaves out1 along +x.
        let (out2_pos, out2_dir) = placed(&scenery, out2);
        assert_abs_diff_eq!(out2_pos, mm(0., 0., 150.), epsilon = 1e-9);
        assert_abs_diff_eq!(out2_dir, Vector3::z(), epsilon = 1e-9);

        let (out1_pos, out1_dir) = placed(&scenery, out1);
        assert_abs_diff_eq!(out1_pos, mm(50., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(out1_dir, Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
    /// A group positioning run through an untilted beam splitter connected only on its second input.
    ///
    /// With no tilt, the second input's reflection travels straight back, so `out1_trans1_refl2` is
    /// placed on the source axis at 50 mm, facing −z, while the transmission continues to
    /// `out2_trans2_refl1` at 150 mm along +z.
    #[test]
    fn group_positioning_via_input_2_untilted() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let bs = scenery.add_node(BeamSplitter::new(
            "bs",
            &SplittingConfigBuilder::FixedRatio(0.5),
        )?)?;
        let out1 = scenery.add_node(Dummy::new("out1"))?;
        let out2 = scenery.add_node(Dummy::new("out2"))?;
        scenery.connect_nodes(src, "output_1", bs, "input_2", millimeter!(100.0))?;
        scenery.connect_nodes(bs, "out1_trans1_refl2", out1, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(bs, "out2_trans2_refl1", out2, "input_1", millimeter!(50.0))?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;

        let (out2_pos, out2_dir) = placed(&scenery, out2);
        assert_abs_diff_eq!(out2_pos, mm(0., 0., 150.), epsilon = 1e-9);
        assert_abs_diff_eq!(out2_dir, Vector3::z(), epsilon = 1e-9);

        let (out1_pos, out1_dir) = placed(&scenery, out1);
        assert_abs_diff_eq!(out1_pos, mm(0., 0., 50.), epsilon = 1e-9);
        assert_abs_diff_eq!(out1_dir, -Vector3::z(), epsilon = 1e-9);
        Ok(())
    }
    /// The world image of local +z under an entrance frame, for asserting its direction.
    fn entrance_dir(node: &BeamSplitter, port: &str) -> Vector3<f64> {
        node.axis_entrance_frame(port)
            .transform_vector_f64(&Vector3::z())
    }
    #[test]
    fn axis_entrance_frame_first_input_is_identity() -> OpmResult<()> {
        let node = tilted_beam_splitter()?;
        assert_eq!(node.axis_entrance_frame("input_1"), Isometry::identity());
        Ok(())
    }
    #[test]
    fn axis_entrance_frame_second_input_tilted() -> OpmResult<()> {
        let node = tilted_beam_splitter()?;
        let frame = node.axis_entrance_frame("input_2");
        // 45° about y mirrors +z onto −x; the plane runs through the origin, so no decenter.
        assert_abs_diff_eq!(
            frame.transform_vector_f64(&Vector3::z()),
            -Vector3::x(),
            epsilon = 1e-12
        );
        let t = frame.translation();
        assert_abs_diff_eq!(
            Vector3::new(t.x.value, t.y.value, t.z.value),
            Vector3::zeros(),
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn axis_entrance_frame_second_input_untilted() -> OpmResult<()> {
        let node = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?;
        // No tilt mirrors +z straight back onto −z.
        assert_abs_diff_eq!(
            entrance_dir(&node, "input_2"),
            -Vector3::z(),
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn axis_entrance_frame_second_input_decentered() -> OpmResult<()> {
        let mut node = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?;
        node.set_alignment(millimeter!(0., 0., 10.), degree!(0., 0., 0.))?;
        let frame = node.axis_entrance_frame("input_2");
        // The plane sits 10 mm along +z; the origin mirrored on it lands at 20 mm, direction −z.
        assert_abs_diff_eq!(
            entrance_dir(&node, "input_2"),
            -Vector3::z(),
            epsilon = 1e-12
        );
        let t = frame.translation();
        assert_abs_diff_eq!(
            Vector3::new(t.x.value, t.y.value, t.z.value),
            mm(0., 0., 20.),
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn axis_entrance_frame_inverted_uses_second_output_port() -> OpmResult<()> {
        let mut node = tilted_beam_splitter()?;
        node.set_inverted(true)?;
        // When inverted, the second input is out2_trans2_refl1; the other ports keep the identity.
        assert_abs_diff_eq!(
            entrance_dir(&node, "out2_trans2_refl1"),
            -Vector3::x(),
            epsilon = 1e-12
        );
        assert_eq!(
            node.axis_entrance_frame("out1_trans1_refl2"),
            Isometry::identity()
        );
        Ok(())
    }
    /// Two sources feeding an untilted combiner, with the second placed at the pose the axis
    /// requires, position the combiner without any warning.
    #[test]
    fn two_input_combiner_consistent_gives_no_warning() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src1 = scenery.add_node(SourcePort::new("s1"))?;
        let mut s2 = SourcePort::new("s2");
        // The consistent pose for input_2 of an untilted combiner 100 mm behind each source: on the
        // axis at 200 mm, facing back along −z (turned 180° about y).
        s2.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(0., 0., 200.),
            degree!(0., 180., 0.),
        )?))?;
        let src2 = scenery.add_node(s2)?;
        let bs = scenery.add_node(BeamSplitter::new(
            "bs",
            &SplittingConfigBuilder::FixedRatio(0.5),
        )?)?;
        scenery.connect_nodes(src1, "output_1", bs, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(src2, "output_1", bs, "input_2", millimeter!(100.0))?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src1,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        config.map_source(
            src2,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );

        testing_logger::setup();
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        testing_logger::validate(|logs| {
            let warnings: Vec<&str> = logs
                .iter()
                .filter(|l| l.level == log::Level::Warn)
                .map(|l| l.body.as_str())
                .collect();
            assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        });
        let (bs_pos, _) = placed(&scenery, bs);
        assert_abs_diff_eq!(bs_pos, mm(0., 0., 100.), epsilon = 1e-9);
        Ok(())
    }
    /// Two sources both at the origin feeding a combiner are inconsistent: the combiner is placed
    /// from `input_1`, and the warning names `input_2` and the pose its source would need.
    #[test]
    fn two_sources_at_origin_warn_with_suggested_start() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src1 = scenery.add_node(SourcePort::new("s1"))?;
        let src2 = scenery.add_node(SourcePort::new("s2"))?;
        let bs = scenery.add_node(BeamSplitter::new(
            "bs",
            &SplittingConfigBuilder::FixedRatio(0.5),
        )?)?;
        scenery.connect_nodes(src1, "output_1", bs, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(src2, "output_1", bs, "input_2", millimeter!(100.0))?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src1,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        config.map_source(
            src2,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );

        testing_logger::setup();
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        testing_logger::validate(|logs| {
            let warnings = logs
                .iter()
                .filter(|l| l.level == log::Level::Warn)
                .map(|l| l.body.clone())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(warnings.contains("input_2"), "warnings: {warnings}");
            assert!(warnings.contains("180.0"), "warnings: {warnings}");
            assert!(warnings.contains("must start at"), "warnings: {warnings}");
            assert!(warnings.contains("200.000"), "warnings: {warnings}");
        });
        // The combiner is placed from input_1 (source on the axis at 100 mm), not from input_2.
        let (bs_pos, _) = placed(&scenery, bs);
        assert_abs_diff_eq!(bs_pos, mm(0., 0., 100.), epsilon = 1e-9);
        Ok(())
    }
    /// A forward reference to a tilted beam splitter, fed via its second input, is positioned using
    /// the target's mirrored entrance frame.
    ///
    /// The reference is placed before its target (a forward reference), so its placement runs through
    /// `set_node_isometry`, which must ask the referenced beam splitter for the `input_2` entrance
    /// frame. If that delegation were missing, the reference would be placed as for an ordinary first
    /// input (facing +z) instead of the mirrored +x, so the facing direction distinguishes the two.
    #[test]
    fn forward_reference_to_beam_splitter_via_input_2_uses_mirrored_frame() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let mut bs = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?;
        bs.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        let bs = scenery.add_node(bs)?;
        // The reference is added after the (still unplaced) beam splitter and sits before it in the
        // beam path, so it is a forward reference: it is positioned first and then places the target.
        let bs_ref = NodeReference::from_node(&scenery.node(bs)?)?;
        let bs_ref = scenery.add_node(bs_ref)?;
        let dummy = scenery.add_node(Dummy::new("out"))?;
        scenery.connect_nodes(src, "output_1", bs_ref, "input_2", millimeter!(100.0))?;
        scenery.connect_nodes(
            bs_ref,
            "out2_trans2_refl1",
            bs,
            "input_1",
            millimeter!(50.0),
        )?;
        scenery.connect_nodes(bs, "out1_trans1_refl2", dummy, "input_1", millimeter!(50.0))?;

        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;

        // The reference sits on the source axis at 100 mm and faces +x, the mirrored-frame signature
        // of a beam splitter fed via input_2 (an ordinary first input would leave it facing +z).
        let (ref_pos, ref_dir) = placed(&scenery, bs_ref);
        assert_abs_diff_eq!(ref_pos, mm(0., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(ref_dir, Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
}
