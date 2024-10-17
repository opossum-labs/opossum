//! Analyzer performing a ghost focus analysis using ray tracing
use chrono::Local;
use log::{info, warn};
use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

use crate::{
    error::{OpmResult, OpossumError},
    get_version,
    light_result::{LightRays, LightResult},
    millimeter,
    nodes::NodeGroup,
    optic_node::OpticNode,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::analysis_report::{AnalysisReport, NodeReport},
};

use super::{raytrace::AnalysisRayTrace, Analyzer, RayTraceConfig};
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

        let node_report =
            NodeReport::new("ray propagation", "Global ray propagation", "global", props);
        analysis_report.add_node_report(node_report);
        for node in scenery.graph().nodes() {
            let node_name = &node.optical_ref.borrow().name();
            let uuid = node.uuid().as_simple().to_string();
            let mut props = Properties::default();
            let hit_maps = node.optical_ref.borrow().hit_maps();
            for hit_map in &hit_maps {
                props.create(hit_map.0, "surface hit map", None, hit_map.1.clone().into())?;
            }
            let node_report = NodeReport::new("hitmap", node_name, &uuid, props);
            analysis_report.add_node_report(node_report);
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
    /// considers possible reflected [`Rays`](crate::rays::Rays).
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn analyze(
        &mut self,
        _incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
    ) -> OpmResult<LightRays> {
        warn!(
            "{}: No ghost focus analysis function defined.",
            self.node_type()
        );
        Ok(LightRays::default())
    }
}

/// struct that holds the history of the ray positions that is needed for report generation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GhostFocusHistory {
    /// vector of ray positions for each raybundle at a specifc spectral position
    pub rays_pos_history: Vec<Vec<MatrixXx3<Length>>>,
    /// view direction if the rayposition thistory is plotted
    pub plot_view_direction: Option<Vector3<f64>>,
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
    ) -> OpmResult<Vec<Vec<MatrixXx2<Length>>>> {
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
            Vec::<Vec<MatrixXx2<Length>>>::with_capacity(self.rays_pos_history.len());
        for ray_bundle in &self.rays_pos_history {
            let mut rays_pos_projection = Vec::<MatrixXx2<Length>>::with_capacity(ray_bundle.len());
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
            projected_history.push(rays_pos_projection);
        }

        Ok(projected_history)
    }
}

impl From<Vec<Rays>> for GhostFocusHistory {
    fn from(value: Vec<Rays>) -> Self {
        let mut ghost_focus_history = Vec::<Vec<MatrixXx3<Length>>>::with_capacity(value.len());
        for rays in &value {
            let mut rays_history = Vec::<MatrixXx3<Length>>::with_capacity(rays.nr_of_rays(false));
            for ray in rays {
                rays_history.push(ray.position_history());
            }
            ghost_focus_history.push(rays_history);
        }
        Self {
            rays_pos_history: ghost_focus_history,
            plot_view_direction: None,
        }
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
                for ray_pos in bounce_positions {
                    proj_pos_mm.push(MatrixXx2::from_vec(
                        ray_pos
                            .iter()
                            .map(uom::si::f64::Length::get::<millimeter>)
                            .collect::<Vec<f64>>(),
                    ));
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
                    RGBAColor(c.r, c.g, c.b, 1.),
                    Some(series_label),
                ));
            }

            Ok(Some(plt_series))
        }
    }
}
