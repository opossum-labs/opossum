use nalgebra::Point2;
use num::Zero;
use uom::si::f64::Length;
use crate::impl_validator;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct OnlyOneZero;
impl_validator!(OnlyOneZero, |_self, v: &Point2<f64>| !(v.x.is_zero() && v.y.is_zero()), Point2<f64>);
impl_validator!(OnlyOneZero, |_self, v: &Point2<Length>| !(v.x.is_zero() && v.y.is_zero()), Point2<Length>);
