#![warn(missing_docs)]
use super::{Coating, CoatingType};
use crate::light::Ray;
use nalgebra::Vector3;
use uom::si::f64::Ratio;

/// Simulation of a Fresnel reflection (e.g. uncaoted surface)
///
/// This coating model simulates the Fresnel reflection of an (uncoated) surface. The reflectivity thereby depends on
/// the angle of incidence and the refractive index of the following medium.
/// For further information check the corresponding [Wikipedia article](https://en.wikipedia.org/wiki/Fresnel_equations).
/// Currently, an (50/50) unpolarized beam is assumed.
pub struct Fresnel;

impl Coating for Fresnel {
    /// Formulas taken from [german wikipedia](https://de.wikipedia.org/wiki/Fresnelsche_Formeln).
    fn calc_reflectivity(
        &self,
        incoming_ray: &Ray,
        surface_normal: Vector3<f64>,
        n2: f64,
    ) -> Ratio {
        let n1 = incoming_ray.refractive_index();

        // Fix: Explicitly normalize the direction to ensure the dot product
        // represents the actual cosine of the angle.
        let direction = incoming_ray.direction().normalize();

        // The cosine of the angle of incidence.
        // We use .abs() to handle rays hitting from "behind" if necessary,
        // and clamp to avoid precision issues with sqrt later.
        let cos_alpha = direction
            .dot(&(-1.0 * surface_normal))
            .abs()
            .clamp(0.0, 1.0);

        let n1_over_n2 = n1 / n2;
        let sin2_alpha = (1.0 - cos_alpha * cos_alpha).max(0.0);
        let sin2_beta = n1_over_n2 * n1_over_n2 * sin2_alpha;

        // Total Internal Reflection (TIR)
        if sin2_beta >= 1.0 {
            return 1.0.into();
        }

        let cos_beta = (1.0 - sin2_beta).sqrt();

        // Fresnel equations for unpolarized light
        let rs = n1.mul_add(cos_alpha, -(n2 * cos_beta)) / n1.mul_add(cos_alpha, n2 * cos_beta);
        let rp = n2.mul_add(cos_alpha, -(n1 * cos_beta)) / n2.mul_add(cos_alpha, n1 * cos_beta);

        f64::midpoint(rs * rs, rp * rp).into()
    }
}
impl From<Fresnel> for CoatingType {
    fn from(_coating: Fresnel) -> Self {
        Self::Fresnel
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, nanometer, percent};
    use approx::assert_abs_diff_eq;
    use nalgebra::vector;

    #[test]
    fn from() {
        let coating = Fresnel;
        assert!(matches!(coating.into(), CoatingType::Fresnel));
    }
    #[test]
    fn calc_refl_same_index() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_eq!(
            coating.calc_reflectivity(&ray, surface_normal, 1.0),
            percent!(0.0)
        );

        ray.set_refractive_index(2.0).unwrap();
        assert_eq!(
            coating.calc_reflectivity(&ray, surface_normal, 2.0),
            percent!(0.0)
        );
    }
    #[test]
    fn calc_refl_glass_perpendicular() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_abs_diff_eq!(
            coating.calc_reflectivity(&ray, surface_normal, 1.5).value,
            0.04
        );
    }
    #[test]
    fn calc_refl_glass_45_deg() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        ray.set_direction(vector![0.0, 1.0, 1.0]).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_abs_diff_eq!(
            coating.calc_reflectivity(&ray, surface_normal, 1.5).value,
            0.05023991101223595
        );
    }
}
