use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    core_optics::optic_surface::OpticSurface,
    error::OpmResult,
    light::{Rays, lightdata::ray_data_builder::RayDataBuilder},
    nodes::NodeGroup,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Configuration for performing a ghost focus analysis
pub struct GhostFocusConfig {
    max_bounces: usize,
    fluence_estimator: FluenceEstimator,
    source_map: HashMap<Uuid, RayDataBuilder>,
}

impl GhostFocusConfig {
    /// Returns the max bounces of this [`GhostFocusConfig`].
    #[must_use]
    pub const fn max_bounces(&self) -> usize {
        self.max_bounces
    }
    /// Sets the maximum number of ray bounces to be considered during ghost focus analysis.
    pub const fn set_max_bounces(&mut self, max_bounces: usize) {
        self.max_bounces = max_bounces;
    }
    /// Returns the fluence estimator of this [`GhostFocusConfig`].
    #[must_use]
    pub const fn fluence_estimator(&self) -> &FluenceEstimator {
        &self.fluence_estimator
    }
    /// Sets the fluence estimator to be considered during ghost focus analysis.
    pub const fn set_fluence_estimator(&mut self, fluence_estimator: FluenceEstimator) {
        self.fluence_estimator = fluence_estimator;
    }
    /// Maps an ray data builder to the given source UUID
    ///
    /// If a builder was already mapped this function returns `true`. A new mapping
    /// reutrns `false`
    pub fn map_source(&mut self, node_id: Uuid, ray_data_builder: RayDataBuilder) -> bool {
        self.source_map.insert(node_id, ray_data_builder).is_some()
    }
    /// Returns the ray data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn get_source(&self, uuid: &Uuid) -> Option<&RayDataBuilder> {
        self.source_map.get(uuid)
    }
    /// Removes and returns the ray data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn remove_source(&mut self, uuid: &Uuid) -> Option<RayDataBuilder> {
        self.source_map.remove(uuid)
    }
    /// Removes all source mappings whose UUIDs no longer exist in the given model.
    pub fn prune_source_map(&mut self, model: &NodeGroup) {
        self.source_map.retain(|uuid, _builder| model.exists(*uuid));
    }
    /// Returns a reference to the source map of this [`GhostFocusConfig`].
    #[must_use]
    pub const fn source_map(&self) -> &HashMap<Uuid, RayDataBuilder> {
        &self.source_map
    }
}
impl Default for GhostFocusConfig {
    fn default() -> Self {
        Self {
            max_bounces: 1,
            fluence_estimator: FluenceEstimator::Voronoi,
            source_map: HashMap::new(),
        }
    }
}
impl PropagationStrategy for GhostFocusConfig {
    fn missed_surface_strategy(&self) -> MissedSurfaceStrategy {
        MissedSurfaceStrategy::Ignore
    }
    fn on_surface_interaction(
        &self,
        surf: &mut OpticSurface,
        rays: &mut Rays,
        reflected_rays: Rays,
        backward: bool,
    ) -> OpmResult<()> {
        // Ghost focus specific fluence evaluation and caching
        surf.evaluate_fluence_of_ray_bundle(rays, self.fluence_estimator())?;
        surf.add_to_rays_cache(reflected_rays, backward);
        Ok(())
    }
}
#[cfg(test)]
mod test_ghost_focus_config {
    use super::GhostFocusConfig;
    use crate::{
        core_optics::hit_map::fluence_estimator::FluenceEstimator,
        light::lightdata::ray_data_builder::RayDataBuilder, nodes::SourcePort,
    };
    #[test]
    fn default() {
        let c = GhostFocusConfig::default();
        assert_eq!(c.max_bounces(), 1);
        assert_eq!(c.fluence_estimator(), &FluenceEstimator::Voronoi);
    }
    #[test]
    fn set_max_bounces() {
        let mut c = GhostFocusConfig::default();
        c.set_max_bounces(10);
        assert_eq!(c.max_bounces(), 10);
    }
    #[test]
    fn set_fluence_estimator() {
        let mut c = GhostFocusConfig::default();
        c.set_fluence_estimator(FluenceEstimator::HelperRays);
        assert_eq!(c.fluence_estimator(), &FluenceEstimator::HelperRays);
    }
    #[test]
    fn test_map_and_get_source() {
        use crate::light::lightdata::ray_data_source::{CollimatedSrc, PointSrc, RayDataSource};
        use uuid::Uuid;
        let mut config = GhostFocusConfig::default();
        let uuid = Uuid::new_v4();
        let builder: RayDataBuilder = RayDataSource::Collimated(CollimatedSrc::default()).into();

        assert_eq!(config.map_source(uuid, builder.clone()), false);
        assert_eq!(config.get_source(&uuid), Some(&builder));

        let builder2: RayDataBuilder = RayDataSource::PointSrc(PointSrc::default()).into();
        assert_eq!(config.map_source(uuid, builder2.clone()), true);
        assert_eq!(config.get_source(&uuid), Some(&builder2));
    }

    #[test]
    fn test_remove_source() {
        use crate::light::lightdata::ray_data_source::{CollimatedSrc, RayDataSource};
        use uuid::Uuid;
        let mut config = GhostFocusConfig::default();
        let uuid = Uuid::new_v4();
        let builder: RayDataBuilder = RayDataSource::Collimated(CollimatedSrc::default()).into();

        config.map_source(uuid, builder.clone());
        assert_eq!(config.remove_source(&uuid), Some(builder));
        assert!(config.get_source(&uuid).is_none());
        assert!(config.remove_source(&uuid).is_none());
    }

    #[test]
    fn test_prune_source_map() {
        use crate::{
            light::lightdata::ray_data_source::{CollimatedSrc, RayDataSource},
            nodes::NodeGroup,
        };
        use uuid::Uuid;

        let mut config = GhostFocusConfig::default();
        let uuid2 = Uuid::new_v4();
        let builder: RayDataBuilder = RayDataSource::Collimated(CollimatedSrc::default()).into();

        let mut scene = NodeGroup::default();
        let node_id = scene.add_node(SourcePort::default()).unwrap();

        config.map_source(node_id, builder.clone());
        config.map_source(uuid2, builder.clone());

        config.prune_source_map(&scene);

        assert!(config.get_source(&node_id).is_some());
        assert!(config.get_source(&uuid2).is_none());
    }
}
