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

/// Expands a logical validator expression into nested validator instances.
///
/// Supports parentheses, `&&`, `||`, and `!` (not) operators.
#[macro_export]
macro_rules! validator_expr {
    // parentheses first, recursive first
    (( $($inner:tt)+ )) => {
        $crate::validator_expr!($($inner)+)
    };

    (!$left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
        $crate::generic_validators::AndValidator::new(
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($left)),
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($mid))
        ),
        $crate::validator_expr!($($rest)+))
    };

    (!$left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
        $crate::generic_validators::AndValidator::new(
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($left)),
            $crate::validator_expr!($mid)
        ),
        $crate::validator_expr!($($rest)+))
    };

    ($left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
        $crate::generic_validators::AndValidator::new(
            $crate::validator_expr!($left)
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($mid)),
        ),
        $crate::validator_expr!($($rest)+))
    };

    ($left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
        $crate::generic_validators::AndValidator::new(
            $crate::validator_expr!($left),
            $crate::validator_expr!($mid)
        ),
        $crate::validator_expr!($($rest)+))
    };

    // NOT + && operator
    (! $inner:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidator::new(
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($inner)),
            $crate::validator_expr!($($rest)+)
        )
    };

    // NOT + || operator
    (! $inner:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator::new(
            $crate::generic_validators::NotValidator::new($crate::validator_expr!($inner)),
            $crate::validator_expr!($($rest)+)
        )
    };

    // NOT operator
    (! $inner:tt) => {
        $crate::generic_validators::NotValidator::new($crate::validator_expr!($inner))
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

    // single validator
    ($v:expr) => { $v };
}

/// Expands a logical validator type expression into nested validator types.
///
/// Supports parentheses, `&&`, `||`, and `!` operators.
#[macro_export]
macro_rules! validator_type_expr {
    // Parentheses: unwrap and recurse
    ($t:ty; ( $($inner:tt)+ )) => {
        $crate::validator_type_expr!($t; $($inner)+)
    };


    ($t:ty; !$left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<$t,
        $crate::generic_validators::AndValidator<$t,
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $left)>,
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $mid)>
        >,
        $crate::validator_type_expr!($t; $($rest)+)>
    };

    ($t:ty; !$left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<$t,
        $crate::generic_validators::AndValidator<$t,
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $left)>,
            $crate::validator_type_expr!($t; $mid)
        >,
        $crate::validator_type_expr!($t; $($rest)+)>
    };

    ($t:ty; $left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<$t,
        $crate::generic_validators::AndValidator<$t,
            $crate::validator_type_expr!($t; $left)
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $mid)>,
        >,
        $crate::validator_type_expr!($t; $($rest)+)>
    };

    ($t:ty; $left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<$t,
        $crate::generic_validators::AndValidator<$t,
            $crate::validator_type_expr!($t; $left),
            $crate::validator_type_expr!($t; $mid)
        >,
        $crate::validator_type_expr!($t; $($rest)+)>
    };

    // NOT + && operator
    ($t:ty; ! $inner:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidator<$t,
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $inner)>,
            $crate::validator_type_expr!($t; $($rest)+)
        >
    };

    // NOT + || operator
    ($t:ty; ! $inner:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidator<$t,
            $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $inner)>,
            $crate::validator_type_expr!($t; $($rest)+)
        >
    };

    // NOT operator
    ($t:ty; ! $inner:tt) => {
        $crate::generic_validators::NotValidator<$t,$crate::validator_type_expr!($t; $inner)>
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
    use crate::generic_validators::{
        AllFinite, AllNotZero, AllPositive, AndValidator, OrValidator, Validated,
    };
    use nalgebra::Point2;
    use uom::si::f64::Length;

    #[test]
    fn test_validated_macro_scalar() {
        let value = 5.0f64;

        let manual_validator = AndValidator::new(AllPositive, AllFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, AllPositive && AllFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);

        assert!(validated_manual.set(5.).is_ok());
        assert!(validated_macro.set(5.).is_ok());

        let invalid_value = -1.0;
        assert!(
            validated!(invalid_value, AllPositive && AllFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_macro_point2() {
        let value = Point2::new(2.0, 3.0);

        let manual_validator =
            AndValidator::new(AndValidator::new(AllPositive, AllFinite), AllNotZero);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro =
            validated!(value, AllPositive && AllFinite && AllNotZero).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = Point2::new(0.0, 3.0); // zero in x
        assert!(
            validated!(invalid_value, AllPositive && AllFinite && AllNotZero)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_type_macro_scalar() {
        type ManualType = Validated<f64, AndValidator<f64, AllPositive, AllFinite>>;
        let _manual: ManualType;

        type MacroType = validated_type!(f64, AllPositive && AllFinite);
        let _macro: MacroType;

        // The types compile and are identical in structure
    }

    #[test]
    fn test_validated_type_macro_point2() {
        type ManualType =
            Validated<Point2<Length>, AndValidator<Point2<Length>, AllFinite, AllPositive>>;
        let _manual: ManualType;

        type MacroType = validated_type!(Point2<Length>, AllFinite && AllPositive);
        let _macro: MacroType;
    }

    #[test]
    fn test_or_validator_scalar() {
        let value = 5.0f64;

        let manual_validator = OrValidator::new(AllPositive, AllNotZero);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro = validated!(value, AllPositive || AllNotZero).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = -0.0; // neither positive nor non-zero fails
        assert!(
            validated!(invalid_value, AllPositive || AllNotZero)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_and_or_mixed_scalar() {
        let value = 5.0f64;

        let manual_validator =
            AndValidator::new(OrValidator::new(AllPositive, AllNotZero), AllFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro =
            validated!(value, (AllPositive || AllNotZero) && AllFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = -0.0;
        assert!(
            validated!(invalid_value, (AllPositive || AllNotZero) && AllFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_and_or_point2() {
        let value = Point2::new(5.0, 2.0);

        let manual_validator =
            AndValidator::new(OrValidator::new(AllPositive, AllNotZero), AllFinite);
        let mut validated_manual = Validated::new(value, manual_validator).unwrap();

        let mut validated_macro =
            validated!(value, (AllPositive || AllNotZero) && AllFinite).unwrap();

        assert_eq!(validated_manual.value, validated_macro.value);
        assert!(validated_manual.set(value).is_ok());
        assert!(validated_macro.set(value).is_ok());

        let invalid_value = Point2::new(0.0, -3.0);
        assert!(
            validated!(invalid_value, (AllPositive || AllNotZero) && AllFinite)
                .unwrap_err()
                .to_string()
                .contains("Value must satisfy")
        );
    }

    #[test]
    fn test_validated_type_and_or() {
        type ManualType =
            Validated<f64, AndValidator<f64, OrValidator<f64, AllPositive, AllNotZero>, AllFinite>>;
        let _manual: ManualType;

        type MacroType = validated_type!(f64, (AllPositive || AllNotZero) && AllFinite);
        let _macro: MacroType;

        // Compiles correctly, structure identical
    }

    #[test]
    fn test_validated_type_and_or_point2() {
        type ManualType = Validated<
            Point2<Length>,
            AndValidator<
                Point2<Length>,
                OrValidator<Point2<Length>, AllPositive, AllNotZero>,
                AllFinite,
            >,
        >;
        let _manual: ManualType;

        type MacroType = validated_type!(Point2<Length>, (AllPositive || AllNotZero) && AllFinite);
        let _macro: MacroType;
    }

    #[test]
    fn test_not_validator_macro() {
        let value = -5.0f64;
        let validated_macro = validated!(value, !AllPositive).unwrap();
        assert_eq!(validated_macro.value, value);

        let invalid_value = 3.0;
        assert!(validated!(invalid_value, !AllPositive).is_err());
    }
}

#[cfg(test)]
mod macro_type_tests {
    use crate::generic_validators::{
        AllFinite, AllNormal, AllNotZero, AllPositive, AndValidator, NotValidator, OrValidator,
        Validated,
    };
    use static_assertions::assert_type_eq_all;

    #[test]
    fn test_type_expr_simple_and() {
        type Manual = AndValidator<f64, AllPositive, AllFinite>;
        type Macro = validator_type_expr!(f64; AllPositive && AllFinite);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_simple_or() {
        type Manual = OrValidator<f64, AllPositive, AllFinite>;
        type Macro = validator_type_expr!(f64; AllPositive || AllFinite);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_not_only() {
        type Manual = NotValidator<f64, AllPositive>;
        type Macro = validator_type_expr!(f64; !AllPositive);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_not_and_chain() {
        type Manual = AndValidator<f64, NotValidator<f64, AllPositive>, AllFinite>;
        type Macro = validator_type_expr!(f64; !AllPositive && AllFinite);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_and_or_chain() {
        type Manual = OrValidator<f64, AndValidator<f64, AllPositive, AllFinite>, AllNotZero>;
        type Macro = validator_type_expr!(f64; AllPositive && AllFinite || AllNotZero);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_and_or_chain_with_nots() {
        type Manual = OrValidator<
            f64,
            AndValidator<f64, NotValidator<f64, AllPositive>, NotValidator<f64, AllFinite>>,
            AndValidator<f64, AllNotZero, AllNormal>,
        >;
        type Macro =
            validator_type_expr!(f64; !AllPositive && !AllFinite || AllNotZero && AllNormal);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_parentheses_with_nots() {
        type Manual = OrValidator<
            f64,
            AndValidator<f64, NotValidator<f64, AllPositive>, AllFinite>,
            AllNotZero,
        >;
        type Macro = validator_type_expr!(f64; (!AllPositive && AllFinite) || AllNotZero);
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_type_expr_deeply_nested() {
        type Manual = AndValidator<
            f64,
            NotValidator<f64, OrValidator<f64, AllPositive, NotValidator<f64, AllFinite>>>,
            OrValidator<f64, AllNotZero, AllNormal>,
        >;
        type Macro =
            validator_type_expr!(f64; !(AllPositive || !AllFinite) && (AllNotZero || AllNormal));
        assert_type_eq_all!(Manual, Macro);
    }

    #[test]
    fn test_validated_type_macro_not_and_or() {
        type ManualType = Validated<
            f64,
            OrValidator<
                f64,
                AndValidator<f64, NotValidator<f64, AllPositive>, NotValidator<f64, AllFinite>>,
                AndValidator<f64, AllNotZero, AllNormal>,
            >,
        >;

        type MacroType =
            validated_type!(f64, !AllPositive && !AllFinite || AllNotZero && AllNormal);

        // compile-time equality check
        assert_type_eq_all!(ManualType, MacroType);
    }
}
#[cfg(test)]
mod nested_macro_tests {
    use core::f64;

    use crate::generic_validators::{
        AllFinite, AllNormal, AllNotZero, AllPositive, AndValidator, NotValidator, OrValidator,
        Validated,
    };
    use nalgebra::Point2;
    use static_assertions::assert_type_eq_all;

    // ----------------------------------------
    // 1. Combination of NOT / AND / OR with scalars
    // ----------------------------------------
    #[test]
    fn test_nested_and_or_not_scalar() {
        let value = 5.0f64;

        // Manual nested validator: !A && (B || C) && D
        let manual_validator: AndValidator<f64, _, _> = AndValidator::new(
            AndValidator::new(
                NotValidator::new(AllPositive),
                OrValidator::new(AllFinite, AllNotZero),
            ),
            AllNotZero,
        );

        // Validated API test – positive -> NotValidator fails
        let manual_res = Validated::new(value, manual_validator);
        assert!(
            manual_res.is_err(),
            "Manual validator should fail for positive value"
        );

        // Macro equivalent
        let macro_res = validated!(
            value,
            !AllPositive && (AllFinite || AllNotZero) && AllNotZero
        );
        assert!(
            macro_res.is_err(),
            "Macro validator should also fail for positive value"
        );

        // Valid case: negative finite non-zero value
        let valid_value = -3.5;
        let macro_ok = validated!(
            valid_value,
            !AllPositive && (AllFinite || AllNotZero) && AllNotZero
        );
        assert!(macro_ok.is_ok());
        assert_eq!(macro_ok.unwrap().value, valid_value);
    }

    // ----------------------------------------
    // 2. Combination with Point2 and NOT / OR / AND
    // ----------------------------------------
    #[test]
    fn test_nested_and_or_not_point2() {
        let value = Point2::new(1.0, 2.0);

        // Manual: (!A && (B || C)) && D
        let manual_validator: AndValidator<Point2<f64>, _, _> = AndValidator::new(
            AndValidator::new(
                NotValidator::new(AllPositive),
                OrValidator::new(AllFinite, AllNotZero),
            ),
            AllFinite,
        );

        let manual_res = Validated::new(value, manual_validator);
        assert!(
            manual_res.is_err(),
            "Manual validator should fail for positive point"
        );

        // Macro equivalent
        let macro_res = validated!(
            value,
            (!AllPositive && (AllFinite || AllNotZero)) && AllFinite
        );
        assert!(macro_res.is_err());

        // Valid point (negative x, finite)
        let valid_point = Point2::new(-1.0, 3.0);
        let macro_ok = validated!(
            valid_point,
            (!AllPositive && (AllFinite || AllNotZero)) && AllFinite
        );
        assert!(macro_ok.is_ok());
        assert_eq!(macro_ok.unwrap().value, valid_point);
    }

    // ----------------------------------------
    // 3. Complex nested mix with scalars
    // ----------------------------------------
    #[test]
    fn test_complex_mixed_macro_scalar() {
        let value = 4.0f64;

        // Manual equivalent: ((A || !B) && C) || D
        let manual_validator: OrValidator<f64, _, _> = OrValidator::new(
            AndValidator::new(
                OrValidator::new(AllPositive, NotValidator::new(AllNormal)),
                AllFinite,
            ),
            AllNotZero,
        );

        let manual_res = Validated::new(value, manual_validator);
        assert!(manual_res.is_ok());

        // Macro equivalent
        let macro_res = validated!(
            value,
            ((AllPositive || !AllNotZero) && AllFinite) || AllNormal
        );
        assert!(macro_res.is_ok());
        assert_eq!(macro_res.unwrap().value, value);

        // Invalid value (zero)
        let invalid_value = f64::NAN;
        let macro_err = validated!(
            invalid_value,
            ((AllPositive || !AllNotZero) && AllFinite) || AllNormal
        );
        assert!(macro_err.is_err());
    }

    // ----------------------------------------
    // 4. Complexer expression with Point2
    // ----------------------------------------
    #[test]
    fn test_complex_mixed_macro_point2() {
        let value = Point2::new(1.0, -2.0);

        // Manual: ((!A || B) && C) || D
        let manual_validator: OrValidator<Point2<f64>, _, _> = OrValidator::new(
            AndValidator::new(
                OrValidator::new(NotValidator::new(AllPositive), AllNotZero),
                AllFinite,
            ),
            AllNotZero,
        );

        let manual_res = Validated::new(value, manual_validator);
        assert!(manual_res.is_ok());

        // Macro equivalent
        let macro_res = validated!(
            value,
            ((!AllPositive || AllNotZero) && AllFinite) || AllNotZero
        );
        assert!(macro_res.is_ok());
        assert_eq!(macro_res.unwrap().value, value);

        let invalid_point = Point2::new(f64::NAN, -0.0);
        let macro_err = validated!(
            invalid_point,
            ((!AllPositive || AllNotZero) && AllFinite) || AllNotZero
        );
        assert!(macro_err.is_err());
    }

    // ----------------------------------------
    // 5. Type expression, pure compile time check
    // ----------------------------------------
    #[test]
    fn test_nested_type_macro() {
        // Nested type: ((A || !B) && C) || D
        type ManualType = Validated<
            f64,
            OrValidator<
                f64,
                AndValidator<
                    f64,
                    OrValidator<f64, AllPositive, NotValidator<f64, AllNotZero>>,
                    AllFinite,
                >,
                AllNotZero,
            >,
        >;

        type MacroType = validated_type!(
            f64,
            ((AllPositive || !AllNotZero) && AllFinite) || AllNotZero
        );

        // Kompiliertest + statische Typgleichheit
        assert_type_eq_all!(ManualType, MacroType);
    }
}
