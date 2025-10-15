use nalgebra::Point2;
use uom::si::f64::Length;
use crate::impl_validator;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNotNaN;

impl_validator!(IsNotNaN, |v: &f64| !v.is_nan(), f64);
impl_validator!(IsNotNaN, |v: &Length| !v.is_nan(), Length);
impl_validator!(IsNotNaN, |v: &Point2<f64>| !v.x.is_nan() && !v.y.is_nan(), Point2<f64>);
impl_validator!(IsNotNaN, |v: &Point2<Length>| !v.x.is_nan() && !v.y.is_nan(), Point2<Length>);