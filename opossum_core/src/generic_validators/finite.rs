use nalgebra::Point2;
use uom::si::f64::{Angle, Energy, Length};
use crate::prelude::*;

use crate::impl_validator;
use serde::{Deserialize, Serialize};
use crate::generic_validators::{ValidateVec, Validate};

impl Validate<Vec<(Length, f64)>> for AllFinite {
    fn validate(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec.iter().all(|val| val.0.is_finite() && !val.1.is_finite()) {
            Ok(())
        } else {
            Err(OpossumError::Other("All entries must be finite!".to_string()))
        }
    }
}

impl ValidateVec<(Length, f64)> for XFinite {
    fn validate_vec(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec.iter().all(|val| val.1.is_finite()) {
            Ok(())
        } else {
            Err(OpossumError::Other("All x-entries must be finite!".to_string()))
        }
    }
}

impl ValidateVec<(Length, f64)> for YFinite {
    fn validate_vec(&self, value_vec: &Vec<(Length, f64)>) -> OpmResult<()> {
        if value_vec.iter().all(|val| val.1.is_finite()) {
            Ok(())
        } else {
            Err(OpossumError::Other("All y-entries must be finite!".to_string()))
        }
    }
}


#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AllFinite;

impl_validator!(AllFinite, |_self, v: &f64| v.is_finite(), f64);
impl_validator!(AllFinite, |_self, v: &Length| v.is_finite(), Length);
impl_validator!(AllFinite, |_self, v: &Energy| v.is_finite(), Energy);
impl_validator!(AllFinite, |_self, v: &Angle| v.is_finite(), Angle);

impl_validator!(
    AllFinite,
    |_self, v: &Point2<f64>| v.x.is_finite() && v.y.is_finite(),
    Point2<f64>
);
impl_validator!(
    AllFinite,
    |_self, v: &Point2<Length>| v.x.is_finite() && v.y.is_finite(),
    Point2<Length>
);

impl_validator!(
    AllFinite,
    |_self, v: &(f64, f64)| v.0.is_finite() && v.1.is_finite(),
    (f64, f64)
);
impl_validator!(
    AllFinite,
    |_self, v: &(Length, Length)| v.0.is_finite() && v.1.is_finite(),
    (Length, Length)
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct XFinite;

impl_validator!(
    XFinite,
    |_self, v: &Point2<f64>| v.x.is_finite(),
    Point2<f64>
);
impl_validator!(
    XFinite,
    |_self, v: &Point2<Length>| v.x.is_finite(),
    Point2<Length>
);

impl_validator!(
    XFinite,
    |_self, v: &Vec<Point2<Length>>| v.iter().all(|val| val.x.is_finite()),
    Vec<Point2<Length>>
);

impl_validator!(
    XFinite,
    |_self, v: &Vec<(Length, Energy)>| v.iter().all(|val| val.0.is_finite()),
    Vec<(Length, Energy)>
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct YFinite;

impl_validator!(
    YFinite,
    |_self, v: &Point2<f64>| v.y.is_finite(),
    Point2<f64>
);
impl_validator!(
    YFinite,
    |_self, v: &Point2<Length>| v.y.is_finite(),
    Point2<Length>
);

impl_validator!(
    YFinite,
    |_self, v: &Vec<Point2<Length>>| v.iter().all(|val| val.y.is_finite()),
    Vec<Point2<Length>>
);

impl_validator!(
    YFinite,
    |_self, v: &Vec<(Length, Energy)>| v.iter().all(|val| val.1.is_finite()),
    Vec<(Length, Energy)>
);

impl_validator!(
    YFinite,
    |_self, v: &(Length, f64)| v.1.is_finite(),
    (Length, f64)
);

impl_validator!(
    YFinite,
    |_self, v: &(f64, f64)| v.1.is_finite(),
    (f64, f64)
);

#[cfg(test)]
mod tests {
    use crate::generic_validators::Validate;

    use super::*;
    use nalgebra::Point2;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    #[test]
    fn test_is_finite_f64() {
        let validator = AllFinite;

        assert!(validator.validate(&1.23).is_ok());
        assert!(validator.validate(&0.0).is_ok());
        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
        assert!(validator.validate(&f64::NAN).is_err());
    }

    #[test]
    fn test_is_finite_length() {
        let validator = AllFinite;
        let l = Length::new::<meter>(1.0);
        let l_inf = Length::new::<meter>(f64::INFINITY);

        assert!(validator.validate(&l).is_ok());
        assert!(validator.validate(&Length::new::<meter>(0.0)).is_ok());
        assert!(validator.validate(&l_inf).is_err());
        assert!(validator.validate(&Length::new::<meter>(f64::NAN)).is_err());
    }

    #[test]
    fn test_is_finite_point2_f64() {
        let validator = AllFinite;

        let p = Point2::new(1.0, 2.0);
        let p_inf = Point2::new(f64::INFINITY, 0.0);
        let p_nan = Point2::new(0.0, f64::NAN);

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_point2_length() {
        let validator = AllFinite;

        let p = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_inf = Point2::new(
            Length::new::<meter>(f64::INFINITY),
            Length::new::<meter>(0.0),
        );
        let p_nan = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_tuple_f64() {
        let validator = AllFinite;

        let p = (1.0, 2.0);
        let p_inf = (f64::INFINITY, 0.0);
        let p_nan = (0.0, f64::NAN);

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_tuple_length() {
        let validator = AllFinite;

        let p = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_inf = (
            Length::new::<meter>(f64::INFINITY),
            Length::new::<meter>(0.0),
        );
        let p_nan = (Length::new::<meter>(0.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }
}
