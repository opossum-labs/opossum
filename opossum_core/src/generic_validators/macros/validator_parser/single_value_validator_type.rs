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

    // generic validator type like AllInRange<Angle>
    ($t:ty; $v:ident::< $($args:ty),+ >) => {
        $v::<$($args),+>
    };

    // single validator
    ($t:ty; $v:ty) => { $v };
}
