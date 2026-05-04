use serde::{Deserialize, Serialize};

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::ValidateVec,
};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default)]
pub struct Min3Entries;

impl<T: Clone> ValidateVec<T> for Min3Entries {
    fn validate_vec(&self, values: &[T]) -> OpmResult<()> {
        if values.len() < 3 {
            Err(OpossumError::Other(
                "Vector must have at least 3 entries!".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_min_3_entries_vec_f64() {
        let validator = Min3Entries;
        assert!(validator.validate_vec(&vec![1.0, 2.0, 3.0]).is_ok());
        assert!(validator.validate_vec(&vec![1.0, 2.0]).is_err());
    }

    #[test]
    fn test_min_3_entries_vec_length() {
        let validator = Min3Entries;
        assert!(
            validator
                .validate_vec(&vec![
                    Length::new::<meter>(1.0),
                    Length::new::<meter>(2.0),
                    Length::new::<meter>(3.0)
                ])
                .is_ok()
        );
        assert!(validator.validate_vec(&Vec::<Length>::new()).is_err());
    }

    #[test]
    fn test_min_3_entries_vec_angle() {
        let validator = Min3Entries;
        assert!(
            validator
                .validate_vec(&vec![
                    Angle::new::<radian>(1.0),
                    Angle::new::<radian>(2.0),
                    Angle::new::<radian>(3.0)
                ])
                .is_ok()
        );
        assert!(validator.validate_vec(&Vec::<Angle>::new()).is_err());
    }
}
