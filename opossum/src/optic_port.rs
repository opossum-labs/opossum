use serde::{Deserialize, Serialize};

use crate::{aperture::Aperture, coatings::CoatingType};

use core::fmt::Debug;

#[derive(Serialize, Deserialize, Clone)]
pub struct OpticPort {
    aperture: Aperture,
    coating: CoatingType,
}

impl Default for OpticPort {
    fn default() -> Self {
        Self {
            aperture: Aperture::default(),
            coating: CoatingType::IdealAR,
        }
    }
}
impl OpticPort {
    pub const fn new(aperture: Aperture, coating: CoatingType) -> Self {
        Self { aperture, coating }
    }
    pub fn set_aperture(&mut self, aperture: Aperture) {
        self.aperture = aperture;
    }
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    pub const fn aperture(&self) -> &Aperture {
        &self.aperture
    }
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }
}

impl Debug for OpticPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpticPort")
            .field("aperture", &self.aperture)
            .field("coating", &self.coating)
            .finish()
    }
}
