use nalgebra::Point2;
use num::Zero;
use uom::si::f64::Length;
use crate::impl_validator;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNotZero;

impl_validator!(IsNotZero, |v: &usize| !v.is_zero(), usize);
impl_validator!(IsNotZero, |v: &i32| !v.is_zero(), i32);
impl_validator!(IsNotZero, |v: &f64| !v.is_zero(), f64);
impl_validator!(IsNotZero, |v: &Length| !v.is_zero(), Length);
impl_validator!(IsNotZero, |v: &Point2<f64>| !v.x.is_zero() && !v.y.is_zero(), Point2<f64>);
impl_validator!(IsNotZero, |v: &Point2<Length>| !v.x.is_zero() && !v.y.is_zero(), Point2<Length>);

