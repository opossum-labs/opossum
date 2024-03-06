use serde::Deserialize;
use serde::Serialize;
use uom::si::length::micrometer;

use super::{RefractiveIndex, RefractiveIndexType};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RefrIndexSellmeier1 {
    k1: f64,
    k2: f64,
    k3: f64,
    l1: f64,
    l2: f64,
    l3: f64,
}
impl RefrIndexSellmeier1 {
    #[must_use]
    pub const fn new(k1: f64, k2: f64, k3: f64, l1: f64, l2: f64, l3: f64) -> Self {
        Self {
            k1,
            k2,
            k3,
            l1,
            l2,
            l3,
        }
    }
}
impl RefractiveIndex for RefrIndexSellmeier1 {
    fn get_refractive_index(&self, wavelength: uom::si::f64::Length) -> f64 {
        let lambda = wavelength.get::<micrometer>();
        let l_sq = lambda * lambda;
        f64::sqrt(
            1.0 + self.k1 * l_sq / (l_sq - self.l1)
                + self.k2 * l_sq / (l_sq - self.l2)
                + self.k3 * l_sq / (l_sq - self.l3),
        )
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Sellmeier1(self.clone())
    }
}
