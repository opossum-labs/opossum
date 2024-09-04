use super::{Coating, CoatingType};
use crate::ray::Ray;
use nalgebra::Vector3;

/// Ideal coating with zero reflectivity
///
/// This model represents a perfect antireflective coating with zero reflectivity and
/// full transmission independent of wavelength, angle of incidence, or refractive index of the
/// following medium.
pub struct IdealAR;

impl Coating for IdealAR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: &Ray,
        _surface_normal: Vector3<f64>,
        _n2: f64,
    ) -> f64 {
        0.0
    }
    fn to_enum(&self) -> super::CoatingType {
        CoatingType::IdealAR
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, nanometer, ray::Ray};
    use nalgebra::vector;

    #[test]
    fn to_enum() {
        let coating = IdealAR;
        assert!(matches!(coating.to_enum(), CoatingType::IdealAR));
    }
    #[test]
    fn calc_refl() {
        let coating = IdealAR;
        let ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        assert_eq!(coating.calc_reflectivity(&ray, surface_normal, 1.5), 0.0);
    }
}
