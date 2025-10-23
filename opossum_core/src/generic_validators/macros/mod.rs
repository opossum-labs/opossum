mod logical_expression;
mod validated_vec;
mod validated;

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
macro_rules! impl_vec_validator {
    ($validator:path, $func:expr,  $t:ty) => {
        impl $crate::generic_validators::ValidateVec<$t> for $validator {
            fn validate_vec(&self, values: &Vec<$t>) -> $crate::error::OpmResult<()> {
                if $func(&self, &values) {
                    Ok(())
                } else {
                    Err($crate::error::OpossumError::Other(format!(
                        "All values must satisfy {}",
                        stringify!($func)
                    )))
                }
            }
        }
    };
}