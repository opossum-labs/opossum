//! What happens to a ray where it hits an optic surface.
//!
//! Where a ray hits a component is answered by its geometry (see
//! [`Geometry::intersect`](crate::geometry::Geometry::intersect)); what the surface then does to the
//! ray is a [`SurfaceEffect`]. The component brings both together (see
//! [`RuntimeGeometry::interact`](crate::core_optics::runtime_geometry::RuntimeGeometry::interact)).

use crate::{
    coatings::CoatingType, core_optics::hit_map::rays_hit_map::HitPoint, error::OpmResult,
    geometry::SurfaceHit, light::Ray, refractive_index::RefractiveIndexType,
};

/// What a surface does to a ray at the point where the ray hits it.
pub trait SurfaceEffect {
    /// Acts on a ray at its hit on a surface.
    ///
    /// The ray is moved to the hit and continues from there as the transmitted part. A part the
    /// surface sends back is returned as a new ray.
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray hitting the surface.
    /// * `hit` - where the ray meets the surface.
    /// * `coating` - the coating of the surface.
    ///
    /// # Returns
    ///
    /// The part the surface sends back, if any, and the point the surface records, if any.
    ///
    /// # Errors
    ///
    /// This function returns an error if the physics at the hit cannot be evaluated for the ray, for
    /// instance if a refractive index or a coating is not defined at its wavelength.
    fn act_at(
        &self,
        ray: &mut Ray,
        hit: &SurfaceHit,
        coating: &CoatingType,
    ) -> OpmResult<(Option<Ray>, Option<HitPoint>)>;
}

/// Refraction by Snell's law, the coating of the surface splitting off a reflected part (see
/// [`Ray::refract_at`]).
///
/// It holds the refractive index behind the surface. `None` keeps the refractive index of each ray:
/// a passive surface, which does not change the direction of the transmitted part.
pub struct Refraction<'a>(pub Option<&'a RefractiveIndexType>);

impl SurfaceEffect for Refraction<'_> {
    fn act_at(
        &self,
        ray: &mut Ray,
        hit: &SurfaceHit,
        coating: &CoatingType,
    ) -> OpmResult<(Option<Ray>, Option<HitPoint>)> {
        let n2 = self
            .0
            .map(|index| index.get_refractive_index(ray.wavelength()))
            .transpose()?;
        ray.refract_at(hit, coating, n2)
    }
}
