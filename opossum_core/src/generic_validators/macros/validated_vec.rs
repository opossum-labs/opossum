#[macro_export]
macro_rules! validated_vec {
    ($value:expr, $($rest:tt)+) => {
        $crate::validated_vec!(@split $value, [] $($rest)+)
    };

    (@split $value:expr, [$($accum:tt)*] , $($tail:tt)+) => {{
        let elem_validator = $crate::validator_expr!($($accum)*);
        let cont_validator = $crate::validator_vec_expr!($($tail)+);
        crate::generic_validators::ValidatedVec::new(
            $value,
            elem_validator,
            cont_validator
        )
    }
    };

    (@split $value:expr, [$($accum:tt)*] $head:tt $($tail:tt)*) => {
        $crate::validated_vec!(@split $value, [$($accum)* $head] $($tail)*)
    };
}

#[macro_export]
macro_rules! validated_vec_type {
    (Vec< $inner:ty >, $($rest:tt)+) => {
        $crate::validated_vec_type!(@split $inner [] $($rest)+)
    };

    (@split $inner:ty [$($accum:tt)*] , $($tail:tt)+) => {
        $crate::generic_validators::ValidatedVec<
            $inner,
            $crate::validator_type_expr!($inner; $($accum)*),
            $crate::validator_vec_type_expr!($inner; $($tail)+)
        >
    };

    (@split $inner:ty [$($accum:tt)*] $head:tt $($tail:tt)*) => {
        $crate::validated_vec_type!(@split $inner [$($accum)* $head] $($tail)*)
    };
}


