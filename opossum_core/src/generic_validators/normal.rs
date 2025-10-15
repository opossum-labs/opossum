use crate::impl_validator;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNormal;
impl_validator!(IsNormal, |v: &f64| v.is_normal(), f64);
impl_validator!(IsNormal, |v: &Length| v.is_normal(), Length);
impl_validator!(IsNormal, |v: &Point2<f64>| v.x.is_normal() && v.y.is_normal(), Point2<f64>);
impl_validator!(IsNormal, |v: &Point2<Length>| v.x.is_normal() && v.y.is_normal(), Point2<Length>);

