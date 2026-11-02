#![warn(missing_docs)]

use super::{InvertGraphGuard, OpticGraph};
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNodeExt, PortType, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult, Rays},
    radian,
    utils::geom_transformation::Isometry,
};
use log::{info, warn};
use nalgebra::Point3;
use num_traits::Zero;
use petgraph::{
    Direction,
    algo::is_cyclic_directed,
    graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
};
use std::collections::{BTreeSet, VecDeque};
use uom::si::f64::Length;

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
    /// # Arguments
    ///
    /// * `incoming_data` - the light entering the graph through its external input ports.
    /// * `config` - the ray tracing configuration.
    /// * `ray_ends` - if given, collects the ray bundles leaving through output ports that are
    ///   neither connected nor mapped outward.
    ///
    /// # Errors
    ///
    /// This function returns an error if underlying functions fail.
    pub fn analyze_raytrace(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
        mut ray_ends: Option<&mut Vec<Rays>>,
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

                for (port, data) in outgoing_edges {
                    let leftover = guard.set_outgoing_edge_data(idx, &port, data);
                    // Only an actual output port can be an end: a nested group returns its
                    // incoming data along with its outputs, so input-port keys show up here too.
                    if let Some(ray_ends) = ray_ends.as_deref_mut()
                        && let Some(LightData::Geometric(rays)) = leftover
                        && guard.g[idx]
                            .ports()
                            .names(&PortType::Output)
                            .contains(&port)
                        && !guard.is_mapped_output_port(node_id, &port)
                    {
                        ray_ends.push(rays);
                    }
                }
            }
        }

        Ok(light_result)
    }

    /// Positions nodes in the graph based on optical ray propagation.
    ///
    /// The nodes are visited in the order `PlacingOrder` describes, which may offer a node that no
    /// axis reaches once more at the end.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the optical axis entering the graph through its external input ports.
    /// * `config` - the ray tracing configuration of the positioning run.
    /// * `ray_ends` - if given, collects the ray bundles leaving through output ports that are
    ///   neither connected nor mapped outward.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node positions could not be determined (e.g. optical axis misses element
    /// or "gets lost").
    pub fn calc_node_positions(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
        mut ray_ends: Option<&mut Vec<Rays>>,
    ) -> OpmResult<LightResult> {
        let mut order = PlacingOrder::new(self)?;
        let mut light_result = LightResult::default();

        while let Some((idx, last_offer)) = order.next() {
            if self.is_unreached_and_unplaced(idx)? {
                match self.place_in_front_of_successor(idx)? {
                    InFront::Placed => {}
                    InFront::NothingPlaced if !last_offer => {
                        order.put_back(self, idx);
                        continue;
                    }
                    InFront::NothingPlaced | InFront::Refused => {
                        let node_info = format!("{}", self.g[idx]);
                        warn!(
                            "{node_info} has no incoming connections and can thus not being placed. Skipping."
                        );
                        if !last_offer {
                            order.release(self, idx);
                        }
                        continue;
                    }
                }
            }
            calculate_single_node_position(
                self,
                idx,
                incoming_data,
                config,
                &mut light_result,
                ray_ends.as_deref_mut(),
            )?;
            if last_offer {
                self.end_axes_where_they_arrive(idx, ray_ends.as_deref_mut())?;
            } else {
                order.release(self, idx);
            }
        }

        Ok(light_result)
    }

    /// Ends the axes the node at `idx` sent out after the nodes it feeds were visited: each one
    /// becomes a ray end where it arrives at them.
    ///
    /// # Arguments
    ///
    /// * `idx` - the node that was visited after the nodes it feeds.
    /// * `ray_ends` - if given, collects the ends; otherwise nothing is done.
    ///
    /// # Errors
    ///
    /// Returns an error if an axis cannot be carried to where it arrives.
    fn end_axes_where_they_arrive(
        &mut self,
        idx: NodeIndex,
        ray_ends: Option<&mut Vec<Rays>>,
    ) -> OpmResult<()> {
        let Some(ray_ends) = ray_ends else {
            return Ok(());
        };
        let edges: Vec<EdgeIndex> = self
            .g
            .edges_directed(idx, Direction::Outgoing)
            .map(|edge| edge.id())
            .collect();
        for edge in edges {
            let Some((_, target)) = self.g.edge_endpoints(edge) else {
                continue;
            };
            let target = self.g[target].uuid();
            let Some(flow) = self.g.edge_weight_mut(edge) else {
                continue;
            };
            let Some(light) = flow.data_mut().take() else {
                continue;
            };
            let port = flow.target_port().to_owned();
            ray_ends.push(self.axis_arriving_at(target, &port, &light)?);
        }
        Ok(())
    }

    /// Returns whether no axis reaches the node at `idx` - it has no input connection - and it is
    /// not placed yet, neither absolutely nor by an earlier run.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be found in the graph.
    fn is_unreached_and_unplaced(&self, idx: NodeIndex) -> OpmResult<bool> {
        Ok(!self.has_input_connections(self.g[idx].uuid())?
            && self.g[idx].positioning().effective_position().is_none())
    }

    /// Places the node at `idx`, which no axis reaches, in front of the first of its successors
    /// that is placed: the face of its output port at the connection's distance before where the
    /// axis enters that successor, facing it. Its alignment then moves it from there.
    ///
    /// A placed successor is passed over, with a warning, if the light would pass it against the
    /// way it is placed - a reference turned other than its target - or if it has no surface at
    /// the port to tell where the light enters it, as a group.
    ///
    /// # Returns
    ///
    /// Whether the node was placed, or why not.
    ///
    /// # Errors
    ///
    /// Returns an error if the target of a reference cannot be found, if the geometry of the node
    /// cannot be built, or if its placement cannot be set.
    fn place_in_front_of_successor(&mut self, idx: NodeIndex) -> OpmResult<InFront> {
        let mut refused = false;
        let node_info = format!("{}", self.g[idx]);
        let connections = self.get_outgoing_connection_info_of_node(self.g[idx].uuid());
        let geometry = self.g[idx].create_geometry()?;
        for port in self.g[idx].ports().names(&PortType::Output) {
            let Some(connection) = connections.iter().find(|c| c.src_port == port) else {
                continue;
            };
            let Some(successor) = self.node_idx_by_uuid(connection.target_id) else {
                continue;
            };
            let inverted = self.g[successor].inverted();
            let passed = match self.g[successor].referenced_node_id() {
                Some(target) => self.reference_target_mut(target)?,
                None => &mut self.g[successor],
            };
            let Some(placed) = passed.positioning().effective_position().copied() else {
                continue;
            };
            if passed.inverted() != inverted {
                warn!(
                    "{node_info} cannot be placed in front of {passed}: the light would pass it against the way it is placed"
                );
                refused = true;
                continue;
            }
            if passed.face_of_port(&connection.target_port).is_err() {
                warn!(
                    "{node_info} cannot be placed in front of {passed}: it has no surface at port '{}' to tell where the light enters it",
                    connection.target_port
                );
                refused = true;
                continue;
            }
            let Some(exit) = self.g[idx]
                .face_of_port(&port)
                .ok()
                .and_then(|face| geometry.as_ref()?.face(face))
                .map(|face| *face.frame())
            else {
                continue;
            };
            let in_front = placed
                .append(&self.axis_entrance_frame_of(connection.target_id, &connection.target_port))
                .append(&Isometry::new_along_z(-connection.distance)?)
                .append(&Isometry::new_from_transform(exit.get_inv_transform()));
            self.g[idx].set_positioning(NodePositioning::Automatic(Some(in_front)))?;
            return Ok(InFront::Placed);
        }
        Ok(if refused {
            InFront::Refused
        } else {
            InFront::NothingPlaced
        })
    }
}

/// What became of placing a node in front of what it feeds (see
/// [`OpticGraph::place_in_front_of_successor`]).
enum InFront {
    /// The node stands in front of one of its successors.
    Placed,
    /// A successor is placed, but the node cannot stand in front of it; a warning said why.
    Refused,
    /// None of its successors is placed yet.
    NothingPlaced,
}

/// The order in which a positioning run visits the nodes of a graph, worked out as it goes.
///
/// It is a topological order that, of the nodes ready to be visited, takes ordinary nodes first,
/// then references, and last the nodes no axis reaches that are not placed yet. A component
/// reached directly and through a reference is thus placed by its own input, whatever order the
/// nodes were added in; a reference only places it when the light reaches the reference first on
/// the same path. A node no axis reaches comes last, so what it feeds is placed if anything
/// places it, and it can stand in front of it. If nothing it feeds is placed yet - a combiner
/// waiting for its light, say - it is put back: the nodes after it go on without it, and it is
/// offered once more when no other node is left. Ties go by node index.
struct PlacingOrder {
    /// The rank of every node, by its index: 0 for an ordinary node, 1 for a reference, 2 for a
    /// node no axis reaches that is not placed.
    ranks: Vec<u8>,
    /// How many of its predecessors every node, by its index, still waits for.
    waiting: Vec<usize>,
    /// The nodes ready to be visited, by rank and index.
    ready: BTreeSet<(u8, NodeIndex)>,
    /// The nodes put back, to be offered once more at the end.
    put_back: VecDeque<NodeIndex>,
}

impl PlacingOrder {
    /// Starts the order of the nodes of `graph`, with the nodes no connection leads to ready.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Analysis`] if the graph contains a cycle, or an error if a node
    /// cannot be found in it.
    fn new(graph: &OpticGraph) -> OpmResult<Self> {
        if is_cyclic_directed(&graph.g) {
            return Err(OpossumError::Analysis("topological sort failed".into()));
        }
        let ranks = graph
            .g
            .node_indices()
            .map(|idx| {
                Ok(if graph.is_unreached_and_unplaced(idx)? {
                    2
                } else {
                    u8::from(graph.g[idx].referenced_node_id().is_some())
                })
            })
            .collect::<OpmResult<Vec<u8>>>()?;
        let waiting: Vec<usize> = graph
            .g
            .node_indices()
            .map(|idx| graph.g.edges_directed(idx, Direction::Incoming).count())
            .collect();
        let ready = graph
            .g
            .node_indices()
            .filter(|idx| waiting.get(idx.index()) == Some(&0))
            .map(|idx| (ranks.get(idx.index()).copied().unwrap_or_default(), idx))
            .collect();
        Ok(Self {
            ranks,
            waiting,
            ready,
            put_back: VecDeque::new(),
        })
    }
    /// Returns the next node to visit, and whether it was put back before: then this is its last
    /// offer. `None` once every node was offered.
    fn next(&mut self) -> Option<(NodeIndex, bool)> {
        if let Some((_, idx)) = self.ready.pop_first() {
            return Some((idx, false));
        }
        self.put_back.pop_front().map(|idx| (idx, true))
    }
    /// Lets the nodes after the node at `idx` of `graph` stop waiting for it.
    fn release(&mut self, graph: &OpticGraph, idx: NodeIndex) {
        for edge in graph.g.edges_directed(idx, Direction::Outgoing) {
            let next = edge.target();
            if let Some(count) = self.waiting.get_mut(next.index()) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    let rank = self.ranks.get(next.index()).copied().unwrap_or_default();
                    self.ready.insert((rank, next));
                }
            }
        }
    }
    /// Puts the node at `idx` of `graph` back: the nodes after it go on without it, and it is
    /// offered once more when no other node is left.
    fn put_back(&mut self, graph: &OpticGraph, idx: NodeIndex) {
        self.release(graph, idx);
        self.put_back.push_back(idx);
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

/// Places one node of the graph on the optical axis and passes the axis on to its successors.
///
/// A node with inputs that no axis reaches - one in a strand no source feeds - is passed over
/// with a warning: there is nothing to place it along, and nothing to pass on.
///
/// # Arguments
///
/// * `graph` - The optical graph to operate on.
/// * `node_idx` - Index of the node to place.
/// * `incoming_data` - The optical axis entering the graph through its external input ports.
/// * `up_direction` - Running "up" direction vector, updated by this function.
/// * `config` - The ray tracing configuration of the positioning run.
/// * `light_result` - Collects what leaves the graph through its mapped output ports.
/// * `ray_ends` - Optional collector for unconnected, unmapped ray bundles.
///
/// # Errors
///
/// This function returns an error if the node cannot be placed, if its calculation fails while
/// it has successors, or if its outgoing beams cannot be passed on.
fn calculate_single_node_position(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_data: &LightResult,
    config: &RayTraceConfig,
    light_result: &mut LightResult,
    mut ray_ends: Option<&mut Vec<Rays>>,
) -> OpmResult<()> {
    let node_id = graph.g[node_idx].uuid();
    let node_info = format!("{}", graph.g[node_idx]);
    let incoming_edges: LightResult = graph.get_incoming(node_id, incoming_data)?;

    ensure_node_isometry(graph, node_idx, &incoming_edges)?;

    if incoming_edges.is_empty() && !graph.g[node_idx].ports().names(&PortType::Input).is_empty() {
        warn!(
            "{node_info} got no valid optical axis data from previous node and can thus not being placed. Skipping."
        );
        return Ok(());
    }
    // Kept only if a failing node without successors may have to end the axis it got.
    let arriving = ray_ends.is_some().then(|| incoming_edges.clone());
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
            // The axis this node was placed by ends where it sits.
            if let (Some(ray_ends), Some(arriving)) = (ray_ends.as_deref_mut(), arriving) {
                for port in graph.g[node_idx].ports().names(&PortType::Input) {
                    if let Some(light) = arriving.get(&port) {
                        ray_ends.push(graph.axis_arriving_at(node_id, &port, light)?);
                    }
                }
            }
            LightResult::default()
        }
    };

    let has_successors = graph.has_output_connections(node_id)?;
    graph.collect_group_output_ports(node_id, &outgoing_edges, light_result)?;
    for (port, data) in outgoing_edges {
        // The first ray of a bundle carries the axis. Swallowed by the node or stopped beside it
        // (`RayTraceConfig::for_positioning`), it gives no direction to place a successor along.
        if has_successors
            && matches!(&data, LightData::Geometric(rays) if rays.iter().next().is_none_or(|ray| !ray.valid()))
        {
            return Err(OpossumError::Analysis(format!(
                "no light leaves {node_info} at port '{port}' within its clear aperture: the \
                 optical axis ends there, so the components after it cannot be placed"
            )));
        }
        let leftover = graph.set_outgoing_edge_data(node_idx, &port, data);
        if let Some(ray_ends) = ray_ends.as_deref_mut()
            && let Some(LightData::Geometric(rays)) = leftover
            && !graph.is_mapped_output_port(node_id, &port)
        {
            ray_ends.push(rays);
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{
        analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
        apertures::{CircleShape, OpticShape},
        core_optics::{
            Alignable, NodeAttrExt, OpticNode, OpticNodeExt, OpticRef, PortType,
            node_attr::NodePositioning,
        },
        degree,
        error::{OpmResult, OpossumError},
        geometry::body::{OPTIC_SHAPE, default_optic_shape},
        joule,
        light::LightResult,
        millimeter,
        nodes::{
            BeamSplitter, Dummy, IdealFilter, Lens, NodeGroup, NodeReference, ParaxialSurface,
            SourcePort, SpotDiagram, ThinMirror,
            ideal_filter::{FilterConst, FilterTypeBuilder},
            round_collimated_ray_builder,
        },
        percent,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use nalgebra::{MatrixXx3, Point3, Vector3};
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
    /// Put a source in front of the given row of nodes, 100 mm apart, and position the row.
    ///
    /// # Returns
    ///
    /// The positioned scenery.
    ///
    /// # Errors
    ///
    /// Returns an error if positioning fails.
    fn position_behind_a_source(mut scenery: NodeGroup, row: &[Uuid]) -> OpmResult<NodeGroup> {
        let source = scenery.add_node(SourcePort::default())?;
        let mut upstream = source;
        for &node in row {
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
        Ok(scenery)
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
        let screen = scenery.add_node(Dummy::default())?;
        position_behind_a_source(scenery, &[row, &[screen]].concat())?
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
    /// With nothing after it, a component the optical axis misses leaves nothing unplaced: the run
    /// finishes and the component itself is placed.
    #[test]
    fn positioning_passes_a_missed_component_with_nothing_after_it() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let lens = scenery.add_node(decentred_lens(default_optic_shape())?)?;
        let scenery = position_behind_a_source(scenery, &[lens])?;
        assert!(scenery.node(lens)?.effective_position().is_some());
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
    /// Feed a lens directly by `first` and, through a reference, by `second`, both 100 mm in front
    /// of it, and position the model.
    ///
    /// # Arguments
    ///
    /// * `first` - the source feeding the lens directly.
    /// * `second` - the source feeding the reference.
    /// * `reference_inverted` - whether the reference passes the lens backwards.
    /// * `reference_strand_first` - whether `second` and the reference are added to the model
    ///   before `first`.
    ///
    /// # Returns
    ///
    /// The positioned model, the uuid of the lens and the uuid of `second`.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be built or positioning fails.
    fn lens_fed_twice(
        first: SourcePort,
        second: SourcePort,
        reference_inverted: bool,
        reference_strand_first: bool,
    ) -> OpmResult<(NodeGroup, Uuid, Uuid)> {
        let mut scenery = NodeGroup::default();
        let lens = scenery.add_node(Dummy::new("lens"))?;
        let mut reference = NodeReference::from_node(&scenery.node(lens)?)?;
        reference.set_inverted(reference_inverted)?;
        let entrance = reference.ports().names(&PortType::Input)[0].clone();
        let (first, second, reference) = if reference_strand_first {
            let second = scenery.add_node(second)?;
            let reference = scenery.add_node(reference)?;
            (scenery.add_node(first)?, second, reference)
        } else {
            let first = scenery.add_node(first)?;
            let second = scenery.add_node(second)?;
            (first, second, scenery.add_node(reference)?)
        };
        scenery.connect_nodes(first, "output_1", lens, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(second, "output_1", reference, &entrance, millimeter!(100.0))?;
        let mut config = RayTraceConfig::default();
        for source in [first, second] {
            config.map_source(
                source,
                round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
            );
        }
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        Ok((scenery, lens, second))
    }
    /// Where the node `id` of `scenery` was placed.
    fn placement(scenery: &NodeGroup, id: Uuid) -> OpmResult<Isometry> {
        scenery
            .node(id)?
            .effective_position()
            .copied()
            .ok_or_else(|| OpossumError::Other(format!("node {id} was not placed")))
    }
    /// A source whose position is left to the positioning run.
    fn automatic_source(name: &str) -> OpmResult<SourcePort> {
        let mut source = SourcePort::new(name);
        source.set_positioning(NodePositioning::Automatic(None))?;
        Ok(source)
    }
    /// A component reached directly and through a reference is placed by its own input, whatever
    /// order the nodes were added in: a second source standing aside does not move it.
    #[test]
    fn a_component_is_placed_by_its_own_input_not_through_a_reference() -> OpmResult<()> {
        for reference_strand_first in [false, true] {
            let mut aside = SourcePort::new("aside");
            aside.set_positioning(NodePositioning::Absolute(Isometry::new(
                millimeter!(5.0, 0.0, 0.0),
                degree!(0.0, 0.0, 0.0),
            )?))?;
            let (scenery, lens, _) =
                lens_fed_twice(SourcePort::default(), aside, false, reference_strand_first)?;
            let at = placement(&scenery, lens)?.translation();
            assert!(
                at.x.value.abs() < 1e-12 && (at.z.value - 0.1).abs() < 1e-12,
                "reference strand added first: {reference_strand_first}, lens at {at:?}"
            );
        }
        Ok(())
    }
    /// An automatic source feeding a reference stands 100 mm in front of the lens it refers to,
    /// along the lens's axis, whatever order the nodes were added in. Its alignment moves it from
    /// there.
    #[test]
    fn an_automatic_source_stands_in_front_of_the_component_it_feeds() -> OpmResult<()> {
        let decenter = millimeter!(0.0, 2.0, 0.0);
        for reference_strand_first in [false, true] {
            let mut tilted = SourcePort::new("tilted");
            tilted.set_positioning(NodePositioning::Absolute(Isometry::new(
                millimeter!(0.0, 0.0, 0.0),
                degree!(0.0, 30.0, 0.0),
            )?))?;
            let automatic = automatic_source("automatic")?.with_decenter(decenter)?;
            let (scenery, lens, automatic) =
                lens_fed_twice(tilted, automatic, false, reference_strand_first)?;
            let in_front =
                placement(&scenery, lens)?.append(&Isometry::new_along_z(millimeter!(-100.0))?);
            let aligned = in_front.append(&Isometry::new(decenter, degree!(0.0, 0.0, 0.0))?);
            let source = scenery.node(automatic)?;
            assert_same_placements(
                &[in_front, aligned],
                &[
                    placement(&scenery, automatic)?,
                    source.effective_surface_iso("output_1")?,
                ],
            );
        }
        Ok(())
    }
    /// An automatic source feeding the second input of an absolutely placed beam splitter stands
    /// 50 mm in front of it on the side that input faces, aimed at the splitter's center.
    #[test]
    fn an_automatic_source_faces_the_second_input_of_a_splitter() -> OpmResult<()> {
        let (scenery, splitter, second) = splitter_fed_on_its_second_input(false)?;
        assert_aimed_at(
            &placement(&scenery, second)?,
            &placement(&scenery, splitter)?,
            0.05,
        );
        Ok(())
    }
    /// A splitter placed by a source on its first input combines it with the light of an
    /// automatic source on its second input. The splitter waits for both sources, and the
    /// automatic one for the splitter to be placed: it is put back, the splitter is placed by the
    /// first source, and then the automatic source stands in front of the second input. Its axis
    /// ends at the splitter's center, which was visited before it.
    #[test]
    fn an_automatic_source_stands_in_front_of_a_combiner_placed_by_another_source() -> OpmResult<()>
    {
        let (scenery, splitter, second) = splitter_fed_on_its_second_input(true)?;
        let center = placement(&scenery, splitter)?;
        assert_relative_eq!(center.translation().z.value, 0.1, epsilon = 1e-12);
        let start = placement(&scenery, second)?;
        assert_aimed_at(&start, &center, 0.05);
        let runs_from_to = |path: &MatrixXx3<Length>, from: &Isometry, to: &Isometry| {
            let near = |row: usize, point: Point3<Length>| {
                (0..3).all(|i| (path[(row, i)] - point[i]).value.abs() < 1e-9)
            };
            path.nrows() == 2 && near(0, from.translation()) && near(1, to.translation())
        };
        let ends_at_center = scenery.ray_ends().iter().any(|end| {
            end.iter().next().is_some_and(|ray| {
                runs_from_to(&ray.position_history_with_current(), &start, &center)
            })
        });
        assert!(
            ends_at_center,
            "the axis of the automatic source ends at the splitter"
        );
        Ok(())
    }
    /// A beam splitter turned by 45° whose center is 100 mm along z, fed on its second input by an
    /// automatic source 50 mm away, positioned with the ends of its axes collected.
    ///
    /// # Arguments
    ///
    /// * `placed_by_a_source` - whether a source at the origin, feeding the first input, places the
    ///   splitter; otherwise it is placed there absolutely.
    ///
    /// # Returns
    ///
    /// The positioned model, the uuid of the splitter and the uuid of the automatic source.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be built or positioning fails.
    fn splitter_fed_on_its_second_input(
        placed_by_a_source: bool,
    ) -> OpmResult<(NodeGroup, Uuid, Uuid)> {
        let mut scenery = NodeGroup::default();
        let mut splitter = BeamSplitter::default().with_tilt(degree!(45.0, 0.0, 0.0))?;
        if !placed_by_a_source {
            splitter.set_positioning(NodePositioning::Absolute(Isometry::new_along_z(
                millimeter!(100.0),
            )?))?;
        }
        let splitter = scenery.add_node(splitter)?;
        let second = scenery.add_node(automatic_source("second")?)?;
        scenery.connect_nodes(second, "output_1", splitter, "input_2", millimeter!(50.0))?;
        let mut sources = vec![second];
        if placed_by_a_source {
            let first = scenery.add_node(SourcePort::default())?;
            scenery.connect_nodes(first, "output_1", splitter, "input_1", millimeter!(100.0))?;
            sources.push(first);
        }
        let mut config = RayTraceConfig::default();
        for source in sources {
            config.map_source(
                source,
                round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
            );
        }
        let mut config = config.for_positioning();
        config.set_collect_ray_ends(true);
        AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &config)?;
        Ok((scenery, splitter, second))
    }
    /// Assert that `source` stands `distance` meters from `target`, aimed at it from the side,
    /// across the z axis.
    fn assert_aimed_at(source: &Isometry, target: &Isometry, distance: f64) {
        let (from, to) = (source.translation(), target.translation());
        let offset = Vector3::new(
            (to.x - from.x).value,
            (to.y - from.y).value,
            (to.z - from.z).value,
        );
        let direction = source.transform_vector_f64(&Vector3::z());
        assert_relative_eq!(offset.norm(), distance, epsilon = 1e-12);
        assert_relative_eq!(offset.normalize(), direction, epsilon = 1e-12);
        assert_relative_eq!(direction.z, 0.0, epsilon = 1e-12);
    }
    /// An automatic source is not placed in front of a component the light would pass against the
    /// way it is placed - a reference turned other than the lens it refers to - nor in front of a
    /// group, which has no surface of its own to tell where the light enters it. It says why.
    #[test]
    fn an_automatic_source_is_not_placed_where_its_light_cannot_enter() -> OpmResult<()> {
        testing_logger::setup();
        let (scenery, _, automatic) = lens_fed_twice(
            SourcePort::default(),
            automatic_source("backwards")?,
            true,
            false,
        )?;
        assert!(scenery.node(automatic)?.effective_position().is_none());

        let mut group = NodeGroup::new("group");
        let inner = group.add_node(Dummy::default())?;
        group.map_input_port(inner, "input_1", "input_1")?;
        group.set_positioning(NodePositioning::Absolute(Isometry::new_along_z(
            millimeter!(100.0),
        )?))?;
        let mut scenery = NodeGroup::default();
        let group = scenery.add_node(group)?;
        let source = scenery.add_node(automatic_source("into group")?)?;
        scenery.connect_nodes(source, "output_1", group, "input_1", millimeter!(50.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        assert!(scenery.node(source)?.effective_position().is_none());
        testing_logger::validate(|logs| {
            let warned = |about: &str| {
                logs.iter()
                    .any(|log| log.level == log::Level::Warn && log.body.contains(about))
            };
            assert!(warned("against the way it is placed"), "inverted reference");
            assert!(warned("no surface at port"), "group");
        });
        Ok(())
    }
}
