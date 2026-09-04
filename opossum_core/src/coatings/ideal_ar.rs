#![warn(missing_docs)]
use super::{Coating, CoatingType};
use crate::light::Ray;
use nalgebra::Vector3;
use uom::si::f64::Ratio;

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
    ) -> Ratio {
        0.0.into()
    }
}
impl From<IdealAR> for CoatingType {
    fn from(_coating: IdealAR) -> Self {
        Self::IdealAR
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{error::OpmResult, joule, light::Ray, nanometer};
    use nalgebra::vector;
    use num_traits::Zero;

    #[test]
    fn from() {
        let coating = IdealAR;
        assert!(matches!(coating.into(), CoatingType::IdealAR));
    }
    #[test]
    fn calc_refl() -> OpmResult<()> {
        let coating = IdealAR;
        let ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?;
        let surface_normal = vector![0.0, 0.0, -1.0];
        assert!(
            coating
                .calc_reflectivity(&ray, surface_normal, 1.5)
                .is_zero()
        );
        Ok(())
    }
}
