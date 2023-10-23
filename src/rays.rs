#![warn(missing_docs)]
//! Module for handling rays
use crate::error::{OpmResult, OpossumError};
use uom::si::{f64::Length, length::nanometer};

///Vector or point to represent coordinates or directions in 3D space
pub struct Vec3D{
    x:      f64,
    y:      f64,
    z:      f64,
}

///Struct that contains all informatino about a ray
pub struct Ray{
    ///Stores all positions of the ray
    pos:    Vec<Vec3D>,
    ///stores the current propagation direction of the ray
    dir:    Vec3D,
    ///stores the polarization vector of the ray
    pol:    Vec3D,
    ///Wavelength of the ray in nm
    wvl:    length::nanometer,
    ///id of the ray
    id:     usize,
    ///Bounce count of the ray. Necessary to check if the maximum number of bounces is reached
    bounce: usize,
    ///True if ray is allowd to further propagate, false else
    valid:  bool,
}

///Struct containing all relevenat information of a created bundle of rays
pub struct Rays{
    ///vector containing rays
    rays:           Vec<Ray>,
    ///Maximum number of bounces
    max_bounces:    usize,
}