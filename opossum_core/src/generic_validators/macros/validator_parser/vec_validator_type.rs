/// Expands a logical validator type expression into nested validator types.
///
/// Supports parentheses, `&&`, `||`, and `!` operators.
#[macro_export]
macro_rules! validator_vec_type_expr {
    // Parentheses: unwrap and recurse
    ($t:ty; ( $($inner:tt)+ )) => {
        $crate::validator_vec_type_expr!($t; $($inner)+)
    };


    ($t:ty; !$left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<$t,
        $crate::generic_validators::AndValidatorVec<$t,
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $left)>,
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $mid)>
        >,
        $crate::validator_vec_type_expr!($t; $($rest)+)>
    };

    ($t:ty; !$left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<$t,
        $crate::generic_validators::AndValidatorVec<$t,
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $left)>,
            $crate::validator_vec_type_expr!($t; $mid)
        >,
        $crate::validator_vec_type_expr!($t; $($rest)+)>
    };

    ($t:ty; $left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<$t,
        $crate::generic_validators::AndValidatorVec<$t,
            $crate::validator_vec_type_expr!($t; $left)
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $mid)>,
        >,
        $crate::validator_vec_type_expr!($t; $($rest)+)>
    };

    ($t:ty; $left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<$t,
        $crate::generic_validators::AndValidatorVec<$t,
            $crate::validator_vec_type_expr!($t; $left),
            $crate::validator_vec_type_expr!($t; $mid)
        >,
        $crate::validator_vec_type_expr!($t; $($rest)+)>
    };

    // NOT + && operator
    ($t:ty; ! $inner:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidatorVec<$t,
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $inner)>,
            $crate::validator_vec_type_expr!($t; $($rest)+)
        >
    };

    // NOT + || operator
    ($t:ty; ! $inner:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<$t,
            $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $inner)>,
            $crate::validator_vec_type_expr!($t; $($rest)+)
        >
    };

    // NOT operator
    ($t:ty; ! $inner:tt) => {
        $crate::generic_validators::NotValidatorVec<$t,$crate::validator_vec_type_expr!($t; $inner)>
    };

    // AND
    ($t:ty; $left:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidatorVec<
            $t,
            $crate::validator_vec_type_expr!($t; $left),
            $crate::validator_vec_type_expr!($t; $($rest)+)
        >
    };

    // OR
    ($t:ty; $left:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec<
            $t,
            $crate::validator_vec_type_expr!($t; $left),
            $crate::validator_vec_type_expr!($t; $($rest)+)
        >
    };

    // generic validator type like AllInRange<Angle>
    ($t:ty; $v:ident::< $($args:ty),+ >) => {
        $v::<$($args),+>
    };

    // single validator
    ($t:ty; $v:ty) => { $v };
}
