
#[macro_export]
macro_rules! impl_validator {   
    ($validator:path, $func:expr,  $t:ty) => {
        impl crate::generic_validators::Validate<$t> for $validator {
            fn validate(&self, value: &$t) -> crate::error::OpmResult<()> {
                if $func(&self, &value) {
                    Ok(())
                } else {
                    Err(crate::error::OpossumError::Other(format!("Value must satisfy {}", stringify!($func))))
                }
            }
        }
    };
}


#[macro_export]
macro_rules! validator_expr {
    // Klammern zuerst, rekursiv
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
    
    // Einzelner Validator
    ($v:expr) => { $v };
}

#[macro_export]
macro_rules! validator_type_expr {
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

    // Einzelner Validator
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

// #[macro_export]
// macro_rules! validated {
//     // Mehrere Validatoren als Liste mit Komma
//     ($value:expr, $first:expr $(, $rest:expr)*) => {{
//         let combined_validator = {
//             let mut v = $first;
//             $(
//                 v = $crate::generic_validators::AndValidator::new(v, $rest);
//             )*
//             v
//         };
//         $crate::generic_validators::Validated::new($value, combined_validator)
//     }};

//     // Einzelner Validator
//     ($value:expr, $validator:expr) => {{
//         $crate::generic_validators::Validated::new($value, $validator)
//     }};
// }

// macro_rules! validated {
//     // === Mehrere Validatoren mit &&
//     ($value:expr, $left:ident && $right:ident) => {{
//         let validator = $crate::generic_validators::AndValidator::<_, $left, $right>::new(
//             $left::default(),
//             $right::default(),
//         );
//         $crate::validated::Validated::new($value, validator)
//     }};

//     // Rekursiv für 3+ Validatoren: A && B && C && D …
//     ($value:expr, $left:ident && $($rest:tt)+) => {{
//         let next = validated!($value, $($rest)+)?;
//         let validator = $crate::generic_validators::AndValidator::<_, $left, _>::new(
//             $left::default(),
//             next.validator().clone(),
//         );
//         $crate::validated::Validated::new($value, validator)
//     }};

//     // === Nur ein einzelner Validator
//     ($value:expr, $v:ident) => {{
//         let validator = $v::default();
//         $crate::validated::Validated::new($value, validator)
//     }};
// }
