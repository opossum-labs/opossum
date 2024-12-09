use nalgebra::Vector3;

use crate::ray::Ray;

use super::{Coating, CoatingType};

/// Simulation of a Fresnel reflection (e.g. uncaoted surface)
///
/// This coating model simulates the Fresnel reflection of an (uncoated) surface. The reflectivity thereby depends on
/// the angle of incidence and the refractive index of the following medium.
/// For further information check the corresponding [Wikipedia article](https://en.wikipedia.org/wiki/Fresnel_equations).
/// Currently, an (50/50) unpolarized beam is assumed.
pub struct Fresnel;

impl Coating for Fresnel {
    /// Formulas taken from [german wikipedia](https://de.wikipedia.org/wiki/Fresnelsche_Formeln).
    fn calc_reflectivity(&self, incoming_ray: &Ray, surface_normal: &Vector3<f64>, n2: f64) -> f64 {
        // Note: invert surface normal, since it is the "output_1" direction.
        let alpha = incoming_ray.direction().angle(&(-1.0 * surface_normal));
        let n1 = incoming_ray.refractive_index();
        let beta = f64::acos(
            f64::sqrt(
                n1.powi(2)
                    .mul_add(-f64::powi(f64::sin(alpha), 2), n2.powi(2)),
            ) / n2,
        );
        // s-polarization
        let r_s = n1.mul_add(f64::cos(alpha), -(n2 * f64::cos(beta)))
            / n1.mul_add(f64::cos(alpha), n2 * f64::cos(beta));
        // p-polarization
        let r_p = n2.mul_add(f64::cos(alpha), -(n1 * f64::cos(beta)))
            / n2.mul_add(f64::cos(alpha), n1 * f64::cos(beta));
        // so far, we assume unpolarized (50/50) rays -> take average
        r_p.mul_add(r_p, r_s.powi(2)) / 2.
    }
    fn to_enum(&self) -> super::CoatingType {
        CoatingType::Fresnel
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, nanometer};
    use approx::assert_abs_diff_eq;
    use nalgebra::vector;

    #[test]
    fn to_enum() {
        let coating = Fresnel;
        assert!(matches!(coating.to_enum(), CoatingType::Fresnel));
    }
    #[test]
    fn calc_refl_same_index() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_eq!(coating.calc_reflectivity(&ray, &surface_normal, 1.0), 0.0);

        ray.set_refractive_index(2.0).unwrap();
        assert_eq!(coating.calc_reflectivity(&ray, &surface_normal, 2.0), 0.0);
    }
    #[test]
    fn calc_refl_glass_perpendicular() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_abs_diff_eq!(coating.calc_reflectivity(&ray, &surface_normal, 1.5), 0.04);
    }
    #[test]
    fn calc_refl_glass_45_deg() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        ray.set_refractive_index(1.0).unwrap();
        ray.set_direction(vector![0.0, 1.0, 1.0]).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        let coating = Fresnel;
        assert_abs_diff_eq!(
            coating.calc_reflectivity(&ray, &surface_normal, 1.5),
            0.05023991101223595
        );
    }
}
