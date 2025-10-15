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


#[cfg(test)]
mod tests {
    use crate::generic_validators::Validate;

    use super::*;
    use nalgebra::Point2;
    use uom::si::f64::{Length, Angle};
    use uom::si::length::meter;
    use uom::si::angle::radian;

    #[test]
    fn test_is_in_range_f64_inclusive() {
        let validator = IsInRange::new(1.0, 5.0, true).unwrap();

        assert!(validator.validate(&1.0).is_ok()); 
        assert!(validator.validate(&5.0).is_ok()); 
        assert!(validator.validate(&3.0).is_ok()); 
        assert!(validator.validate(&0.0).is_err());
        assert!(validator.validate(&6.0).is_err());
    }

    #[test]
    fn test_is_in_range_f64_exclusive() {
        let validator = IsInRange::new(1.0, 5.0, false).unwrap();

        assert!(validator.validate(&1.0).is_err());
        assert!(validator.validate(&5.0).is_err());
        assert!(validator.validate(&3.0).is_ok());
    }

    #[test]
    fn test_is_in_range_length() {
        let validator = IsInRange::new(
            Length::new::<meter>(1.0),
            Length::new::<meter>(5.0),
            true
        ).unwrap();

        assert!(validator.validate(&Length::new::<meter>(1.0)).is_ok());
        assert!(validator.validate(&Length::new::<meter>(5.0)).is_ok());
        assert!(validator.validate(&Length::new::<meter>(0.5)).is_err());
    }

    #[test]
    fn test_is_in_range_angle() {
        let validator = IsInRange::new(
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(3.14),
            true
        ).unwrap();

        assert!(validator.validate(&Angle::new::<radian>(0.0)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(3.14)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(-1.0)).is_err());
    }

    #[test]
    fn test_is_in_range_point2_f64() {
        let validator = IsInRange::new(1.0, 5.0, true).unwrap();
        let p_valid = Point2::new(2.0, 3.0);
        let p_invalid = Point2::new(0.0, 4.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_is_in_range_point2_length() {
        let validator = IsInRange::new(
            Length::new::<meter>(1.0),
            Length::new::<meter>(5.0),
            true
        ).unwrap();

        let p_valid = Point2::new(Length::new::<meter>(2.0), Length::new::<meter>(3.0));
        let p_invalid = Point2::new(Length::new::<meter>(0.5), Length::new::<meter>(4.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }
}
