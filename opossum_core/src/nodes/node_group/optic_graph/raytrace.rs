#![warn(missing_docs)]

use super::{InvertGraphGuard, OpticGraph};
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
    radian,
    utils::geom_transformation::Isometry,
};
use log::{info, warn};
use nalgebra::Point3;
use num_traits::Zero;
use petgraph::graph::NodeIndex;
use uom::si::f64::Length;
use uuid::Uuid;

fn filter_ray_limits(light_result: &mut LightResult, r_config: &RayTraceConfig) {
    for lr in light_result {
        if let LightData::Geometric(rays) = lr.1 {
            rays.filter_by_nr_of_bounces(r_config.max_number_of_bounces());
            rays.filter_by_nr_of_refractions(r_config.max_number_of_refractions());
        }
    }
}

impl OpticGraph {
    /// Performs ray tracing analysis through the optical graph.
    ///
    /// # Errors
    ///
    /// This function returns an error if underlying functions fail.
    pub fn analyze_raytrace(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let is_inverted = self.is_inverted();
        let mut guard = InvertGraphGuard::new(self, is_inverted)?;

        if !guard.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }

        let sorted = guard.topologically_sorted()?;
        let mut light_result = incoming_data.clone();

        for idx in sorted {
            let node_id = guard.g[idx].uuid();
            let node_info = format!("{}", guard.g[idx]);

            if guard.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = guard.take_incoming(node_id, incoming_data)?;

                // Evaluate node or reference proxy using shared helper
                let mut outgoing_edges = guard.evaluate_node_or_reference(idx, |node| {
                    AnalysisRayTrace::analyze(&mut **node, incoming_edges, config)
                })?;

                filter_ray_limits(&mut outgoing_edges, config);

                // Map outgoing ports to external group ports
                guard.collect_group_output_ports(node_id, &outgoing_edges, &mut light_result)?;

                for outgoing_edge in outgoing_edges {
                    guard.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                }
            }
        }

        Ok(light_result)
    }

    /// Positions nodes in the graph based on optical ray propagation.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node positions could not be determined (e.g. optical axis misses element
    /// or "gets lost").
    pub fn calc_node_positions(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let sorted = self.topologically_sorted()?;
        let mut light_result = LightResult::default();

        for idx in sorted {
            let node_id = self
                .node_by_idx(idx)
                .map_or_else(|_| Uuid::nil(), |node| node.uuid());
            let has_no_input_connections = !self.has_input_connections(node_id)?;

            let already_placed = match self.g[idx].node_attr().positioning() {
                NodePositioning::Absolute(_) => true,
                NodePositioning::Automatic(cached) => cached.is_some(),
            };

            if has_no_input_connections && !already_placed {
                let node_info = format!("{}", self.g[idx]);
                warn!(
                    "{node_info} has no incoming connections and can thus not being placed. Skipping."
                );
            } else {
                calculate_single_node_position(
                    self,
                    idx,
                    incoming_data,
                    config,
                    &mut light_result,
                )?;
            }
        }

        Ok(light_result)
    }
}

fn position_node(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_id = node_attr.uuid();

    if let Some((align_id, distance)) = node_attr.get_align_like_node_at_distance() {
        let align_ref_iso = graph
            .node(*align_id)?
            .positioning()
            .effective_position()
            .copied();

        if let Some(align_ref_iso) = align_ref_iso {
            let align_iso = Isometry::new(
                Point3::new(Length::zero(), Length::zero(), *distance),
                radian!(0., 0., 0.),
            )?;
            let new_iso = align_ref_iso.append(&align_iso);

            graph.g[node_idx].set_positioning(NodePositioning::Automatic(Some(new_iso)))?;
        } else {
            warn!(
                "Cannot align node like NodeIdx:{}. Fall back to standard positioning method",
                node_idx.index()
            );
            graph.set_node_isometry(incoming_edges, *align_id)?;
        }
    } else {
        graph.set_node_isometry(incoming_edges, node_id)?;
    }
    Ok(())
}

fn ensure_node_isometry(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_info = format!("{}", graph.g[node_idx]);

    match node_attr.positioning() {
        NodePositioning::Absolute(_) => {
            info!("Node {node_info} has an absolute position. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(Some(_)) => {
            info!("Node {node_info} has already been placed. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(None) => {}
    }

    if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
        let target_iso = graph
            .reference_target_mut(target_uuid)?
            .positioning()
            .effective_position()
            .copied();

        if let Some(target_iso) = target_iso {
            info!("Target node for reference {node_info} is already placed. Adopting isometry.");
            graph.g[node_idx].set_positioning(NodePositioning::Automatic(Some(target_iso)))?;
            return Ok(());
        }

        position_node(graph, node_idx, incoming_edges)?;

        let calculated_iso = graph.g[node_idx]
            .positioning()
            .effective_position()
            .copied();

        if let Some(calculated_iso) = calculated_iso {
            let target_node = graph.reference_target_mut(target_uuid)?;
            info!(
                "Forward reference {node_info} positioned target node {}.",
                target_node.name()
            );
            target_node.set_positioning(NodePositioning::Automatic(Some(calculated_iso)))?;
        }
        return Ok(());
    }

    position_node(graph, node_idx, incoming_edges)?;
    Ok(())
}

fn execute_node_calculation(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: LightResult,
    config: &RayTraceConfig,
) -> OpmResult<LightResult> {
    if graph.g[node_idx].referenced_node_id().is_some() {
        graph.evaluate_node_or_reference(node_idx, |node| {
            AnalysisRayTrace::analyze(&mut **node, incoming_edges, config)
        })
    } else {
        let node = &mut graph.g[node_idx];
        AnalysisRayTrace::calc_node_positions(&mut **node, incoming_edges, config)
    }
}

fn calculate_single_node_position(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_data: &LightResult,
    config: &RayTraceConfig,
    light_result: &mut LightResult,
) -> OpmResult<()> {
    let node_id = graph.g[node_idx].uuid();
    let node_info = format!("{}", graph.g[node_idx]);
    let incoming_edges: LightResult = graph.get_incoming(node_id, incoming_data)?;

    ensure_node_isometry(graph, node_idx, &incoming_edges)?;

    let output = execute_node_calculation(graph, node_idx, incoming_edges, config);

    let outgoing_edges = match output {
        Ok(edges) => edges,
        Err(e) => {
            if graph.has_output_connections(node_id)? {
                return Err(OpossumError::Analysis(format!(
                    "calculation of optical axis for node {node_info} failed: {e}"
                )));
            }
            warn!(
                "Calculation of optical axis for terminal node {node_info} failed: {e}. Ignoring as it has no successors."
            );
            LightResult::default()
        }
    };

    // Positioning stops a missed surface (see `RayTraceConfig::for_positioning`), so an invalid axis
    // means it ran outside this node; placing its successors along it would bend it where no optic is.
    if graph.has_output_connections(node_id)? && axis_is_lost(&outgoing_edges) {
        return Err(OpossumError::Analysis(format!(
            "the optical axis misses node {node_info}: it does not pass the node within its clear \
             aperture"
        )));
    }
    graph.collect_group_output_ports(node_id, &outgoing_edges, light_result)?;
    for (port, data) in outgoing_edges {
        graph.set_outgoing_edge_data(node_idx, &port, data);
    }
    Ok(())
}

/// Whether a node lost the optical axis during a positioning run.
///
/// # Arguments
///
/// * `outgoing_edges` - the light the node passes on.
///
/// # Returns
///
/// `true` if an output carries an axis ray that is no longer valid.
fn axis_is_lost(outgoing_edges: &LightResult) -> bool {
    outgoing_edges.values().any(|data| {
        matches!(data, LightData::Geometric(rays) if rays.iter().next().is_some_and(|ray| !ray.valid()))
    })
}

#[cfg(test)]
mod test {
    use crate::{
        analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
        apertures::{CircleShape, OpticShape},
        core_optics::{Alignable, NodeAttrExt, OpticRef, PortType},
        degree,
        error::{OpmResult, OpossumError},
        geometry::body::{OPTIC_SHAPE, default_optic_shape},
        joule,
        light::LightResult,
        millimeter,
        nodes::{
            BeamSplitter, Dummy, IdealFilter, Lens, NodeGroup, ParaxialSurface, SourcePort,
            SpotDiagram, ThinMirror,
            ideal_filter::{FilterConst, FilterTypeBuilder},
            round_collimated_ray_builder,
        },
        percent,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use nalgebra::{Point3, Vector3};
    use num_traits::Zero;
    use uom::si::f64::Length;
    use uuid::Uuid;

    /// How far the lens behind which the screen is placed is shifted off the axis, in millimeter.
    const DECENTRE: f64 = 20.0;
    /// The focal length of the default lens in millimeter: a thick lens with R1 = -R2 = 500 mm,
    /// d = 10 mm and n = 1.5, so 1/f = (n-1) (2/R - (n-1) d / (n R²)).
    fn default_lens_focal_length() -> f64 {
        let (n, radius, thickness) = (1.5, 500.0, 10.0);
        1.0 / ((n - 1.0) * (2.0 / radius - (n - 1.0) * thickness / (n * radius * radius)))
    }
    /// A default lens shifted [`DECENTRE`] up, so the optical axis crosses it that far below its
    /// own axis, with the given optic shape.
    fn decentred_lens(optic_shape: OpticShape) -> OpmResult<Lens> {
        let mut lens = Lens::default().with_decenter(Point3::new(
            Length::zero(),
            millimeter!(DECENTRE),
            Length::zero(),
        ))?;
        lens.set_property(OPTIC_SHAPE, optic_shape.into())?;
        Ok(lens)
    }
    /// Put a source in front of and a screen behind the given row of nodes, 100 mm apart, and
    /// position the row.
    ///
    /// # Returns
    ///
    /// Where the screen was placed.
    ///
    /// # Errors
    ///
    /// Returns an error if positioning fails or does not place the screen.
    fn place_screen_behind(mut scenery: NodeGroup, row: &[Uuid]) -> OpmResult<Isometry> {
        let source = scenery.add_node(SourcePort::default())?;
        let screen = scenery.add_node(Dummy::default())?;
        let mut upstream = source;
        for &node in row.iter().chain(&[screen]) {
            scenery.connect_nodes(upstream, "output_1", node, "input_1", millimeter!(100.0))?;
            upstream = node;
        }
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        scenery
            .node(screen)?
            .effective_position()
            .copied()
            .ok_or_else(|| OpossumError::Other("the screen was not placed".into()))
    }
    /// Assert that the screen was placed along the optical axis as the decentred lens bends it:
    /// towards the lens's own axis, by h / f.
    fn assert_bent_by_the_decentred_lens(screen: &Isometry) {
        let direction = screen.transform_vector_f64(&Vector3::z());
        assert_relative_eq!(
            direction.y / direction.z,
            DECENTRE / default_lens_focal_length(),
            max_relative = 0.01
        );
    }
    /// The optical axis has to pass every component it is connected through: running outside one,
    /// it would be bent where there is no glass. Positioning stops there and names the component.
    #[test]
    fn positioning_stops_when_the_axis_misses_a_component() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let lens = scenery.add_node(decentred_lens(default_optic_shape())?)?;
        let error = place_screen_behind(scenery, &[lens])
            .expect_err("the axis runs outside the lens's optic shape of 12.5 mm");
        assert!(error.to_string().contains("optical axis"), "{error}");
        assert!(error.to_string().contains("lens"), "{error}");
        // With an optic shape of 50 mm the axis passes the lens and is bent by it.
        let mut scenery = NodeGroup::default();
        let large = CircleShape::new(millimeter!(50.0))?.into();
        let lens = scenery.add_node(decentred_lens(large)?)?;
        assert_bent_by_the_decentred_lens(&place_screen_behind(scenery, &[lens])?);
        Ok(())
    }
    /// The axis is placed by geometry alone: a filter blocking it entirely leaves it running on,
    /// and the lens behind still bends it.
    #[test]
    fn positioning_does_not_depend_on_energy() -> OpmResult<()> {
        let blocking = IdealFilter::new(
            "blocking filter",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(0.0))?),
        )?;
        let mut scenery = NodeGroup::default();
        let filter = scenery.add_node(blocking)?;
        let large = CircleShape::new(millimeter!(50.0))?.into();
        let lens = scenery.add_node(decentred_lens(large)?)?;
        assert_bent_by_the_decentred_lens(&place_screen_behind(scenery, &[filter, lens])?);
        Ok(())
    }
    /// A detector does not bend the axis, so the axis running outside its optic shape does not
    /// stop positioning: it only warns that the detector's measurement may fail. A detector the
    /// axis passes within its optic shape gives no such warning.
    #[test]
    fn positioning_warns_when_the_axis_misses_a_detector() -> OpmResult<()> {
        for (decentre, warns) in [(DECENTRE, true), (0.0, false)] {
            let mut detector = SpotDiagram::default().with_decenter(Point3::new(
                Length::zero(),
                millimeter!(decentre),
                Length::zero(),
            ))?;
            detector.set_property(OPTIC_SHAPE, default_optic_shape().into())?;
            let mut scenery = NodeGroup::default();
            let detector = scenery.add_node(detector)?;
            testing_logger::setup();
            let screen = place_screen_behind(scenery, &[detector])?;
            let direction = screen.transform_vector_f64(&Vector3::z());
            assert_relative_eq!(direction.z, 1.0, epsilon = 1e-12);
            testing_logger::validate(|logs| {
                let warned = logs.iter().any(|log| {
                    log.level == log::Level::Warn
                        && log.body.contains("misses the optic shape of detector")
                });
                assert_eq!(warned, warns, "detector decentred by {decentre} mm");
            });
        }
        Ok(())
    }
    /// Add `row` to `group` as a chain, 100 mm apart, each node's first output feeding the next
    /// node's first input.
    ///
    /// # Returns
    ///
    /// The uuids of the first and the last node of the row.
    fn chain(group: &mut NodeGroup, row: &[OpticRef]) -> OpmResult<(Uuid, Uuid)> {
        let mut ids = Vec::new();
        for node in row {
            ids.push(group.add_node_ref(node.clone())?);
        }
        for pair in row.windows(2) {
            let output = pair[0].ports().names(&PortType::Output)[0].clone();
            let input = pair[1].ports().names(&PortType::Input)[0].clone();
            group.connect_nodes(
                pair[0].uuid(),
                &output,
                pair[1].uuid(),
                &input,
                millimeter!(100.0),
            )?;
        }
        Ok((ids[0], ids[ids.len() - 1]))
    }
    /// A group holding `row` as a chain, with the row's first input and last output mapped outward.
    fn grouped(row: &[OpticRef]) -> OpmResult<OpticRef> {
        let mut group = NodeGroup::new("group");
        let (first, last) = chain(&mut group, row)?;
        group.map_input_port(first, "input_1", "input_1")?;
        group.map_output_port(last, "output_1", "output_1")?;
        Ok(OpticRef::new(Box::new(group)))
    }
    /// Put a source 100 mm in front of `row`, position the model and return where each of the
    /// nodes `placed` was put, wherever it sits in the model.
    fn placements(row: &[OpticRef], placed: &[Uuid]) -> OpmResult<Vec<Isometry>> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let (first, _) = chain(&mut scenery, row)?;
        scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(100.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        placed
            .iter()
            .map(|id| {
                scenery
                    .node_recursive(*id)?
                    .0
                    .effective_position()
                    .copied()
                    .ok_or_else(|| OpossumError::Other(format!("node {id} was not placed")))
            })
            .collect()
    }
    /// Assert that both rows of placements put every node at the same place, turned the same way.
    fn assert_same_placements(expected: &[Isometry], actual: &[Isometry]) {
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in expected.iter().zip(actual) {
            assert_relative_eq!(
                expected.get_transform().to_homogeneous(),
                actual.get_transform().to_homogeneous(),
                epsilon = 1e-9
            );
        }
    }
    /// The bridge of `workshop_09_phelix`: a mirror sending the axis up, one sending it aside, a
    /// lens, then a mirror whose tilt folds the axis in a plane given by the up direction the axis
    /// arrives with, and a screen.
    fn bridge() -> OpmResult<[OpticRef; 5]> {
        Ok([
            OpticRef::new(Box::new(
                ThinMirror::new("up").with_tilt(degree!(45.0, 0.0, 0.0))?,
            )),
            OpticRef::new(Box::new(
                ThinMirror::new("aside").with_tilt(degree!(0.0, 45.0, 0.0))?,
            )),
            OpticRef::new(Box::new(ParaxialSurface::new("lens", millimeter!(1000.0))?)),
            OpticRef::new(Box::new(
                ThinMirror::new("fold").with_tilt(degree!(0.0, 45.0, 0.0))?,
            )),
            OpticRef::new(Box::new(Dummy::new("screen"))),
        ])
    }
    /// A group only organizes a model: grouping components, whether the group ends behind the
    /// mirrors that turn the up direction or starts after them, places every component exactly as
    /// the flat model does.
    #[test]
    fn grouping_does_not_change_placement() -> OpmResult<()> {
        let [up, aside, lens, fold, screen] = bridge()?;
        let ids = [&up, &aside, &lens, &fold, &screen].map(OpticRef::uuid);
        let flat = placements(
            &[
                up.clone(),
                aside.clone(),
                lens.clone(),
                fold.clone(),
                screen.clone(),
            ],
            &ids,
        )?;
        let turning_mirrors_grouped = placements(
            &[
                grouped(&[up.clone(), aside.clone(), lens.clone()])?,
                fold.clone(),
                screen.clone(),
            ],
            &ids,
        )?;
        assert_same_placements(&flat, &turning_mirrors_grouped);
        let fold_grouped = placements(&[up, aside, lens, grouped(&[fold, screen])?], &ids)?;
        assert_same_placements(&flat, &fold_grouped);
        Ok(())
    }
    /// The transmitted arm of a beam splitter continues the incoming axis unchanged, whatever the
    /// reflected arm does: it is placed exactly as behind a plain window, even when the splitter
    /// reflects out of the plane the up direction lies in.
    #[test]
    fn the_transmitted_arm_of_a_splitter_is_placed_as_behind_a_window() -> OpmResult<()> {
        let fold = OpticRef::new(Box::new(
            ThinMirror::new("fold").with_tilt(degree!(0.0, 45.0, 0.0))?,
        ));
        let screen = OpticRef::new(Box::new(Dummy::new("screen")));
        let ids = [&fold, &screen].map(OpticRef::uuid);
        let splitter = BeamSplitter::default().with_tilt(degree!(45.0, 0.0, 0.0))?;
        let behind_splitter = placements(
            &[
                OpticRef::new(Box::new(splitter)),
                fold.clone(),
                screen.clone(),
            ],
            &ids,
        )?;
        let window = OpticRef::new(Box::new(Dummy::new("window")));
        let behind_window = placements(&[window, fold, screen], &ids)?;
        assert_same_placements(&behind_window, &behind_splitter);
        Ok(())
    }
}
