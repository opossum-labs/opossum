use super::GeoSurface;
use crate::coatings::CoatingType;

/// This struct represents an optical surface, which consists of the geometric surface shape ([`GeoSurface`]) and further
/// properties such as the [`CoatingType`].
pub struct OpticalSurface {
    geo_surface: Box<dyn GeoSurface>,
    coating: CoatingType,
}

impl OpticalSurface {
    /// Creates a new [`OpticalSurface`].
    #[must_use]
    pub fn new(geo_surface: Box<dyn GeoSurface>) -> Self {
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
    pub fn geo_surface(&self) -> &dyn GeoSurface {
        &(*self.geo_surface)
    }
}
