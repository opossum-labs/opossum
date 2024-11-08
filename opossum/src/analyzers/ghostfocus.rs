//! Analyzer performing a ghost focus analysis using ray tracing
use std::collections::{hash_map::Values, HashMap};

use chrono::Local;
use log::{info, warn};
use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter, radiant_exposure::joule_per_square_centimeter};
use uuid::Uuid;

use crate::{
    error::{OpmResult, OpossumError},
    get_version,
    light_result::{LightRays, LightResult},
    millimeter,
    nodes::{NodeGroup, OpticGraph},
    optic_node::OpticNode,
    optic_ports::PortType,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{
        proptype::{count_str, format_value_with_prefix},
        Properties, Proptype,
    },
    rays::Rays,
    reporting::{analysis_report::AnalysisReport, node_report::NodeReport},
};

use super::{raytrace::AnalysisRayTrace, Analyzer, AnalyzerType, RayTraceConfig};
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// Configuration for performing a ghost focus analysis
pub struct GhostFocusConfig {
    max_bounces: usize,
}

impl GhostFocusConfig {
    /// Returns the max bounces of this [`GhostFocusConfig`].
    #[must_use]
    pub const fn max_bounces(&self) -> usize {
        self.max_bounces
    }
    /// Sets the maximum number of ray bounces to be considered during ghost focus analysis.
    pub fn set_max_bounces(&mut self, max_bounces: usize) {
        self.max_bounces = max_bounces;
    }
}
impl Default for GhostFocusConfig {
    fn default() -> Self {
        Self { max_bounces: 1 }
    }
}
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
}
impl Analyzer for GhostFocusAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Calculate node positions of scenery{scenery_name}.");
        AnalysisRayTrace::calc_node_position(
            scenery,
            LightResult::default(),
            &RayTraceConfig::default(),
        )?;
        info!(
            "Performing ghost focus analysis of scenery{scenery_name} up to {} ray bounces.",
            self.config.max_bounces
        );
        scenery.clear_edges();
        for bounce in 0..=self.config.max_bounces {
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
            scenery.clear_edges();
            for rays in &ray_collection {
                scenery.add_to_accumulated_rays(rays, bounce);
            }
        }
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::new(get_version(), Local::now());
        analysis_report.add_scenery(scenery);
        let mut props = Properties::default();
        let ghost_focus_history = GhostFocusHistory::from(scenery.accumulated_rays().clone());

        let proptype = Proptype::from(ghost_focus_history);
        props.create("propagation", "ray propagation", None, proptype)?;

        let mut node_report =
            NodeReport::new("ray propagation", "Global ray propagation", "global", props);
        node_report.set_show_item(true);
        analysis_report.add_node_report(node_report);

        for node in scenery.graph().nodes() {
            let node_name = &node.optical_ref.borrow().name();
            let hit_maps = node.optical_ref.borrow().hit_maps();
            for hit_map in &hit_maps {
                let critical_positions = hit_map.1.critical_fluences();
                if !critical_positions.is_empty() {
                    for (i, (rays_uuid, (fluence, hist_idx, bounce))) in
                        critical_positions.iter().enumerate()
                    {
                        let critical_ghost_hist = GhostFocusHistory::from((
                            scenery.accumulated_rays(),
                            *rays_uuid,
                            *hist_idx,
                        ));
                        let origin_str =
                            critical_ghost_hist.rays_origin_report_str(scenery.graph());
                        let mut hit_map_props = Properties::default();
                        hit_map_props.create(
                            "Origin",
                            "Surface bounces that enabled this fluence",
                            None,
                            origin_str.clone().into(),
                        )?;
                        let fluence_data = hit_map
                            .1
                            .get_rays_hit_map(*bounce, rays_uuid)
                            .unwrap()
                            .calc_fluence_with_kde((100, 100), None)?;
                        hit_map_props.create(
                            "Peak fluence (Voronoi)",
                            "Peak fluence on this surface using Voronoi estimator",
                            None,
                            format!(
                                "{} J/cm²",
                                format_value_with_prefix(
                                    fluence.get::<joule_per_square_centimeter>()
                                )
                            )
                            .into(),
                        )?;
                        hit_map_props.create(
                            "Ray propagation",
                            "ray propagation",
                            None,
                            Proptype::from(critical_ghost_hist),
                        )?;
                        hit_map_props.create(
                            "Peak fluence (KDE)",
                            "Peak fluence on this surface using kernel density estimator",
                            None,
                            format!(
                                "{} J/cm²",
                                format_value_with_prefix(
                                    fluence_data
                                        .get_peak_fluence()
                                        .get::<joule_per_square_centimeter>()
                                )
                            )
                            .into(),
                        )?;
                        hit_map_props.create(
                            "Fluence",
                            "2D spatial energy distribution",
                            None,
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
        analysis_report.set_analysis_type("Ghost Focus Analysis");
        Ok(analysis_report)
    }
}

/// Trait for implementing the energy flow analysis.
pub trait AnalysisGhostFocus: OpticNode + AnalysisRayTrace {
    /// Perform a ghost focus analysis of an [`OpticNode`].
    ///
    /// This function is similar to the corresponding [`AnalysisRayTrace`] function but also
    /// considers possible reflected [`Rays`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn analyze(
        &mut self,
        _incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        warn!(
            "{}: No ghost focus analysis function defined.",
            self.node_type()
        );
        Ok(LightRays::default())
    }

    /// Effectively the analyze function of detector nodes with a single surface for a ghost-focus analysis
    /// Helper function to reduce code-doubling
    /// # Attributes
    /// - `incoming_data`: the incoming data for this anaylsis in form of [`LightRays`]
    /// - `config`: the [`RayTraceConfig`] of this analysis
    /// # Errors
    /// This function errors if `pass_through_detector_surface` fails    
    fn analyze_single_surface_detector(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
    ) -> OpmResult<LightRays> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(bouncing_rays) = incoming_data.get(in_port) else {
            let mut out_light_rays = LightRays::default();
            out_light_rays.insert(out_port.into(), Vec::<Rays>::new());
            return Ok(out_light_rays);
        };
        let mut rays = bouncing_rays.clone();
        self.pass_through_detector_surface(
            in_port,
            &mut rays,
            &AnalyzerType::GhostFocus(config.clone()),
        )?;

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays);
        Ok(out_light_rays)
    }
}

///Struct to store the node origin uuid and parent ray bundle Uuid of a ray bundle
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RaysOrigin {
    parent_rays: Option<Uuid>,
    node_origin: Option<Uuid>,
}
impl RaysOrigin {
    ///creates a new [`RaysOrigin`]
    #[must_use]
    pub const fn new(parent_rays: Option<Uuid>, node_origin: Option<Uuid>) -> Self {
        Self {
            parent_rays,
            node_origin,
        }
    }
}

/// Struct to store the correlation between a ray bundle and its parent ray bundle as well as its node origin
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RaysNodeCorrelation {
    correlation: HashMap<Uuid, RaysOrigin>,
}
impl RaysNodeCorrelation {
    ///creates a new [`RaysNodeCorrelation`]
    #[must_use]
    pub fn new(rays_uuid: &Uuid, rays_origin: &RaysOrigin) -> Self {
        let mut correlation = HashMap::<Uuid, RaysOrigin>::new();
        correlation.insert(*rays_uuid, rays_origin.clone());
        Self { correlation }
    }

    /// inserts a key value pair in the correlation hashmap
    pub fn insert(&mut self, k: &Uuid, v: &RaysOrigin) {
        self.correlation.insert(*k, v.clone());
    }

    /// returns the values of the correlation hashmap
    #[must_use]
    pub fn values(&self) -> Values<'_, Uuid, RaysOrigin> {
        self.correlation.values()
    }
}

/// struct that holds the history of the ray positions that is needed for report generation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GhostFocusHistory {
    /// vector of ray positions for each raybundle at a specifc spectral position
    pub rays_pos_history: Vec<Vec<Vec<MatrixXx3<Length>>>>,
    /// view direction if the ray position history is plotted
    pub plot_view_direction: Option<Vector3<f64>>,
    ///stores the corrleation between a rays bundle and its parent node as well as parent ray bundle for each bounce in a vector
    pub ray_node_correlation: Vec<RaysNodeCorrelation>,
}
impl GhostFocusHistory {
    /// Projects the positions o fthie [`GhostFocusHistory`] onto a 2D plane
    /// # Attributes
    /// `plane_normal_vec`: normal vector of the plane to project onto
    ///
    /// # Errors
    /// This function errors if the length of the plane normal vector is zero
    /// # Returns
    /// This function returns a set of 2d vectors in the defined plane projected to a view that is perpendicular to this plane.
    pub fn project_to_plane(
        &self,
        plane_normal_vec: Vector3<f64>,
    ) -> OpmResult<Vec<Vec<Vec<MatrixXx2<Length>>>>> {
        let vec_norm = plane_normal_vec.norm();

        if vec_norm < f64::EPSILON {
            return Err(OpossumError::Other(
                "The plane normal vector must have a non-zero length!".into(),
            ));
        }

        let normed_normal_vec = plane_normal_vec / vec_norm;

        //define an axis on the plane.
        //Do this by projection of one of the main coordinate axes onto that plane
        //Beforehand check, if these axes are not parallel to the normal vec
        let (co_ax_1, co_ax_2) = if plane_normal_vec.cross(&Vector3::x()).norm() < f64::EPSILON {
            //parallel to the x-axis
            (Vector3::z(), Vector3::y())
        } else if plane_normal_vec.cross(&Vector3::y()).norm() < f64::EPSILON {
            (Vector3::z(), Vector3::x())
        } else if plane_normal_vec.cross(&Vector3::z()).norm() < f64::EPSILON {
            (Vector3::x(), Vector3::y())
        } else {
            //arbitrarily project x-axis onto that plane
            let x_vec = Vector3::x();
            let mut proj_x = x_vec - x_vec.dot(&normed_normal_vec) * plane_normal_vec;
            proj_x /= proj_x.norm();

            //second axis defined by cross product of x-axis projection and plane normal, which yields another vector that is perpendicular to both others
            (proj_x, proj_x.cross(&normed_normal_vec))
        };

        let mut projected_history =
            Vec::<Vec<Vec<MatrixXx2<Length>>>>::with_capacity(self.rays_pos_history.len());
        for ray_vec_in_bounce in &self.rays_pos_history {
            let mut rays_vec_pos_projection =
                Vec::<Vec<MatrixXx2<Length>>>::with_capacity(ray_vec_in_bounce.len());
            for ray_bundle in ray_vec_in_bounce {
                let mut rays_pos_projection =
                    Vec::<MatrixXx2<Length>>::with_capacity(ray_bundle.len());
                for ray_pos in ray_bundle {
                    let mut projected_ray_pos = MatrixXx2::<Length>::zeros(ray_pos.column(0).len());
                    for (row, pos) in ray_pos.row_iter().enumerate() {
                        // let pos_t = Vector3::from_vec(pos.transpose().iter().map(|p| p.get::<millimeter>()).collect::<Vec<f64>>());
                        let pos_t = Vector3::from_vec(
                            pos.iter()
                                .map(uom::si::f64::Length::get::<millimeter>)
                                .collect::<Vec<f64>>(),
                        );
                        let proj_pos = pos_t - pos_t.dot(&normed_normal_vec) * plane_normal_vec;

                        projected_ray_pos[(row, 0)] = millimeter!(proj_pos.dot(&co_ax_1));
                        projected_ray_pos[(row, 1)] = millimeter!(proj_pos.dot(&co_ax_2));
                    }
                    rays_pos_projection.push(projected_ray_pos);
                }
                rays_vec_pos_projection.push(rays_pos_projection);
            }
            projected_history.push(rays_vec_pos_projection);
        }

        Ok(projected_history)
    }

    fn add_specific_ray_history(
        &mut self,
        accumulated_rays: &Vec<HashMap<Uuid, Rays>>,
        rays_uuid: &Uuid,
        hist_idx: usize,
    ) {
        for (bounce, ray_vecs_in_bounce) in accumulated_rays.iter().enumerate() {
            if ray_vecs_in_bounce.contains_key(rays_uuid) {
                let mut rays_per_bounce_history =
                    Vec::<Vec<MatrixXx3<Length>>>::with_capacity(ray_vecs_in_bounce.len());
                if let Some(rays) = ray_vecs_in_bounce.get(rays_uuid) {
                    let mut rays_history =
                        Vec::<MatrixXx3<Length>>::with_capacity(rays.nr_of_rays(true));
                    for ray in rays {
                        if let Some(ray_hist) = ray.position_history_from_to(0, hist_idx) {
                            rays_history.push(ray_hist);
                        }
                    }
                    rays_per_bounce_history.push(rays_history);
                    self.ray_node_correlation[bounce].insert(
                        rays.uuid(),
                        &RaysOrigin::new(*rays.parent_id(), *rays.node_origin()),
                    );

                    self.rays_pos_history[bounce] = rays_per_bounce_history;
                    if let Some(parent_uuid) = rays.parent_id() {
                        self.add_specific_ray_history(
                            accumulated_rays,
                            parent_uuid,
                            *rays.parent_pos_split_idx(),
                        );
                    }
                }
                break;
            }
        }
    }

    ///Returns the report string for the critical ray origin in the ghost focus analysis
    #[must_use]
    pub fn rays_origin_report_str(&self, graph: &OpticGraph) -> String {
        let mut report_str = String::new();
        for (bounce, rays_correlation) in self.ray_node_correlation.iter().enumerate() {
            for rays_origin in rays_correlation.values() {
                if let Some(node_uuid) = rays_origin.node_origin {
                    if bounce == 0 {
                        report_str += "Origin at node '";
                    } else {
                        report_str += format!("bounce {bounce} at node '").as_str();
                    }
                    if let Some(opt_ref) = graph.node_by_uuid(node_uuid) {
                        report_str +=
                            format!("{}', ", opt_ref.optical_ref.borrow().name()).as_str();
                    }
                }
            }
        }
        report_str
    }
}

impl From<Vec<HashMap<Uuid, Rays>>> for GhostFocusHistory {
    fn from(value: Vec<HashMap<Uuid, Rays>>) -> Self {
        let mut ghost_focus_history =
            Vec::<Vec<Vec<MatrixXx3<Length>>>>::with_capacity(value.len());
        let mut ray_node_correlation = Vec::<RaysNodeCorrelation>::with_capacity(value.len());
        for ray_vecs_in_bounce in &value {
            let mut rays_per_bounce_history =
                Vec::<Vec<MatrixXx3<Length>>>::with_capacity(ray_vecs_in_bounce.len());
            let mut ray_node_bounce_correlation = RaysNodeCorrelation::default();
            for rays in ray_vecs_in_bounce.values() {
                let mut rays_history =
                    Vec::<MatrixXx3<Length>>::with_capacity(rays.nr_of_rays(false));
                for ray in rays {
                    rays_history.push(ray.position_history());
                }
                ray_node_bounce_correlation.insert(
                    rays.uuid(),
                    &RaysOrigin::new(*rays.parent_id(), *rays.node_origin()),
                );
                rays_per_bounce_history.push(rays_history);
            }
            ghost_focus_history.push(rays_per_bounce_history);
            ray_node_correlation.push(ray_node_bounce_correlation);
        }
        Self {
            rays_pos_history: ghost_focus_history,
            plot_view_direction: None,
            ray_node_correlation,
        }
    }
}

impl From<(&Vec<HashMap<Uuid, Rays>>, Uuid, usize)> for GhostFocusHistory {
    ///value contains :
    /// 0: a vector of Hashmaps that contain Rays. Same structure as the `accumulated_rays` in [`NodeGroup`]
    /// 1: the uuid of a ray bundle within field 0
    /// 2: the index of the position in the ray position history up to which it should be displayed
    fn from(value: (&Vec<HashMap<Uuid, Rays>>, Uuid, usize)) -> Self {
        let (acc_rays, rays_uuid, hist_idx) = value;
        let mut ray_pos_history = Vec::<Vec<Vec<MatrixXx3<Length>>>>::with_capacity(acc_rays.len());
        let mut ray_node_correlation = Vec::<RaysNodeCorrelation>::with_capacity(acc_rays.len());
        for _i in 0..acc_rays.len() {
            ray_pos_history.push(Vec::<Vec<MatrixXx3<Length>>>::new());
            ray_node_correlation.push(RaysNodeCorrelation::default());
        }
        let mut ghost_focus_history = Self {
            rays_pos_history: ray_pos_history,
            plot_view_direction: None,
            ray_node_correlation,
        };

        ghost_focus_history.add_specific_ray_history(acc_rays, &rays_uuid, hist_idx);

        ghost_focus_history
    }
}

impl Plottable for GhostFocusHistory {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("distance in mm (y axis)".into()))?
            .set(&PlotArgs::PlotSize((1200, 1200)))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::Legend(false))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::MultiLine2D(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        _plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        if self.rays_pos_history.is_empty() {
            Ok(None)
        } else {
            let num_series = self.rays_pos_history.len();
            let mut plt_series = Vec::<PlotSeries>::with_capacity(num_series);

            let Some(plot_view_direction) = self.plot_view_direction else {
                return Err(OpossumError::Other("cannot get plot series for raypropagationvisualizer, plot_view_direction not defined".into()));
            };

            let projected_positions = self.project_to_plane(plot_view_direction)?;
            for (i, bounce_positions) in projected_positions.iter().enumerate() {
                let mut proj_pos_mm =
                    Vec::<MatrixXx2<f64>>::with_capacity(projected_positions.len());
                for rays_in_bounce in bounce_positions {
                    for ray_pos in rays_in_bounce {
                        proj_pos_mm.push(MatrixXx2::from_vec(
                            ray_pos
                                .iter()
                                .map(uom::si::f64::Length::get::<millimeter>)
                                .collect::<Vec<f64>>(),
                        ));
                    }
                }
                let gradient = colorous::TURBO;

                let c = if projected_positions.len() > 10 {
                    gradient.eval_rational(i, projected_positions.len())
                } else {
                    colorous::CATEGORY10[i]
                };

                let plt_data = PlotData::MultiDim2 {
                    vec_of_xy_data: proj_pos_mm,
                };
                let series_label = format!("Bounce: {i}");

                plt_series.push(PlotSeries::new(
                    &plt_data,
                    RGBAColor(c.r, c.g, c.b, 0.1),
                    Some(series_label),
                ));
            }

            Ok(Some(plt_series))
        }
    }
}
