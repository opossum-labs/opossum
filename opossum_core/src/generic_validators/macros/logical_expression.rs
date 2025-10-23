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

    // generic validator type like AllInRange<Angle>
    ($t:ty; $v:ident::< $($args:ty),+ >) => {
        $v::<$($args),+>
    };

    // single validator
    ($t:ty; $v:ty) => { $v };
}


#[macro_export]
macro_rules! validator_vec_expr {
    // parentheses first, recursive first
    (( $($inner:tt)+ )) => {
        $crate::validator_vec_expr!($($inner)+)
    };

    (!$left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
        $crate::generic_validators::AndValidatorVec::new(
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($left)),
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($mid))
        ),
        $crate::validator_vec_expr!($($rest)+))
    };

    (!$left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
        $crate::generic_validators::AndValidatorVec::new(
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($left)),
            $crate::validator_vec_expr!($mid)
        ),
        $crate::validator_vec_expr!($($rest)+))
    };

    ($left:tt && !$mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
        $crate::generic_validators::AndValidatorVec::new(
            $crate::validator_vec_expr!($left)
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($mid)),
        ),
        $crate::validator_vec_expr!($($rest)+))
    };

    ($left:tt && $mid:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
        $crate::generic_validators::AndValidatorVec::new(
            $crate::validator_vec_expr!($left),
            $crate::validator_vec_expr!($mid)
        ),
        $crate::validator_vec_expr!($($rest)+))
    };

    // NOT + && operator
    (! $inner:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidatorVec::new(
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($inner)),
            $crate::validator_vec_expr!($($rest)+)
        )
    };

    // NOT + || operator
    (! $inner:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
            $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($inner)),
            $crate::validator_vec_expr!($($rest)+)
        )
    };

    // NOT operator
    (! $inner:tt) => {
        $crate::generic_validators::NotValidatorVec::new($crate::validator_vec_expr!($inner))
    };

    // AND
    ($left:tt && $($rest:tt)+) => {
        $crate::generic_validators::AndValidatorVec::new(
            $crate::validator_vec_expr!($left),
            $crate::validator_vec_expr!($($rest)+)
        )
    };

    // OR
    ($left:tt || $($rest:tt)+) => {
        $crate::generic_validators::OrValidatorVec::new(
            $crate::validator_vec_expr!($left),
            $crate::validator_vec_expr!($($rest)+)
        )
    };

    // single validator
    ($v:expr) => { $v };
}

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