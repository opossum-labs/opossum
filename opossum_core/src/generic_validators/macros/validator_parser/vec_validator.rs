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
