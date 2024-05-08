//! Optical Table surface for visualization
//!
//! An infinitely large flat 2D surface with hole markers

use crate::error::{OpmResult, OpossumError};
use crate::radian;
use crate::render::{Color, Render, Renderable, SDF};
use crate::utils::geom_transformation::Isometry;
use approx::relative_eq;
use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;

#[derive(Debug)]
/// An infinitely large flat surface with its normal collinear to the optical axis.
pub struct OpticalTable {
    normal: Vector3<f64>,
    anchor_point: Point3<Length>,
    shift: Length,
    isometry: Isometry,
    raster_dist: Length,
}
impl OpticalTable {
    /// Create a new [`OpticalTable`] located at the given z position on the optical axis.
    ///
    /// The plane is oriented vertical with respect to the optical axis (xy plane).
    /// # Errors
    ///
    /// This function will return an error if z is not finite.
    pub fn new(
        normal: Vector3<f64>,
        anchor_point: Point3<Length>,
        raster_dist: Length,
    ) -> OpmResult<Self> {
        if !raster_dist.is_finite() {
            return Err(OpossumError::Other(
                "hole rasster distance must be finite".into(),
            ));
        }
        if normal.iter().any(|x| !x.is_finite()) || relative_eq!(normal.norm(), 0.0) {
            return Err(OpossumError::Other(
                "normal vector components must be finite and its norm != 0!".into(),
            ));
        }
        let isometry = Isometry::new(anchor_point, radian!(0., 0., 0.))?;
        let shift = (anchor_point.x * anchor_point.x
            + anchor_point.y * anchor_point.y
            + anchor_point.z * anchor_point.z)
            .sqrt();
        Ok(Self {
            normal: normal.normalize(),
            anchor_point,
            shift,
            isometry,
            raster_dist,
        })
    }
    /// Returns the anchor point of this [`OpticalTable`]
    #[must_use]
    pub const fn get_anchor_point(&self) -> Point3<Length> {
        self.anchor_point
    }
}
impl Color for OpticalTable {
    fn get_color(&self, p: &Point3<f64>) -> Vector3<f64> {
        let dist = (p.x.rem_euclid(self.raster_dist.value) - self.raster_dist.value / 2.)
            .hypot(p.z.rem_euclid(self.raster_dist.value) - self.raster_dist.value / 2.);
        if dist < 2.0e-3 {
            Vector3::<f64>::new(0.5, 0.5, 0.5)
        } else if dist <= 4.5e-3 {
            let x = (4.5e-3 - dist) / 2.5e-3;
            let smooth = (3. * x).mul_add(x, -(2. * x * x * x));
            let c = 0.3f64.mul_add(-smooth, 0.8);
            Vector3::<f64>::new(c, c, c)
        } else {
            Vector3::<f64>::new(0.8, 0.8, 0.8)
        }
    }
}

impl SDF for OpticalTable {
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        let p_out = self.isometry.inverse_transform_point_f64(p);
        // p.x.mul_add(self.normal.x,  p.y.mul_add(self.normal.y, p.z.mul_add(self.normal.z , self.shift.value)))
        p_out.x.mul_add(
            self.normal.x,
            p_out.y.mul_add(
                self.normal.y,
                p_out.z.mul_add(self.normal.z, self.shift.value),
            ),
        )
    }
}
impl Render<'_> for OpticalTable {}
impl Renderable<'_> for OpticalTable {}
