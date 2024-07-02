#![warn(missing_docs)]
//! Module for the calculation of the signed distane function of nodes.

use super::{Color, Render};
use nalgebra::{Point3, Vector3};

///Helper trait that combines the Render and SDF traits
pub trait Renderable<'a>: Render<'a> + SDF {}

/// Enum to define the binary operation for sdf objects
pub enum SDFOperation {
    /// build a union from all objects
    Union,
    /// build an intersection from all objects
    Intersection,
    /// build the subtraction of all objects from a specific sdf object at position idx in an [`SDFCollection`]
    Subtraction {
        /// index of the sdf oject from which the others should be subtracted
        idx: usize,
    },
}

///Struct to stor a collection of sdf objects. This also includes [`SDFCollection`] structs
pub struct SDFCollection<'a> {
    sdf_objs: Vec<&'a dyn Renderable<'a>>,
    sdf_op: SDFOperation,
    bbox: tessellation::BoundingBox<f64>,
}

impl<'a> Render<'_> for SDFCollection<'a> {}
impl<'a> Renderable<'_> for SDFCollection<'a> {}

impl<'a> Color for SDFCollection<'a> {
    fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
        Vector3::<f64>::new(0.8, 0.7, 0.6)
    }
}
impl<'a> SDF for SDFCollection<'a> {
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        self.sdf_eval(p)
    }
    fn sdf_eval_with_color(&self, p: &Point3<f64>) -> (f64, Vector3<f64>) {
        (self.sdf_eval_point(p), self.get_color(p))
    }
}
impl<'a> SDFCollection<'a> {
    /// Create a new [`SDFCollection`] struct
    /// # Attributes
    /// - `sdf_objs`: vector of sdf objects: must hav implemented the renderable trait
    /// - `sdf_op_opt`: option for the Operation that should be used for combining the sdfs. default: Union
    /// Returns
    /// - None if the length of the SDF oject vector is zero
    /// - `Option<Self>` otherwise
    #[must_use]
    pub fn new(
        sdf_objs: Vec<&'a dyn Renderable<'a>>,
        sdf_op_opt: Option<SDFOperation>,
        bbox: tessellation::BoundingBox<f64>,
    ) -> Option<Self> {
        if sdf_objs.is_empty() {
            None
        } else if let Some(sdf_op) = sdf_op_opt {
            Some(Self {
                sdf_objs,
                sdf_op,
                bbox,
            })
        } else {
            Some(Self {
                sdf_objs,
                sdf_op: SDFOperation::Union,
                bbox,
            })
        }
    }
    /// Returns the bounding box of this collection
    #[must_use]
    pub const fn bounding_box(&self) -> &tessellation::BoundingBox<f64> {
        &self.bbox
    }
    /// Add and sdf object (must implement Renderable) to this [`SDFCollection`]
    pub fn add_sdf_obj(&mut self, sdf_obj: &'a dyn Renderable<'a>) {
        self.sdf_objs.push(sdf_obj);
    }

    /// Evaluate the sdf of this [`SDFCollection`] for a given point
    /// # Attributes
    /// - `p`: Point3 at which the sdf should be evaluated
    /// # Returns
    /// Retruns the smallest signed distance to this point
    #[must_use]
    pub fn sdf_eval(&self, p: &Point3<f64>) -> f64 {
        match self.sdf_op {
            SDFOperation::Intersection => self.sdf_intersection(p),
            SDFOperation::Union => self.sdf_union(p),
            SDFOperation::Subtraction { idx } => self.sdf_subtraction(p, idx),
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as Union for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this Union
    #[must_use]
    pub fn sdf_union(&self, p: &Point3<f64>) -> f64 {
        if self.sdf_objs.len() > 1 {
            self.sdf_objs[0].sdf_union_point(&self.sdf_objs[1..], p)
        } else {
            self.sdf_objs[0].sdf_eval_point(p)
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as intersection for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this intersection
    #[must_use]
    pub fn sdf_intersection(&self, p: &Point3<f64>) -> f64 {
        if self.sdf_objs.len() > 1 {
            self.sdf_objs[0].sdf_intersection_point(&self.sdf_objs[1..], p)
        } else {
            self.sdf_objs[0].sdf_eval_point(p)
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as subtraction for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this subtraction
    #[must_use]
    pub fn sdf_subtraction(&self, p: &Point3<f64>, subtract_from_idx: usize) -> f64 {
        if self.sdf_objs.len() > 1 {
            if subtract_from_idx == self.sdf_objs.len() - 1 {
                self.sdf_objs[subtract_from_idx]
                    .sdf_subtraction_point(&self.sdf_objs[..subtract_from_idx], p)
            } else {
                self.sdf_objs[subtract_from_idx].sdf_subtraction_point(
                    &[
                        &self.sdf_objs[0..subtract_from_idx],
                        &self.sdf_objs[subtract_from_idx + 1..],
                    ]
                    .concat(),
                    p,
                )
            }
        } else {
            self.sdf_objs[0].sdf_eval_point(p)
        }
    }
}

/// Trait for the calculation of signed distance fields which is used for optic rendering and aperture evaluation
/// The signed distance of a point to an object is the orthogonal distance to the surface of that object.
/// It is
/// - negative for points inside of the object
/// - positive for points outside of the object
/// - zero if the point is on the surface
pub trait SDF: Color {
    /// Calculation of the signed distance function value for a single point.
    /// This function must be implemented individually for each object, as the definition of the signed distance function is different for each object
    /// # Arguments
    /// - `p`: 3D point filled with xyz coordinates of type Length
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64;

    /// Calculation of the signed distance function value for a single.
    /// This function must be implemented individually for each object, as the definition of the signed distance function is different for each object
    /// # Arguments
    /// - `p`: 3D point filled with xyz coordinates of type Length
    fn sdf_eval_with_color(&self, p: &Point3<f64>) -> (f64, Vector3<f64>) {
        (self.sdf_eval_point(p), self.get_color(p))
    }

    /// Calculation of the signed distance function value for a vector of points
    /// # Arguments
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance for each input point
    fn sdf_eval_vec_of_points(&self, p_vec: &Vec<Point3<f64>>) -> Vec<f64> {
        let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
        for p in p_vec {
            sdf_out.push(self.sdf_eval_point(p));
        }
        sdf_out
    }
}

// impl tessellation::ImplicitFunction<f64> for SDFCollection<'_> {
//     fn bbox(&self) -> &tessellation::BoundingBox<f64> {
//       &self.bbox
//     }
//    fn value(&self, p: &Point3<f64>) -> f64 {
//      return self.sdf_eval_point(p);
//    }
//    fn normal(&self, p: &nalgebra::Point3<f64>) -> nalgebra::Vector3<f64> {
//      return nalgebra::Vector3::new(p.x, p.y, p.z).normalize();
//    }
//  }
