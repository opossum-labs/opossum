//! Module for additional computational capabilities

pub mod griddata;
pub mod geom_transformation;

// pub use griddata::*;
// pub use geom_transformation::*;
// pub trait Surface {
//     /// Calculate intersection point and its normal vector of a [`Ray`] with a [`Surface`]
//     ///
//     /// This function returns `None` if the given ray does not intersect with the surface.
//     fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
// }
