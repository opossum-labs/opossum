#![warn(missing_docs)]
//! Module for handling rays
use uom::si::f64::{Length, Energy};
use nalgebra::{Point2, Point3};
use nalgebra::{Vector2, Vector3, Complex};

use crate::aperture::{self, Aperture};

///Struct that contains all informatino about a ray
pub struct Ray{
    ///Stores all positions of the ray
    pos:    Point3<f64>, // this should be a vector of points?
    ///stores the current propagation direction of the ray
    dir:    Vector3<f64>,
    ///stores the polarization vector (Jones vector) of the ray
    pol:    Vector2<Complex<f64>>,
    ///energy of the ray
    e: Energy,
    ///Wavelength of the ray in nm
    wvl:    Length,
    ///id of the ray
    id:     usize,
    ///Bounce count of the ray. Necessary to check if the maximum number of bounces is reached
    bounce: usize,
    //True if ray is allowd to further propagate, false else
    //valid:  bool,
}
impl Ray {
    /// Create a new collimated ray.
    /// 
    /// Generate a ray a horizontally polarized ray collinear with the z axis (optical axis).
    pub fn new_collimated(position: Point2<f64>, wave_length: Length, energy: Energy) -> Self {
        Self {
            pos: Point3::new(position.x, position.y, 0.0),
            dir: Vector3::new(0.0,0.0,1.0),
            pol: Vector2::new(Complex::new(1.0,0.0),Complex::new(0.0,0.0)), // horizontal polarization
            e: energy,
            wvl: wave_length,
            id: 0,
            bounce: 0
        }
    }
}
///Struct containing all relevant information of a created bundle of rays
pub struct Rays{
    ///vector containing rays
    rays:           Vec<Ray>,
    //Maximum number of bounces
    //max_bounces:    usize, do we need this here?
}

pub enum RayDistributionStrategy {
    Hexapolar(u8),
    Random(usize),
    Sobol(usize)
}
impl RayDistributionStrategy {
    pub fn generate(&self, radius: f64) -> Vec<Point2<f64>> {

        vec![]
    }
}
impl Rays {
    pub fn new_uniform_collimated(radius: f64, wave_length: Length, energy: Energy, number_of_rays: usize, strategy: RayDistributionStrategy) -> Self {
        let points: Vec<Point2<f64>>=strategy.generate(radius);
        let mut rays: Vec<Ray>=Vec::new();
        for point in points {
            let ray=Ray::new_collimated(point, wave_length, energy);
            rays.push(ray);
        }
        Self {
            rays
        }
    }
}