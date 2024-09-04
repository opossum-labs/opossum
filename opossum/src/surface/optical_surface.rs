use super::{GeoSurf, GeoSurface};
use crate::{coatings::CoatingType, physical_optic_component::SDF};

/// This struct represents an optical surface, which consists of the geometric surface shape ([`GeoSurface`]) and further
/// properties such as the [`CoatingType`].
#[derive(Clone)]
pub struct OpticalSurface {
    geo_surface: GeoSurf,
    coating: CoatingType,
}

impl OpticalSurface {
    /// Creates a new [`OpticalSurface`].
    #[must_use]
    pub fn new(geo_surface: GeoSurf) -> Self {
        Self {
            geo_surface,
            coating: CoatingType::IdealAR,
        }
    }
    /// Returns a reference to the coating of this [`OpticalSurface`].
    #[must_use]
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }
    /// Sets the coating of this [`OpticalSurface`].
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns a reference to the geo surface of this [`OpticalSurface`].
    #[must_use]
    pub fn geo_surface(&self) -> &GeoSurf {
        &self.geo_surface
    }
}

impl SDF for OpticalSurface{
    fn sdf_eval_point(&self, p: &nalgebra::Point3<f64>) -> f64 {
        self.geo_surface.sdf_eval_point(p)
    }
}
