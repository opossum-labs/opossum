#![warn(missing_docs)]
//! Module for the calculation of the signed distane function of nodes.

use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc};

use nalgebra::{Point3, Vector3, Vector4};

use super::{Color, Render};

pub trait Renderable<'a>: Render<'a> + SDF{}


pub enum SDFOperation{
    Union,
    Intersection,
    Subtraction{idx: usize}
}

pub struct SDFCollection<'a>{
    sdf_objs: Vec<&'a dyn Renderable<'a>>,
    sdf_op: SDFOperation
}

impl <'a>Render<'_> for SDFCollection<'a>{}
impl <'a>Renderable<'_> for SDFCollection<'a>{}

impl <'a>Color for SDFCollection<'a>{
    fn get_color(&self, _p:&Point3<f64>) -> Vector3<f64> {
        Vector3::<f64>::new(0.8,0.7,0.6)
    }
}
impl <'a>SDF for SDFCollection<'a>{   
    fn sdf_eval_point(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64 {
        self.sdf_eval(p, p_out)
    }
    fn sdf_eval_with_color(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> (f64, Vector3<f64>){
        (self.sdf_eval_point(p, p_out), self.get_color(p))
    }
}
impl <'a>SDFCollection<'a>{
    pub fn new(sdf_objs: Vec<&'a dyn Renderable<'a>>, sdf_op_opt: Option<SDFOperation>) -> Option<Self>{
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
    pub fn add_sdf_obj(&mut self, sdf_obj: &'a dyn Renderable<'a>){
        self.sdf_objs.push(sdf_obj);
    }

    pub fn sdf_eval(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64{
        match self.sdf_op{
            SDFOperation::Intersection=> self.sdf_intersection(p, p_out),
            SDFOperation::Union => self.sdf_union(p, p_out),
            SDFOperation::Subtraction { idx } => self.sdf_subtraction(p, p_out, idx)
        }
    }

    pub fn sdf_union(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64{
        if self.sdf_objs.len() > 1{
            self.sdf_objs[0].sdf_union_point(&self.sdf_objs[1..], p, p_out)
        }
        else{
            self.sdf_objs[0].sdf_eval_point(p, p_out)
        }
    }

    pub fn sdf_intersection(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64{
        if self.sdf_objs.len() > 1{
            self.sdf_objs[0].sdf_intersection_point(&self.sdf_objs[1..], p, p_out)
        }
        else{
            self.sdf_objs[0].sdf_eval_point(p, p_out)
        }
    }

    pub fn sdf_subtraction(&self, p: &Point3<f64>, p_out: &mut Point3<f64>, subtract_from_idx: usize) -> f64{
        if self.sdf_objs.len() > 1{
            if subtract_from_idx == self.sdf_objs.len() - 1{
                self.sdf_objs[subtract_from_idx].sdf_subtraction_point(&self.sdf_objs[..subtract_from_idx], p, p_out)
            }
            else{
                self.sdf_objs[subtract_from_idx].sdf_subtraction_point(&[&self.sdf_objs[0..subtract_from_idx], &self.sdf_objs[subtract_from_idx+1..]].concat(), p, p_out)
            }
        }
        else{
            self.sdf_objs[0].sdf_eval_point(p, p_out)
        }
    }

    
}


/// Trait for the calculation of signed distance fields which is used for optic rendering and aperture evaluation
/// The signed distance of a point to an object is the orthogonal distance to the surface of that object.
/// It is
/// - negative for points inside of the object
/// - positive for points outside of the object
/// - zero if the point is on the surface
pub trait SDF:Color
{
    /// Calculation of the signed distance function value for a single point.
    /// This function must be implemented individually for each object, as the definition of the signed distance function is different for each object
    /// # Arguments
    /// - `p`: 3D point filled with xyz coordinates of type Length
    fn sdf_eval_point(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64;

    fn sdf_eval_with_color(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> (f64, Vector3<f64>){
        (self.sdf_eval_point(p, p_out), self.get_color(p))
    }

    /// Calculation of the signed distance function value for a vector of points
    /// # Arguments
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance for each input point
    fn sdf_eval_vec_of_points(&self, p_vec: &Vec<Point3<f64>>, p_out: &mut Point3<f64>) -> Vec<f64> {
        let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
        for p in p_vec {
            sdf_out.push(self.sdf_eval_point(p, p_out));
        }
        sdf_out
    }    
}
