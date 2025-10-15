use nalgebra::Point2;
use uom::si::f64::Length;

use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate, impl_validator};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsFinite;

impl_validator!(IsFinite, |v: &f64| v.is_finite(), f64);
impl_validator!(IsFinite, |v: &Length| v.is_finite(), Length);
impl_validator!(IsFinite, |v: &Point2<f64>| v.x.is_finite() && v.y.is_finite(), Point2<f64>);
impl_validator!(IsFinite, |v: &Point2<Length>| v.x.is_finite() && v.y.is_finite(), Point2<Length>);

