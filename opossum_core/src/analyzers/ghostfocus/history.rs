use std::collections::{HashMap, hash_map::Values};

use nalgebra::{MatrixXx2, MatrixXx3, Vector3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uom::si::length::millimeter;
use uuid::Uuid;

use crate::{
    error::{OpmResult, OpossumError},
    light::Rays,
    millimeter,
    nodes::OpticGraph,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    prelude::Proptype,
    utils::LockExt,
};

/// Struct to store the node origin uuid and parent ray bundle Uuid of a ray bundle
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
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
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct RaysNodeCorrelation {
    correlation: HashMap<Uuid, RaysOrigin>,
}
impl RaysNodeCorrelation {
    ///creates a new [`RaysNodeCorrelation`]
    #[must_use]
    pub fn new(rays_uuid: Uuid, rays_origin: &RaysOrigin) -> Self {
        let mut correlation = HashMap::<Uuid, RaysOrigin>::new();
        correlation.insert(rays_uuid, rays_origin.clone());
        Self { correlation }
    }
    /// inserts a key value pair in the correlation hashmap
    pub fn insert(&mut self, k: Uuid, v: &RaysOrigin) {
        self.correlation.insert(k, v.clone());
    }
    /// returns the values of the correlation hashmap
    #[must_use]
    pub fn values(&self) -> Values<'_, Uuid, RaysOrigin> {
        self.correlation.values()
    }
}
/// struct that holds the history of the ray positions that is needed for report generation
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GhostFocusHistory {
    /// vector of ray positions for each raybundle at a specifc spectral position
    pub rays_pos_history: Vec<Vec<Vec<MatrixXx3<Length>>>>,
    /// view direction if the ray position history is plotted
    pub plot_view_direction: Option<Vector3<f64>>,
    /// stores the corrleation between a rays bundle and its parent node as well as parent
    /// ray bundle for each bounce in a vector
    pub ray_node_correlation: Vec<RaysNodeCorrelation>,
}
impl GhostFocusHistory {
    /// Projects the positions of the [`GhostFocusHistory`] onto a 2D plane
    ///
    /// # Attributes
    /// `plane_normal_vec`: normal vector of the plane to project onto
    ///
    /// # Errors
    /// This function errors if the length of the plane normal vector is zero
    /// # Returns
    /// This function returns a set of 2d vectors in the defined plane projected to a view that is perpendicular to this plane.
    fn project_to_plane(
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

        // define an axis on the plane.
        // Do this by projection of one of the main coordinate axes onto that plane
        // Beforehand check, if these axes are not parallel to the normal vec
        let (co_ax_1, co_ax_2) = if plane_normal_vec.cross(&Vector3::x()).norm() < f64::EPSILON {
            //parallel to the x-axis
            (Vector3::z(), Vector3::y())
        } else if plane_normal_vec.cross(&Vector3::y()).norm() < f64::EPSILON {
            (Vector3::z(), Vector3::x())
        } else if plane_normal_vec.cross(&Vector3::z()).norm() < f64::EPSILON {
            (Vector3::x(), Vector3::y())
        } else {
            // arbitrarily project x-axis onto that plane
            let x_vec = Vector3::x();
            let mut proj_x = x_vec - x_vec.dot(&normed_normal_vec) * plane_normal_vec;
            proj_x /= proj_x.norm();

            // second axis defined by cross product of x-axis projection and plane normal,
            // which yields another vector that is perpendicular to both others.
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
        rays_uuid: Uuid,
        hist_idx: usize,
    ) {
        for (bounce, ray_vecs_in_bounce) in accumulated_rays.iter().enumerate() {
            if ray_vecs_in_bounce.contains_key(&rays_uuid) {
                let mut rays_per_bounce_history =
                    Vec::<Vec<MatrixXx3<Length>>>::with_capacity(ray_vecs_in_bounce.len());
                if let Some(rays) = ray_vecs_in_bounce.get(&rays_uuid) {
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
                        &RaysOrigin::new(rays.parent_id(), *rays.node_origin()),
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

    /// Returns the report string for the critical ray origin in the ghost focus analysis
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if locking of an internal mutex fails.
    #[must_use]
    pub fn rays_origin_report_str(&self, graph: &OpticGraph) -> String {
        let mut report_str = String::new();
        for (bounce, rays_correlation) in self.ray_node_correlation.iter().enumerate() {
            for rays_origin in rays_correlation.values() {
                if let Some(node_uuid) = rays_origin.node_origin {
                    if bounce != 0 {
                        report_str += format!("bounce {bounce} at node '").as_str();
                    }
                    if let Ok(opt_ref) = graph.node(node_uuid) {
                        report_str +=
                            format!("{}', ", opt_ref.optical_ref.lock_opm().unwrap().name())
                                .as_str();
                    }
                }
            }
        }
        report_str
    }
}
impl From<GhostFocusHistory> for Proptype {
    fn from(value: GhostFocusHistory) -> Self {
        Self::GhostFocusHistory(value)
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
                    rays_history.push(ray.position_history_with_current());
                }
                ray_node_bounce_correlation.insert(
                    rays.uuid(),
                    &RaysOrigin::new(rays.parent_id(), *rays.node_origin()),
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
    /// value contains :
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
        ghost_focus_history.add_specific_ray_history(acc_rays, rays_uuid, hist_idx);

        ghost_focus_history
    }
}
impl Plottable for GhostFocusHistory {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("position in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("position in mm (y axis)".into()))?
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
                    RGBAColor(c.r, c.g, c.b, 0.2),
                    Some(series_label),
                ));
            }
            Ok(Some(plt_series))
        }
    }
}
#[cfg(test)]
mod test_rays_origin {
    use super::RaysOrigin;
    use uuid::Uuid;
    #[test]
    fn new() {
        let parent_rays_uuid = Uuid::new_v4();
        let node_origin_uuid = Uuid::new_v4();
        let ro = RaysOrigin::new(Some(parent_rays_uuid), Some(node_origin_uuid));
        assert_eq!(ro.parent_rays.unwrap(), parent_rays_uuid);
        assert_eq!(ro.node_origin.unwrap(), node_origin_uuid);
    }
}

#[cfg(test)]
mod test_rays_node_correlation {
    use super::*;
    use uuid::Uuid;
    #[test]
    fn new() {
        let parent_rays_uuid = Uuid::new_v4();
        let node_origin_uuid = Uuid::new_v4();
        let rays_uuid = Uuid::new_v4();
        let ro = RaysOrigin::new(Some(parent_rays_uuid), Some(node_origin_uuid));
        let rnc = RaysNodeCorrelation::new(rays_uuid, &ro);
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid)
                .unwrap()
                .parent_rays
                .unwrap(),
            parent_rays_uuid
        );
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid)
                .unwrap()
                .node_origin
                .unwrap(),
            node_origin_uuid
        );
    }
    #[test]
    fn insert() {
        let parent_rays_uuid = Uuid::new_v4();
        let node_origin_uuid = Uuid::new_v4();
        let rays_uuid = Uuid::new_v4();
        let parent_rays_uuid2 = Uuid::new_v4();
        let node_origin_uuid2 = Uuid::new_v4();
        let rays_uuid2 = Uuid::new_v4();
        let ro = RaysOrigin::new(Some(parent_rays_uuid), Some(node_origin_uuid));
        let ro2 = RaysOrigin::new(Some(parent_rays_uuid2), Some(node_origin_uuid2));
        let mut rnc = RaysNodeCorrelation::new(rays_uuid, &ro);
        rnc.insert(rays_uuid2, &ro2);
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid2)
                .unwrap()
                .parent_rays
                .unwrap(),
            parent_rays_uuid2
        );
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid2)
                .unwrap()
                .node_origin
                .unwrap(),
            node_origin_uuid2
        );
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid)
                .unwrap()
                .parent_rays
                .unwrap(),
            parent_rays_uuid
        );
        assert_eq!(
            rnc.correlation
                .get(&rays_uuid)
                .unwrap()
                .node_origin
                .unwrap(),
            node_origin_uuid
        );

        assert_eq!(rnc.values().len(), 2);
    }
}

#[cfg(test)]
mod test_rays_ghost_focus_history {
    use super::*;
    use crate::{distributions::position::Grid, joule, light::Rays, millimeter, nanometer};
    use approx::assert_relative_eq;
    use nalgebra::{MatrixXx3, Vector3, point};
    use std::collections::HashMap;
    use uom::si::f64::Length;
    use uuid::Uuid;

    #[test]
    fn from_vec_hashmap_uuid_tuple() {
        let mut accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
        let rays1 = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Grid::new(millimeter!(10.0, 10.0), point![5, 4]).unwrap(),
        )
        .unwrap();
        let mut hash1 = HashMap::<Uuid, Rays>::new();

        hash1.insert(rays1.uuid(), rays1.clone());

        accumulated_rays.push(hash1);

        let hist = GhostFocusHistory::from((&accumulated_rays, rays1.uuid(), 0));

        assert_eq!(hist.rays_pos_history.len(), 1);
        assert_eq!(hist.rays_pos_history[0].len(), 1);
        assert_eq!(hist.rays_pos_history[0][0].len(), 20);
        for (i, pos) in hist.rays_pos_history[0][0][0].row_iter().enumerate() {
            assert_relative_eq!(
                pos[0].value,
                rays1.get_ray_by_idx(i).unwrap().position().x.value
            );
            assert_relative_eq!(
                pos[1].value,
                rays1.get_ray_by_idx(i).unwrap().position().y.value
            );
            assert_relative_eq!(
                pos[2].value,
                rays1.get_ray_by_idx(i).unwrap().position().z.value
            );
        }
    }
    #[test]
    fn from_vec_accumulated_rays() {
        let mut accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
        let rays1 = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Grid::new(millimeter!(10.0, 10.0), point![5, 4]).unwrap(),
        )
        .unwrap();
        let mut hash1 = HashMap::<Uuid, Rays>::new();

        hash1.insert(rays1.uuid(), rays1.clone());

        accumulated_rays.push(hash1);

        let hist = GhostFocusHistory::from(accumulated_rays);

        assert_eq!(hist.rays_pos_history.len(), 1);
        assert_eq!(hist.rays_pos_history[0].len(), 1);
        assert_eq!(hist.rays_pos_history[0][0].len(), 20);
        for (i, pos) in hist.rays_pos_history[0][0][0].row_iter().enumerate() {
            assert_relative_eq!(
                pos[0].value,
                rays1.get_ray_by_idx(i).unwrap().position().x.value
            );
            assert_relative_eq!(
                pos[1].value,
                rays1.get_ray_by_idx(i).unwrap().position().y.value
            );
            assert_relative_eq!(
                pos[2].value,
                rays1.get_ray_by_idx(i).unwrap().position().z.value
            );
        }
    }

    #[test]
    fn project_to_plane() {
        let mut accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
        let rays1 = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Grid::new(millimeter!(10.0, 10.0), point![5, 4]).unwrap(),
        )
        .unwrap();
        let rays2 = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Grid::new(millimeter!(10.0, 10.0), point![5, 4]).unwrap(),
        )
        .unwrap();
        let mut hash1 = HashMap::<Uuid, Rays>::new();
        let mut hash2 = HashMap::<Uuid, Rays>::new();

        hash1.insert(rays1.uuid(), rays1.clone());
        hash2.insert(rays2.uuid(), rays2.clone());

        accumulated_rays.push(hash1);
        accumulated_rays.push(hash2);

        let hist = GhostFocusHistory::from(accumulated_rays);

        let projected = hist.project_to_plane(Vector3::x()).unwrap();

        for (i, bounced_rays) in projected.iter().enumerate() {
            for rays in bounced_rays {
                for (ray_idx, ray) in rays.iter().enumerate() {
                    for pos in ray.row_iter() {
                        if i == 0 {
                            assert_relative_eq!(
                                pos[0].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().z.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().y.value
                            );
                        } else {
                            assert_relative_eq!(
                                pos[0].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().z.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().y.value
                            );
                        }
                    }
                }
            }
        }
        let projected = hist.project_to_plane(Vector3::y()).unwrap();

        for (i, bounced_rays) in projected.iter().enumerate() {
            for rays in bounced_rays {
                for (ray_idx, ray) in rays.iter().enumerate() {
                    for pos in ray.row_iter() {
                        if i == 0 {
                            assert_relative_eq!(
                                pos[0].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().z.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().x.value
                            );
                        } else {
                            assert_relative_eq!(
                                pos[0].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().z.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().x.value
                            );
                        }
                    }
                }
            }
        }
        let projected = hist.project_to_plane(Vector3::z()).unwrap();

        for (i, bounced_rays) in projected.iter().enumerate() {
            for rays in bounced_rays {
                for (ray_idx, ray) in rays.iter().enumerate() {
                    for pos in ray.row_iter() {
                        if i == 0 {
                            assert_relative_eq!(
                                pos[0].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().x.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays1.get_ray_by_idx(ray_idx).unwrap().position().y.value
                            );
                        } else {
                            assert_relative_eq!(
                                pos[0].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().x.value
                            );
                            assert_relative_eq!(
                                pos[1].value,
                                rays2.get_ray_by_idx(ray_idx).unwrap().position().y.value
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn add_specific_ray_history() {
        let mut accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
        let rays1 = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Grid::new(millimeter!(10.0, 10.0), point![5, 4]).unwrap(),
        )
        .unwrap();
        let mut hash1 = HashMap::<Uuid, Rays>::new();

        hash1.insert(rays1.uuid(), rays1.clone());
        accumulated_rays.push(hash1);

        let mut ray_pos_history =
            Vec::<Vec<Vec<MatrixXx3<Length>>>>::with_capacity(accumulated_rays.len());
        let mut ray_node_correlation =
            Vec::<RaysNodeCorrelation>::with_capacity(accumulated_rays.len());
        for _i in 0..accumulated_rays.len() {
            ray_pos_history.push(Vec::<Vec<MatrixXx3<Length>>>::new());
            ray_node_correlation.push(RaysNodeCorrelation::default());
        }

        let mut hist = GhostFocusHistory {
            rays_pos_history: ray_pos_history,
            plot_view_direction: None,
            ray_node_correlation,
        };

        hist.add_specific_ray_history(&accumulated_rays, rays1.uuid(), 0);

        assert_eq!(hist.rays_pos_history.len(), 1);
        assert_eq!(hist.rays_pos_history[0].len(), 1);
        assert_eq!(hist.rays_pos_history[0][0].len(), 20);
        for (i, pos) in hist.rays_pos_history[0][0][0].row_iter().enumerate() {
            assert_relative_eq!(
                pos[0].value,
                rays1.get_ray_by_idx(i).unwrap().position().x.value
            );
            assert_relative_eq!(
                pos[1].value,
                rays1.get_ray_by_idx(i).unwrap().position().y.value
            );
            assert_relative_eq!(
                pos[2].value,
                rays1.get_ray_by_idx(i).unwrap().position().z.value
            );
        }
    }

    #[test]
    fn from_ghost_focus_history() {
        assert!(matches!(
            GhostFocusHistory::default().into(),
            Proptype::GhostFocusHistory(_)
        ));
    }
}
