#![warn(missing_docs)]
//! Module for the calculation of the signed distane function of nodes.

use nalgebra::Point3;
use uom::si::f64::Length;

use crate::millimeter;

/// Trait for the calculation of signed distance fields which is used for optic rendering and aperture evaluation
/// The signed distance of a point to an object is the orthogonal distance to the surface of that object.
/// It is
/// - negative for points inside of the object
/// - positive for points outside of the object
/// - zero if the point is on the surface
pub trait SDF {
    /// Calculation of the signed distance function value for a single point.
    /// This function must be implemented individually for each object, as the definition of the signed distance function is different for each object
    /// # Arguments
    /// - `p`: 3D point filled with xyz coordinates of type Length
    fn sdf_eval_point(&self, p: &Point3<Length>) -> Length;

    /// Calculation of the signed distance function value for a vector of points
    /// # Arguments
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance for each input point
    fn sdf_eval_vec_of_points(&self, p_vec: &Vec<Point3<Length>>) -> Vec<Length> {
        let mut sdf_out = Vec::<Length>::with_capacity(p_vec.len());
        for p in p_vec {
            sdf_out.push(self.sdf_eval_point(p));
        }
        sdf_out
    }

    /// Calculation of a union of signed distance functions for a vector of points.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the objects' union for each input point
    fn sdf_union_vec_of_points(
        &self,
        sdf_vec: Vec<&impl SDF>,
        p_vec: &Vec<Point3<Length>>,
    ) -> Vec<Length> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<Length>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_eval_point(p).min(arg0)
                }));
            }
            sdf_out
        }
    }

    /// Calculation of a union of signed distance functions for a single point.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the objects' union for each input point
    fn sdf_union_point(&self, sdf_vec: Vec<&impl SDF>, p: &Point3<Length>) -> Length {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_eval_point(p).min(arg0)
            })
        }
    }

    /// Calculation of an intersection of signed distance functions for a vector of points.
    /// The intersection of difference objects is calculated by taking the maximum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the objects' intersection for each input point
    fn sdf_intersection_vec_of_points(
        &self,
        sdf_vec: &Vec<&impl SDF>,
        p_vec: &Vec<Point3<Length>>,
    ) -> Vec<Length> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<Length>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_eval_point(p).max(arg0)
                }));
            }
            sdf_out
        }
    }

    // Calculation of an intersection of signed distance functions for a single point.
    /// The intersection of difference objects is calculated by taking the maximum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the objects' intersection for each input point
    fn sdf_intersection_point(&self, sdf_vec: Vec<&impl SDF>, p: &Point3<Length>) -> Length {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_eval_point(p).max(arg0)
            })
        }
    }

    /// Calculation of a subtraction of signed distance functions for a vector of points.
    /// The subtraction of difference objects is calculated by taking the maximum value of the object to subtract from (self) and the negative value of all other objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the input object (self), subtracted by all other objects intersection for each input point
    fn sdf_subtraction_vec_of_points(
        &self,
        sdf_vec: &Vec<&impl SDF>,
        p_vec: &Vec<Point3<Length>>,
    ) -> Vec<Length> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<Length>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    arg0.max(-sdf.sdf_eval_point(p))
                }));
            }
            sdf_out
        }
    }

    // Calculation of a subtraction of signed distance functions for a single point.
    /// The subtraction of difference objects is calculated by taking the maximum value of the object to subtract from (self) and the negative value of all other objects.    
    /// /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the input object (self), subtracted by all other objects intersection
    fn sdf_subtraction_point(&self, sdf_vec: Vec<&impl SDF>, p: &Point3<Length>) -> Length {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                arg0.max(-sdf.sdf_eval_point(p))
            })
        }
    }
}
