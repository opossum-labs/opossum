//! The runtime state of a placed component: its geometry, the optic surfaces traced at its faces and
//! the inversion of the medium it encloses.

use std::collections::BTreeMap;

use log::warn;
use uuid::Uuid;

use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    core_optics::{optic_surface::OpticSurface, surface_effect::SurfaceEffect},
    error::{OpmResult, OpossumError},
    gain::Inversion,
    geometry::{Geometry, body::Body},
    light::{Rays, rays::ReflectedPart},
};

/// The optic surfaces of a node, one per port name.
#[derive(Default, Debug, Clone)]
pub struct RuntimeSurfaces {
    /// Mapping of input port names to optical surfaces.
    pub inputs: BTreeMap<String, OpticSurface>,
    /// Mapping of output port names to optical surfaces.
    pub outputs: BTreeMap<String, OpticSurface>,
}

impl RuntimeSurfaces {
    /// Returns an iterator over all mutable optic surfaces (inputs and outputs).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.inputs.values_mut().chain(self.outputs.values_mut())
    }

    /// Returns an iterator over all optic surfaces (inputs and outputs) and their port names.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &OpticSurface)> {
        self.inputs.iter().chain(self.outputs.iter())
    }

    /// Returns the optic surface under the given port name, input or output.
    #[must_use]
    pub fn get(&self, surf_name: &str) -> Option<&OpticSurface> {
        self.inputs
            .get(surf_name)
            .or_else(|| self.outputs.get(surf_name))
    }

    /// Returns the optic surface under the given port name, input or output, mutably.
    pub fn get_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        self.inputs
            .get_mut(surf_name)
            .or_else(|| self.outputs.get_mut(surf_name))
    }

    /// Returns the optic surface under the given port name as [`RuntimeSurfaces::get_mut`] does.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such surface.
    fn named_mut(&mut self, surf_name: &str) -> OpmResult<&mut OpticSurface> {
        self.get_mut(surf_name)
            .ok_or_else(|| OpossumError::Analysis(format!("Cannot find surface: \"{surf_name}\"")))
    }
}

/// The geometry of a node as an analysis uses it: moved to where the node sits, together with the
/// optic surface of every port, each a face of it, and the inversion of the medium it encloses.
///
/// It is built by [`OpticNodeExt::install_geometry`](crate::core_optics::OpticNodeExt::install_geometry)
/// and never written to a file.
#[derive(Debug, Clone)]
pub struct RuntimeGeometry {
    geometry: Geometry,
    surfaces: RuntimeSurfaces,
    inversion: Option<Inversion>,
}

impl RuntimeGeometry {
    /// Creates the runtime state of a placed geometry with its optic surfaces and an empty
    /// inversion.
    ///
    /// # Arguments
    ///
    /// * `geometry` - the geometry, moved to where the node sits.
    /// * `surfaces` - the optic surfaces, each a face of `geometry`.
    ///
    /// # Errors
    ///
    /// This function returns an error if a surface sits on a face the geometry does not have.
    pub fn new(geometry: Geometry, surfaces: RuntimeSurfaces) -> OpmResult<Self> {
        let kind = geometry.kind();
        if let Some((name, surface)) = surfaces
            .iter()
            .find(|(_, surface)| kind.face(surface.face()).is_none())
        {
            return Err(OpossumError::Other(format!(
                "surface '{name}' sits on face {}, which does not exist (face count: {})",
                surface.face(),
                kind.face_count()
            )));
        }
        Ok(Self {
            geometry,
            surfaces,
            inversion: None,
        })
    }
    /// Returns the placed geometry.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }
    /// Returns the optic surfaces.
    #[must_use]
    pub const fn surfaces(&self) -> &RuntimeSurfaces {
        &self.surfaces
    }
    /// Returns the optic surface under the given port name, input or output, mutably.
    pub fn surface_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        self.surfaces.get_mut(surf_name)
    }
    /// Returns an iterator over all optic surfaces, mutably.
    pub fn surfaces_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.surfaces.values_mut()
    }
    /// Returns an optic surface together with the geometry it is a face of, which answers where a
    /// ray hits it.
    ///
    /// # Arguments
    ///
    /// * `surf_name` - the name of the surface (its port name).
    ///
    /// # Returns
    ///
    /// The geometry and the surface, or `None` if there is no such surface.
    pub fn traced_surface_mut(
        &mut self,
        surf_name: &str,
    ) -> Option<(&Geometry, &mut OpticSurface)> {
        Some((&self.geometry, self.surfaces.get_mut(surf_name)?))
    }
    /// Lets a surface of this component act on a bundle of rays.
    ///
    /// The geometry answers where each valid ray hits the face the surface is (see
    /// [`Geometry::intersect`]). A ray that misses it follows the missed surface strategy. At a hit,
    /// the effect acts on the ray (see [`SurfaceEffect::act_at`]) and, if the ray sends a part back,
    /// on its helper rays as well; the surface records the hit in its hit map. Which of the two parts
    /// is the intended beam is recorded on both (see [`ReflectedPart`]).
    ///
    /// # Arguments
    ///
    /// * `surface` - the name of the surface (its port name).
    /// * `rays` - the bundle. Its rays are moved to the surface and continue as the transmitted part.
    /// * `effect` - what the surface does to a ray that hits it.
    /// * `part` - what the reflected part is to the bundle.
    /// * `missed` - what happens to a ray that misses the surface.
    ///
    /// # Returns
    ///
    /// The bundle of reflected parts and, for every ray of `rays` in order, whether it hit the
    /// surface. A ray that was already invalid did not.
    ///
    /// # Warnings
    ///
    /// This function warns if a ray missed the surface or sent nothing back, and if the bundle has
    /// no valid ray.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such surface, if the effect fails for a ray or
    /// if a hit cannot be recorded.
    pub fn interact(
        &mut self,
        surface: &str,
        rays: &mut Rays,
        effect: &dyn SurfaceEffect,
        part: ReflectedPart,
        missed: MissedSurfaceStrategy,
    ) -> OpmResult<(Rays, Vec<bool>)> {
        let Self {
            geometry, surfaces, ..
        } = self;
        let surface = surfaces.named_mut(surface)?;
        let face = surface.face();
        let uuid = rays.uuid();
        let mut valid_rays_found = false;
        let mut rays_missed = false;
        let mut reflected_rays = Rays::with_capacity(rays.nr_of_rays(false));
        let mut hits = vec![false; rays.nr_of_rays(false)];
        for (ray, hit) in rays
            .iter_mut()
            .zip(hits.iter_mut())
            .filter(|(ray, _)| ray.valid())
        {
            valid_rays_found = true;
            let Some(surface_hit) = geometry.intersect(face, ray) else {
                missed.apply_to(ray);
                rays_missed = true;
                continue;
            };
            *hit = true;
            let (reflected, hit_point) = effect.act_at(ray, &surface_hit, surface.coating())?;
            if let Some(mut reflected) = reflected {
                if let (Some(helpers), Some(reflected_helpers)) =
                    (ray.helper_rays_mut(), reflected.helper_rays_mut())
                {
                    for (helper, reflected_helper) in
                        helpers.iter_mut().zip(reflected_helpers.iter_mut())
                    {
                        let Some(helper_hit) = geometry.intersect(face, helper) else {
                            missed.apply_to(helper);
                            continue;
                        };
                        if let (Some(helper_reflected), _) =
                            effect.act_at(helper, &helper_hit, surface.coating())?
                        {
                            reflected_helper.set_direction(helper_reflected.direction())?;
                        }
                    }
                }
                part.record_split(ray, &mut reflected);
                reflected_rays.add_ray(reflected);
            } else {
                rays_missed = true;
            }
            if let Some(hit_point) = hit_point {
                surface.add_to_hit_map(hit_point, ray.number_of_bounces(), uuid)?;
            }
        }
        if rays_missed {
            warn!("rays totally reflected or missed a surface");
        }
        if !valid_rays_found {
            warn!("ray bundle contains no valid rays - not propagating");
        }
        part.link_bundles(rays, &mut reflected_rays);
        Ok((reflected_rays, hits))
    }
    /// Lets a surface of this component act on the bundles of an analysis (see
    /// [`RuntimeGeometry::interact`]).
    ///
    /// After each bundle, the analysis does what it does at a surface (see
    /// [`PropagationStrategy::on_surface_interaction`]). Then the ghost reflections cached at the
    /// surface for this direction are appended to the bundles.
    ///
    /// # Arguments
    ///
    /// * `surface` - the name of the surface (its port name).
    /// * `bundles` - the bundles of the analysis.
    /// * `effect` - what the surface does to a ray that hits it.
    /// * `part` - what the reflected part is to each bundle.
    /// * `backward` - whether the light runs through the node backward.
    /// * `node` - the node of this component, which the reflected bundles originate from.
    /// * `strategy` - the analysis.
    ///
    /// # Returns
    ///
    /// For every bundle in `bundles` after the call, whether each of its rays hit the surface. The
    /// appended ghost reflections left the surface from a hit, so all of them count as hits.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such surface, if the interaction fails or if
    /// the analysis fails at the surface (e.g. in its fluence evaluation).
    #[allow(clippy::too_many_arguments)]
    pub fn propagate(
        &mut self,
        surface: &str,
        bundles: &mut Vec<Rays>,
        effect: &dyn SurfaceEffect,
        part: ReflectedPart,
        backward: bool,
        node: Uuid,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<Vec<Vec<bool>>> {
        let missed = strategy.missed_surface_strategy();
        let mut hits_per_bundle = Vec::with_capacity(bundles.len());
        for rays in &mut *bundles {
            let (mut reflected, hits) = self.interact(surface, rays, effect, part, missed)?;
            reflected.set_node_origin_uuid(node);
            strategy.on_surface_interaction(
                self.surfaces.named_mut(surface)?,
                rays,
                reflected,
                backward,
            )?;
            hits_per_bundle.push(hits);
        }
        for rays in self.surfaces.named_mut(surface)?.get_rays_cache(backward) {
            hits_per_bundle.push(vec![true; rays.nr_of_rays(false)]);
            bundles.push(rays.clone());
        }
        Ok(hits_per_bundle)
    }
    /// Returns the medium: the body the geometry encloses and the inversion in it.
    ///
    /// [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume) fills the
    /// inversion for an analysis run, and
    /// [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data) clears it again.
    ///
    /// # Returns
    ///
    /// The body and the inversion, if a model built one; or `None` for a single surface, which
    /// encloses no volume.
    #[must_use]
    pub fn medium(&self) -> Option<(&dyn Body, Option<&Inversion>)> {
        Some((self.geometry.kind().as_body()?, self.inversion.as_ref()))
    }
    /// Returns the medium as [`RuntimeGeometry::medium`] does, with the inversion to be built or
    /// depleted.
    pub fn medium_mut(&mut self) -> Option<(&dyn Body, &mut Option<Inversion>)> {
        Some((self.geometry.kind().as_body()?, &mut self.inversion))
    }
    /// Clears the inversion of the medium between analysis runs.
    pub fn clear_inversion(&mut self) {
        self.inversion = None;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        J_per_cm2,
        analyzers::RayTraceConfig,
        apertures::{CircleShape, Outline},
        coatings::{CoatingConstantR, CoatingType},
        core_optics::surface_effect::Refraction,
        degree,
        distributions::{fluence::general_gaussian::General2DGaussian, position::Hexapolar},
        geometry::{Extruded, Face, SurfaceShape},
        joule,
        light::{Ray, Spectrum},
        millimeter, nanometer,
        nodes::{
            SplittingConfig,
            ideal_filter::{EdgeFilter, EdgeFilterType},
            test_helper::helper::traced_at,
        },
        percent, radian,
        refractive_index::refr_index_vaccuum,
        utils::{geom_transformation::Isometry, test_helper::helper::check_logs},
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::{Vector3, vector};
    use uom::si::{
        energy::joule,
        f64::{Energy, Length},
    };

    /// A disk 10 mm thick and 5 mm in radius.
    fn test_body() -> OpmResult<Geometry> {
        Geometry::singlet(
            millimeter!(f64::INFINITY),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?,
        )
    }
    /// The surfaces of a disk: `input_1` on the front face and `output_1` on the given face.
    fn disk_surfaces(output_face: usize) -> OpmResult<RuntimeSurfaces> {
        let surface = |face| OpticSurface::new(face, CoatingType::IdealAR, J_per_cm2!(1.0));
        let mut surfaces = RuntimeSurfaces::default();
        surfaces
            .inputs
            .insert("input_1".into(), surface(Extruded::FRONT)?);
        surfaces
            .outputs
            .insert("output_1".into(), surface(output_face)?);
        Ok(surfaces)
    }
    #[test]
    fn a_single_surface_has_no_medium() -> OpmResult<()> {
        assert!(
            RuntimeGeometry::new(test_body()?, RuntimeSurfaces::default())?
                .medium()
                .is_some()
        );
        assert!(
            RuntimeGeometry::new(Geometry::plane(None), RuntimeSurfaces::default())?
                .medium()
                .is_none()
        );
        Ok(())
    }
    /// Every surface is a face of the geometry: a disk has two faces, so there is no face 2.
    #[test]
    fn a_surface_on_a_missing_face_is_rejected() -> OpmResult<()> {
        assert!(RuntimeGeometry::new(test_body()?, disk_surfaces(Extruded::REAR)?).is_ok());
        let error = RuntimeGeometry::new(test_body()?, disk_surfaces(2)?)
            .expect_err("a surface on face 2 of a disk was accepted");
        assert!(
            error
                .to_string()
                .contains("surface 'output_1' sits on face 2"),
            "{error}"
        );
        Ok(())
    }
    /// The medium starts without an inversion; building one fills it, and clearing empties it.
    #[test]
    fn set_and_clear_the_inversion() -> OpmResult<()> {
        let mut runtime = RuntimeGeometry::new(test_body()?, RuntimeSurfaces::default())?;
        let has_inversion = |runtime: &RuntimeGeometry| {
            runtime
                .medium()
                .is_some_and(|(_, inversion)| inversion.is_some())
        };
        assert!(!has_inversion(&runtime));
        let (_, inversion) = runtime
            .medium_mut()
            .ok_or_else(|| OpossumError::Other("the disk has no medium".into()))?;
        *inversion = Some(Inversion::Uniform(1.0));
        assert!(has_inversion(&runtime));
        runtime.clear_inversion();
        assert!(!has_inversion(&runtime));
        Ok(())
    }
    /// An unbounded plane through the origin as the surface `input_1`.
    fn open_plane(surface: OpticSurface) -> OpmResult<RuntimeGeometry> {
        traced_at(Geometry::plane(None), surface)
    }
    /// Refract a bundle into vacuum at the given component, with its ghost reflection.
    fn refract(runtime: &mut RuntimeGeometry, rays: &mut Rays) -> OpmResult<(Rays, Vec<bool>)> {
        runtime.interact(
            "input_1",
            rays,
            &Refraction(Some(&refr_index_vaccuum())),
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Stop,
        )
    }
    /// Split a bundle at the given component with a beam splitter of the given configuration.
    fn split(
        runtime: &mut RuntimeGeometry,
        rays: &mut Rays,
        config: &SplittingConfig,
    ) -> OpmResult<Rays> {
        runtime
            .interact(
                "input_1",
                rays,
                config,
                ReflectedPart::Branch,
                MissedSurfaceStrategy::Stop,
            )
            .map(|(reflected, _)| reflected)
    }
    #[test]
    fn refraction_of_an_empty_bundle() -> OpmResult<()> {
        let mut rays = Rays::default();
        testing_logger::setup();
        let (reflected, _) = refract(&mut open_plane(OpticSurface::default())?, &mut rays)?;
        check_logs(
            log::Level::Warn,
            vec!["ray bundle contains no valid rays - not propagating"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    #[test]
    fn refraction_of_helper_rays() -> OpmResult<()> {
        let tot_energy = joule!(1.);
        let pos_strategy = Hexapolar::new(millimeter!(15.), 5)?;
        let fluence_strategy = General2DGaussian::new(
            tot_energy,
            millimeter!(0., 0.),
            millimeter!(2.5, 2.5),
            radian!(0.),
        )?;
        let mut rays = Rays::new_collimated_w_fluence_helper(
            nanometer!(1000.),
            &fluence_strategy,
            &pos_strategy,
        )?;
        refract(&mut open_plane(OpticSurface::default())?, &mut rays)?;
        Ok(())
    }
    /// The intended beam of a reflection continues the incoming bundle.
    #[test]
    fn a_main_reflection_continues_the_bundle() -> OpmResult<()> {
        let mut rays = Rays::default();
        let node_uuid = Uuid::new_v4();
        let parent_uuid = Uuid::new_v4();
        rays.set_node_origin_uuid(node_uuid);
        rays.set_parent_uuid(parent_uuid);
        rays.set_parent_node_split_idx(10);
        let (reflected, _) = open_plane(OpticSurface::default())?.interact(
            "input_1",
            &mut rays,
            &Refraction(Some(&refr_index_vaccuum())),
            ReflectedPart::Main,
            MissedSurfaceStrategy::Stop,
        )?;
        assert_eq!(*reflected.parent_pos_split_idx(), 10);
        assert_eq!(reflected.node_origin(), &Some(node_uuid));
        assert_eq!(reflected.parent_id(), Some(parent_uuid));
        assert_eq!(reflected.uuid(), rays.uuid());
        Ok(())
    }
    /// A ghost reflection branches off the incoming bundle where the rays hit the surface: it is a
    /// bounce of its own and starts its position history there.
    #[test]
    fn a_ghost_reflection_branches_off_the_bundle() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut surface = OpticSurface::default();
        surface.set_coating(CoatingConstantR::new(percent!(20.0))?.into());
        let (reflected, _) = refract(&mut open_plane(surface)?, &mut rays)?;
        assert_eq!(reflected.parent_id(), Some(rays.uuid()));
        assert_eq!(*reflected.parent_pos_split_idx(), rays.ray_history_len());
        assert_eq!(reflected.bounce_lvl(), rays.bounce_lvl() + 1);
        assert_eq!(reflected.ray_history_len(), 0);
        Ok(())
    }
    #[test]
    fn an_unknown_surface_is_an_error() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let error = open_plane(OpticSurface::default())?
            .interact(
                "output_1",
                &mut rays,
                &Refraction(None),
                ReflectedPart::Ghost,
                MissedSurfaceStrategy::Stop,
            )
            .expect_err("a surface that does not exist was traced");
        assert!(
            error
                .to_string()
                .contains("Cannot find surface: \"output_1\""),
            "{error}"
        );
        Ok(())
    }
    /// A passive surface keeps the refractive index of each ray, and so its direction.
    #[test]
    fn a_passive_refraction_keeps_the_direction() -> OpmResult<()> {
        let mut rays = Rays::default();
        let mut ray0 = Ray::new(
            millimeter!(0., 0., -10.),
            vector![0.0, 1.0, 1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        ray0.set_refractive_index(1.5)?;
        let mut ray1 = Ray::new(
            millimeter!(0., 1., -10.),
            vector![0.0, 1.0, 1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        ray1.set_refractive_index(2.0)?;
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        open_plane(OpticSurface::default())?.interact(
            "input_1",
            &mut rays,
            &Refraction(None),
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Stop,
        )?;
        for ray in rays.iter() {
            assert_abs_diff_eq!(ray.direction(), vector![0.0, 1.0, 1.0].normalize());
        }
        Ok(())
    }
    #[test]
    fn refraction_missed() -> OpmResult<()> {
        let mut rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        testing_logger::setup();
        let (reflected, _) = refract(&mut open_plane(OpticSurface::default())?, &mut rays)?;
        check_logs(
            log::Level::Warn,
            vec!["rays totally reflected or missed a surface"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    #[test]
    fn refraction_energy() -> OpmResult<()> {
        let mut rays = Rays::default();
        rays.add_ray(Ray::new_collimated(
            millimeter!(0.0, 0.0, -1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let mut s = OpticSurface::default();
        s.set_coating(CoatingConstantR::new(percent!(20.0))?.into());
        let (reflected, _) = refract(&mut open_plane(s)?, &mut rays)?;
        assert_eq!(rays.total_energy(), joule!(0.8));
        assert_eq!(reflected.total_energy(), joule!(0.2));
        Ok(())
    }
    /// A flat surface through the origin, tilted by 45° about the y axis, as the surface
    /// `input_1`.
    fn tilted_splitting_plane(surface: OpticSurface) -> OpmResult<RuntimeGeometry> {
        let iso = Isometry::new(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        traced_at(Geometry::from(Face::new(SurfaceShape::Plane, iso)), surface)
    }
    fn collimated_ray_along_z(wavelength: Length) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0., 0., -10.), wavelength, joule!(1.0))
    }
    fn energies(rays: &Rays) -> Vec<Energy> {
        rays.iter().map(Ray::energy).collect()
    }
    #[test]
    fn splitting_tilted() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut plane = tilted_splitting_plane(OpticSurface::default())?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(reflected.total_energy().get::<joule>(), 0.4);
        let transmitted_ray = rays.iter().next().expect("a transmitted ray");
        let reflected_ray = reflected.iter().next().expect("a reflected ray");
        assert_abs_diff_eq!(transmitted_ray.direction(), Vector3::z(), epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().x.abs(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().z, 0.0, epsilon = 1e-12);
        assert_eq!(reflected_ray.position(), transmitted_ray.position());
        Ok(())
    }
    #[test]
    fn splitting_ignores_the_surface_coating() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut surface = OpticSurface::default();
        surface.set_coating(CoatingConstantR::new(percent!(20.0))?.into());
        let mut plane = tilted_splitting_plane(surface)?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(reflected.total_energy().get::<joule>(), 0.4);
        Ok(())
    }
    #[test]
    fn splitting_keeps_both_parts_intended() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut plane = tilted_splitting_plane(OpticSurface::default())?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        // the reflected part is an intended beam, not a ghost reflection
        assert_eq!(reflected.bounce_lvl(), rays.bounce_lvl());
        // neither part loses the position history before the splitting surface
        assert_eq!(rays.ray_history_len(), 1);
        assert_eq!(reflected.ray_history_len(), 1);
        assert_ne!(reflected.uuid(), rays.uuid());
        assert!(reflected.parent_id().is_none());
        Ok(())
    }
    #[test]
    fn splitting_by_spectrum() -> OpmResult<()> {
        let spectrum: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            nanometer!(1000.0),
            0.0..1.0,
            None,
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
        )?
        .into();
        let config = SplittingConfig::Spectrum(spectrum);
        let mut rays = Rays::default();
        rays.add_ray(collimated_ray_along_z(nanometer!(999.0))?);
        rays.add_ray(collimated_ray_along_z(nanometer!(1001.0))?);
        let mut plane = tilted_splitting_plane(OpticSurface::default())?;
        let reflected = split(&mut plane, &mut rays, &config)?;
        assert_eq!(energies(&rays), [joule!(1.0), joule!(0.0)]);
        assert_eq!(energies(&reflected), [joule!(0.0), joule!(1.0)]);

        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1501.0))?);
        assert!(split(&mut plane, &mut rays, &config).is_err());
        Ok(())
    }
    #[test]
    fn splitting_missed() -> OpmResult<()> {
        let mut rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        testing_logger::setup();
        let reflected = split(
            &mut open_plane(OpticSurface::default())?,
            &mut rays,
            &SplittingConfig::Ratio(0.6),
        )?;
        check_logs(
            log::Level::Warn,
            vec!["rays totally reflected or missed a surface"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    /// A plane through the node, its component reaching 5 mm around the node's axis.
    fn bounded_plane() -> OpmResult<Geometry> {
        Ok(Geometry::plane(Some(Outline::new(
            CircleShape::new(millimeter!(5.0))?,
            Vec::new(),
        )?)))
    }
    /// A ray along z, the given distance off the axis in y.
    fn ray_at(y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    /// Propagate bundles forward through the passive surface `input_1` of a component.
    fn propagate(
        runtime: &mut RuntimeGeometry,
        bundles: &mut Vec<Rays>,
        config: &RayTraceConfig,
    ) -> OpmResult<Vec<Vec<bool>>> {
        runtime.propagate(
            "input_1",
            bundles,
            &Refraction(None),
            ReflectedPart::Ghost,
            false,
            Uuid::new_v4(),
            config,
        )
    }
    /// The surface pass tells for each ray of each bundle whether it hit. Ghost reflections cached
    /// at the surface left it from a hit, so when they are appended they count as hits.
    #[test]
    fn propagation_tells_which_rays_hit() -> OpmResult<()> {
        let mut surface = OpticSurface::default();
        surface.set_forward_rays_cache(vec![Rays::from(vec![ray_at(0.0)?, ray_at(7.0)?])]);
        let mut bundles = vec![Rays::from(vec![ray_at(4.9)?, ray_at(5.1)?])];
        let hits = propagate(
            &mut traced_at(bounded_plane()?, surface)?,
            &mut bundles,
            &RayTraceConfig::default(),
        )?;
        assert_eq!(hits, [vec![true, false], vec![true, true]]);
        assert_eq!(bundles.len(), 2);
        Ok(())
    }
    /// A ray trace invalidates the rays the surface pass leaves below its energy threshold. The
    /// positioning run keeps them: geometry alone places the optical axis.
    #[test]
    fn a_ray_trace_drops_rays_below_the_energy_threshold() -> OpmResult<()> {
        let mut config = RayTraceConfig::default();
        config.set_min_energy_per_ray(joule!(0.5))?;
        for (config, valid) in [(config.for_positioning(), 2), (config, 1)] {
            let weak = Ray::new_collimated(
                millimeter!(0.0, 0.0, -10.0),
                nanometer!(1000.0),
                joule!(0.1),
            )?;
            let mut bundles = vec![Rays::from(vec![ray_at(0.0)?, weak])];
            propagate(
                &mut traced_at(bounded_plane()?, OpticSurface::default())?,
                &mut bundles,
                &config,
            )?;
            assert_eq!(bundles[0].nr_of_rays(true), valid);
        }
        Ok(())
    }
    /// A surface is traced at the face of the geometry it names: the rear face of a plate here, so
    /// a ray along the axis meets it 10 mm behind the front.
    #[test]
    fn a_surface_is_traced_at_the_face_it_names() -> OpmResult<()> {
        let surface = OpticSurface::new(Extruded::REAR, CoatingType::IdealAR, J_per_cm2!(1.0))?;
        let mut bundles = vec![Rays::from(ray_at(0.0)?)];
        propagate(
            &mut traced_at(test_body()?, surface)?,
            &mut bundles,
            &RayTraceConfig::default(),
        )?;
        let ray = bundles[0]
            .iter()
            .next()
            .ok_or_else(|| OpossumError::Other("the bundle lost its ray".into()))?;
        assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
        Ok(())
    }
}
