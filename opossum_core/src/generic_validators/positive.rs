
use nalgebra::Point2;
use uom::si::f64::Length;
use crate::impl_validator;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsPositive;

impl_validator!(IsPositive, |_self, v: &f64| v.is_sign_positive(), f64);
impl_validator!(IsPositive, |_self, v: &Length| v.is_sign_positive(), Length);
impl_validator!(IsPositive, |_self, v: &Point2<f64>| v.x.is_sign_positive() && v.y.is_sign_positive(), Point2<f64>);
impl_validator!(IsPositive, |_self, v: &Point2<Length>| v.x.is_sign_positive() && v.y.is_sign_positive(), Point2<Length>);
