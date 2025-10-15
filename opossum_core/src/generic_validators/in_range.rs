use nalgebra::Point2;
use uom::si::f64::{Angle, Length};
use crate::{error::{OpmResult, OpossumError}, impl_validator};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsInRange<T> {
        min: T,
        max: T,
        inclusive: bool
    }

impl <T: PartialOrd>IsInRange<T>{
    pub fn new(min: T, max:T, inclusive:bool) -> OpmResult<Self>{
        if min < max{
            Ok(Self{min, max ,inclusive})
        }
        else{
            Err(OpossumError::Other("IsInRange: minimum value must be smaller than maximum value".into()))
        }
    }
    pub fn is_in_range(&self, val: &T) -> bool{
        if self.inclusive{
            if *val >= self.min && *val <= self.max{
                true
            }
            else{
                false
            }
        }
        else{
            if *val > self.min && *val < self.max{
                true
            }
            else{
                false
            }
        }
    }
}


impl_validator!(IsInRange<f64>, |r: &IsInRange<f64>, v: &f64| r.is_in_range(v), f64);
impl_validator!(IsInRange<Length>, |r: &IsInRange<Length>, v: &Length| r.is_in_range(v), Length);
impl_validator!(IsInRange<Angle>, |r: &IsInRange<Angle>, v: &Angle| r.is_in_range(v), Angle);
impl_validator!(IsInRange<f64>, |r: &IsInRange<f64>, v: &Point2<f64>| r.is_in_range(&v.x) && r.is_in_range(&v.y), Point2<f64>);
impl_validator!(IsInRange<Length>, |r: &IsInRange<Length>, v: &Point2<Length>| r.is_in_range(&v.x) && r.is_in_range(&v.y), Point2<Length>);


