//! Paraxial surface (ideal lens)
//!
//! This module implements a paraxial surface with a given focal length. The geometric shape correpsonds to a flat surface but
//! the refraction corresponds to a perfect lens.
use super::geo_surface::GeoSurface;
use crate::{
    error::{OpmResult, OpossumError},
    meter, radian,
    ray::Ray,
    utils::geom_transformation::Isometry,
};
use nalgebra::{vector, Point3, Vector3};
use num::Zero;
use roots::{find_roots_quadratic, Roots};
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// A paraxial surface with a given focal length.
pub struct Paraxial {
    focal_length: Length,
    isometry: Isometry,
}
