#![warn(missing_docs)]
//! Module for the calculation of the signed distane function of nodes.

use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc};

use nalgebra::Point3;

use super::Render;


#[derive(Clone)]
pub struct SDFObj<'a>{
    sdf_obj: &'a (dyn SDF + 'static)
}
impl <'a>SDFObj<'a>{
    pub fn new(sdf_obj: &'a (dyn SDF + 'static) )-> Self{
        Self { sdf_obj }
    }
    pub fn get_sdf(&'a self)-> &'a dyn SDF{
        self.sdf_obj
    }
}

pub enum SDFOperation{
    Union,
    Intersection,
    Subtraction{idx: usize}
}

pub struct SDFCollection<'a>{
    sdf_objs: Vec<SDFObj<'a>>,
    sdf_op: SDFOperation
}

impl <'a> Render for SDFCollection<'a>{}

impl <'a> SDF for SDFCollection<'a>{   
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        self.sdf_eval(&vec![*p])[0]
    }
}
impl<'a> SDFCollection<'a>{
    pub fn new(sdf_objs: Vec<SDFObj<'a>>, sdf_op_opt: Option<SDFOperation>) -> Option<Self>{
        if sdf_objs.len() !=0 {
            if let Some(sdf_op) = sdf_op_opt{
                Some(Self { sdf_objs, sdf_op })
            }
            else{
                Some(Self { sdf_objs, sdf_op: SDFOperation::Union })
            }
        }
        else{
            None
        }
    }
    pub fn add_sdf_obj(&mut self, sdf_obj: SDFObj<'a>){
        self.sdf_objs.push(sdf_obj);
    }
    pub fn sdf_eval(&self, p_vec: &Vec<Point3<f64>>) -> Vec<f64>{
        match self.sdf_op{
            SDFOperation::Intersection=> self.sdf_intersection(p_vec),
            SDFOperation::Union => self.sdf_union(p_vec),
            SDFOperation::Subtraction { idx } => self.sdf_subtraction(p_vec, idx)
        }
    }

    pub fn sdf_union(&self, p_vec: &Vec<Point3<f64>>) -> Vec<f64>{
        if self.sdf_objs.len() > 1{
            self.sdf_objs[0].sdf_obj.sdf_union_vec_of_points(&self.sdf_objs[1..], p_vec)
        }
        else{
            self.sdf_objs[0].sdf_obj.sdf_union_vec_of_points(&self.sdf_objs[0..0], p_vec)
        }
    }

    pub fn sdf_intersection(&self, p_vec: &Vec<Point3<f64>>) -> Vec<f64>{
        if self.sdf_objs.len() > 1{
            self.sdf_objs[0].sdf_obj.sdf_intersection_vec_of_points(&self.sdf_objs[1..], p_vec)
        }
        else{
            self.sdf_objs[0].sdf_obj.sdf_intersection_vec_of_points(&self.sdf_objs[..0], p_vec)
        }
    }

    pub fn sdf_subtraction(&self, p_vec: &Vec<Point3<f64>>, subtract_from_idx: usize) -> Vec<f64>{
        if self.sdf_objs.len() > 1{
            if subtract_from_idx == self.sdf_objs.len() - 1{
                self.sdf_objs[subtract_from_idx].sdf_obj.sdf_subtraction_vec_of_points(&self.sdf_objs[..subtract_from_idx], p_vec)
            }
            else{
                self.sdf_objs[subtract_from_idx].sdf_obj.sdf_subtraction_vec_of_points(&[&self.sdf_objs[0..subtract_from_idx], &self.sdf_objs[subtract_from_idx+1..]].concat(), p_vec)
            }
        }
        else{
            self.sdf_objs[0].sdf_obj.sdf_subtraction_vec_of_points(&self.sdf_objs[0..0], p_vec)
        }
    }

    
}

/// Trait for the calculation of signed distance fields which is used for optic rendering and aperture evaluation
/// The signed distance of a point to an object is the orthogonal distance to the surface of that object.
/// It is
/// - negative for points inside of the object
/// - positive for points outside of the object
/// - zero if the point is on the surface
pub trait SDF
{
    /// Calculation of the signed distance function value for a single point.
    /// This function must be implemented individually for each object, as the definition of the signed distance function is different for each object
    /// # Arguments
    /// - `p`: 3D point filled with xyz coordinates of type Length
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64;

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

    /// Calculation of a union of signed distance functions for a vector of points.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`SDF`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the objects' union for each input point
    fn sdf_union_vec_of_points(
        &self,
        sdf_vec: &[SDFObj],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_obj.sdf_eval_point(p).min(arg0)
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
    fn sdf_union_point(&self, sdf_vec: &[SDFObj], p: &Point3<f64>) -> f64 {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_obj.sdf_eval_point(p).min(arg0)
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
        sdf_vec: &[SDFObj],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_obj.sdf_eval_point(p).max(arg0)
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
    fn sdf_intersection_point(&self, sdf_vec: &[SDFObj], p: &Point3<f64>) -> f64 {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_obj.sdf_eval_point(p).max(arg0)
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
        sdf_vec: &[SDFObj],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.len() == 0 {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    arg0.max(-sdf.sdf_obj.sdf_eval_point(p))
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
    fn sdf_subtraction_point(&self, sdf_vec: &[SDFObj], p: &Point3<f64>) -> f64 {
        if sdf_vec.len() == 0 {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                arg0.max(-sdf.sdf_obj.sdf_eval_point(p))
            })
        }
    }
}
