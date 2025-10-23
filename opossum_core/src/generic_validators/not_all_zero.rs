use crate::generic_validators::{Validate, ValidateVec};
use crate::impl_validator;
use crate::prelude::{OpmResult, OpossumError};
use nalgebra::Point2;
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Energy, Length};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct NotAllZero;

impl Validate<Vec<(Length, f64)>> for NotAllZero {
    fn validate(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec
            .iter()
            .any(|val| !val.0.is_zero() || !val.1.is_zero())
        {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "At least one entry must be non-zero!".to_string(),
            ))
        }
    }
}

impl ValidateVec<(Length, f64)> for YNotAllZero {
    fn validate_vec(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec.iter().any(|val| !val.1.is_zero()) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "At least one y-entry must be non-zero!".to_string(),
            ))
        }
    }
}

impl ValidateVec<(Length, f64)> for XNotAllZero {
    fn validate_vec(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec.iter().any(|val| !val.0.is_zero()) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "At least one x-entry must be non-zero!".to_string(),
            ))
        }
    }
}

impl_validator!(NotAllZero, |_self, v: &usize| !v.is_zero(), usize);
impl_validator!(NotAllZero, |_self, v: &i32| !v.is_zero(), i32);
impl_validator!(NotAllZero, |_self, v: &f64| !v.is_zero(), f64);
impl_validator!(NotAllZero, |_self, v: &Length| !v.is_zero(), Length);
impl_validator!(NotAllZero, |_self, v: &Angle| !v.is_zero(), Angle);
impl_validator!(NotAllZero, |_self, v: &Energy| !v.is_zero(), Energy);
impl_validator!(
    NotAllZero,
    |_self, v: &Point2<usize>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<usize>
);
impl_validator!(
    NotAllZero,
    |_self, v: &Point2<i32>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<i32>
);
impl_validator!(
    NotAllZero,
    |_self, v: &Point2<f64>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<f64>
);
impl_validator!(
    NotAllZero,
    |_self, v: &Point2<Length>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<Length>
);

impl_validator!(
    NotAllZero,
    |_self, v: &(usize, usize)| !(v.0.is_zero() && v.1.is_zero()),
    (usize, usize)
);
impl_validator!(
    NotAllZero,
    |_self, v: &(i32, i32)| !(v.0.is_zero() && v.1.is_zero()),
    (i32, i32)
);
impl_validator!(
    NotAllZero,
    |_self, v: &(f64, f64)| !(v.0.is_zero() && v.1.is_zero()),
    (f64, f64)
);
impl_validator!(
    NotAllZero,
    |_self, v: &(Length, Length)| !(v.0.is_zero() && v.1.is_zero()),
    (Length, Length)
);

impl_validator!(
    NotAllZero,
    |_self, v: &Vec<Length>| v.iter().any(|val| !val.is_zero()),
    Vec<Length>
);

impl_validator!(
    NotAllZero,
    |_self, v: &Vec<f64>| v.iter().any(|val| !val.is_zero()),
    Vec<f64>
);

impl_validator!(
    NotAllZero,
    |_self, v: &Vec<Angle>| v.iter().any(|val| !val.is_zero()),
    Vec<Angle>
);

impl_validator!(
    NotAllZero,
    |_self, v: &Vec<Energy>| v.iter().any(|val| !val.is_zero()),
    Vec<Energy>
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct YNotAllZero;

impl_validator!(
    YNotAllZero,
    |_self, v: &Point2<usize>| !v.y.is_zero(),
    Point2<usize>
);
impl_validator!(
    YNotAllZero,
    |_self, v: &Point2<i32>| !v.y.is_zero(),
    Point2<i32>
);
impl_validator!(
    YNotAllZero,
    |_self, v: &Point2<f64>| !v.y.is_zero(),
    Point2<f64>
);
impl_validator!(
    YNotAllZero,
    |_self, v: &Point2<Length>| !v.y.is_zero(),
    Point2<Length>
);

impl_validator!(
    YNotAllZero,
    |_self, v: &Vec<Point2<Length>>| v.iter().any(|val| !val.y.is_zero()),
    Vec<Point2<Length>>
);

impl_validator!(
    YNotAllZero,
    |_self, v: &Vec<Point2<f64>>| v.iter().any(|val| !val.y.is_zero()),
    Vec<Point2<f64>>
);

impl_validator!(
    YNotAllZero,
    |_self, v: &Vec<Point2<Angle>>| v.iter().any(|val| !val.y.is_zero()),
    Vec<Point2<Angle>>
);

impl_validator!(
    YNotAllZero,
    |_self, v: &Vec<Point2<Energy>>| v.iter().any(|val| !val.y.is_zero()),
    Vec<Point2<Energy>>
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct XNotAllZero;

impl_validator!(
    XNotAllZero,
    |_self, v: &Point2<usize>| !v.x.is_zero(),
    Point2<usize>
);
impl_validator!(
    XNotAllZero,
    |_self, v: &Point2<i32>| !v.x.is_zero(),
    Point2<i32>
);
impl_validator!(
    XNotAllZero,
    |_self, v: &Point2<f64>| !v.x.is_zero(),
    Point2<f64>
);
impl_validator!(
    XNotAllZero,
    |_self, v: &Point2<Length>| !v.x.is_zero(),
    Point2<Length>
);

impl_validator!(
    XNotAllZero,
    |_self, v: &Vec<Point2<Length>>| v.iter().any(|val| !val.x.is_zero()),
    Vec<Point2<Length>>
);

impl_validator!(
    XNotAllZero,
    |_self, v: &Vec<Point2<f64>>| v.iter().any(|val| !val.x.is_zero()),
    Vec<Point2<f64>>
);

impl_validator!(
    XNotAllZero,
    |_self, v: &Vec<Point2<Angle>>| v.iter().any(|val| !val.x.is_zero()),
    Vec<Point2<Angle>>
);

impl_validator!(
    XNotAllZero,
    |_self, v: &Vec<Point2<Energy>>| v.iter().any(|val| !val.x.is_zero()),
    Vec<Point2<Energy>>
);
