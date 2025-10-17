use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Energy, Length};

use crate::impl_validator;

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AllNotEmpty;

impl_validator!(AllNotEmpty, |_self, v: &Vec<f64>| !v.is_empty(), Vec<f64>);
impl_validator!(
    AllNotEmpty,
    |_self, v: &Vec<Length>| !v.is_empty(),
    Vec<Length>
);
impl_validator!(
    AllNotEmpty,
    |_self, v: &Vec<Angle>| !v.is_empty(),
    Vec<Angle>
);
impl_validator!(
    AllNotEmpty,
    |_self, v: &Vec<Energy>| !v.is_empty(),
    Vec<Energy>
);
impl_validator!(AllNotEmpty, |_self, v: &String| !v.is_empty(), String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_is_not_empty_vec_f64() {
        let validator = AllNotEmpty;
        assert!(validator.validate(&vec![1.0, 2.0]).is_ok());
        assert!(validator.validate(&Vec::<f64>::new()).is_err());
    }

    #[test]
    fn test_is_not_empty_vec_length() {
        let validator = AllNotEmpty;
        assert!(validator.validate(&vec![Length::new::<meter>(1.0)]).is_ok());
        assert!(validator.validate(&Vec::<Length>::new()).is_err());
    }

    #[test]
    fn test_is_not_empty_vec_angle() {
        let validator = AllNotEmpty;
        assert!(validator.validate(&vec![Angle::new::<radian>(1.0)]).is_ok());
        assert!(validator.validate(&Vec::<Angle>::new()).is_err());
    }

    #[test]
    fn test_is_not_empty_string() {
        let validator = AllNotEmpty;
        assert!(validator.validate(&"hello".to_string()).is_ok());
        assert!(validator.validate(&"".to_string()).is_err());
    }
}
