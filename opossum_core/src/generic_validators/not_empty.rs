use serde::{Deserialize, Serialize};

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::Validate,
    impl_validator,
};

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AllNotEmpty;

impl<T> Validate<Vec<T>> for AllNotEmpty {
    fn validate(&self, value_vec: &Vec<T>) -> OpmResult<()> {
        if value_vec.is_empty() {
            Err(OpossumError::Other("Vector must not empty!".to_string()))
        } else {
            Ok(())
        }
    }
}

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
