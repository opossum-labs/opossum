#[macro_export]
macro_rules! impl_validator {
    ($validator:path, $func:expr,  $t:ty) => {
        impl $crate::generic_validators::Validate<$t> for $validator {
            fn validate(&self, value: &$t) -> $crate::error::OpmResult<()> {
                if $func(&self, &value) {
                    Ok(())
                } else {
                    Err($crate::error::OpossumError::Other(format!(
                        "Value must satisfy {}",
                        stringify!($func)
                    )))
                }
            }
        }
    };
}

#[macro_export]
macro_rules! validator_expr {
    // parentheses first, recursive zuerst
    (( $($inner:tt)+ )) => {
        $crate::validator_expr!($($inner)+)
    };

    // AND
    ($left:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidator::new(
            $crate::validator_expr!($left),
            $crate::validator_expr!($($rest)+)
        )
    };

    // OR
    ($left:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
            $crate::validator_expr!($left),
            $crate::validator_expr!($($rest)+)
        )
    };

    // single
    ($v:expr) => { $v };
}

#[macro_export]
macro_rules! validator_type_expr {
    // Parentheses: unwrap and recurse
    ($t:ty; ( $($inner:tt)+ )) => {
        $crate::validator_type_expr!($t; $($inner)+)
    };
    // AND
    ($t:ty; $left:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidator<
            $t,
            $crate::validator_type_expr!($t; $left),
            $crate::validator_type_expr!($t; $($rest)+)
        >
    };

    // OR
    ($t:ty; $left:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<
            $t,
            $crate::validator_type_expr!($t; $left),
            $crate::validator_type_expr!($t; $($rest)+)
        >
    };

    // single validator
    ($t:ty; $v:ty) => { $v };
}

#[macro_export]
macro_rules! validated_type {
    ($t:ty, $($expr:tt)+) => {
        $crate::generic_validators::Validated<
            $t,
            $crate::validator_type_expr!($t; $($expr)+)
        >
    };
}

#[macro_export]
macro_rules! validated {
    ($value:expr, $($expr:tt)+) => {{
        let validator = $crate::validator_expr!($($expr)+);
        $crate::generic_validators::Validated::new($value, validator)
    }};
}

#[cfg(test)]
mod macro_tests {
    use crate::generic_validators::*;
    use nalgebra::Point2;
    use uom::si::f64::Length;

    #[test]
    fn test_validated_macro_scalar() {
        let value = 5.0f64;

        let manual_validator = AndValidator::new(IsPositive, IsFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, IsPositive && IsFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);

        assert!(validated_manual.set(5.).is_ok());
        assert!(validated_macro.set(5.).is_ok());

        let invalid_value = -1.0;
        assert!(
            validated!(invalid_value, IsPositive && IsFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_macro_point2() {
        let value = Point2::new(2.0, 3.0);

        let manual_validator =
            AndValidator::new(AndValidator::new(IsPositive, IsFinite), IsNotZero);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, IsPositive && IsFinite && IsNotZero).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = Point2::new(0.0, 3.0); // zero in x
        assert!(
            validated!(invalid_value, IsPositive && IsFinite && IsNotZero)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_type_macro_scalar() {
        type ManualType = Validated<f64, AndValidator<f64, IsPositive, IsFinite>>;
        let _manual: ManualType;

        type MacroType = validated_type!(f64, IsPositive && IsFinite);
        let _macro: MacroType;

        // The types compile and are identical in structure
    }

    #[test]
    fn test_validated_type_macro_point2() {
        type ManualType =
            Validated<Point2<Length>, AndValidator<Point2<Length>, IsFinite, IsPositive>>;
        let _manual: ManualType;

        type MacroType = validated_type!(Point2<Length>, IsFinite && IsPositive);
        let _macro: MacroType;
    }

    #[test]
    fn test_or_validator_scalar() {
        let value = 5.0f64;

        let manual_validator = OrValidator::new(IsPositive, IsNotZero);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, IsPositive || IsNotZero).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = -0.0; // neither positive nor non-zero fails
        assert!(
            validated!(invalid_value, IsPositive || IsNotZero)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_and_or_mixed_scalar() {
        let value = 5.0f64;

        let manual_validator = AndValidator::new(OrValidator::new(IsPositive, IsNotZero), IsFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, (IsPositive || IsNotZero) && IsFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = -0.0;
        assert!(
            validated!(invalid_value, (IsPositive || IsNotZero) && IsFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_and_or_point2() {
        let value = Point2::new(5.0, 2.0);

        let manual_validator = AndValidator::new(OrValidator::new(IsPositive, IsNotZero), IsFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, (IsPositive || IsNotZero) && IsFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = Point2::new(0.0, -3.0);
        assert!(
            validated!(invalid_value, (IsPositive || IsNotZero) && IsFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_type_and_or() {
        type ManualType =
            Validated<f64, AndValidator<f64, OrValidator<f64, IsPositive, IsNotZero>, IsFinite>>;
        let _manual: ManualType;

        type MacroType = validated_type!(f64, (IsPositive || IsNotZero) && IsFinite);
        let _macro: MacroType;

        // Compiles correctly, structure identical
    }

    #[test]
    fn test_validated_type_and_or_point2() {
        type ManualType = Validated<
            Point2<Length>,
            AndValidator<
                Point2<Length>,
                OrValidator<Point2<Length>, IsPositive, IsNotZero>,
                IsFinite,
            >,
        >;
        let _manual: ManualType;

        type MacroType = validated_type!(Point2<Length>, (IsPositive || IsNotZero) && IsFinite);
        let _macro: MacroType;
    }
}
