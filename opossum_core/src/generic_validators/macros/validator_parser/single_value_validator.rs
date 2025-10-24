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
