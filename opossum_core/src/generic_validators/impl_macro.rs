
#[macro_export]
macro_rules! impl_validator {   
    // ($validator:ident, $func:expr, $(Point2<$t:ty>), *) => {
    //     $(
    //     impl crate::generic_validators::Validate<nalgebra::Point2<$t>> for $validator {
    //         fn validate(&self, value: &nalgebra::Point2<$t>) -> crate::error::OpmResult<()> {
    //             if $func(&value.x) && $func(&value.y) {
    //                 Ok(())
    //             } else {
    //                 Err(crate::error::OpossumError::Other(format!("Value must satisfy {}", stringify!($func))))
    //             }
    //         }
    //     }
    // )*
    // };
    // // Für einfache Typen
    ($validator:ident, $func:expr,  $($t:ty),*) => {
        $(
            impl crate::generic_validators::Validate<$t> for $validator {
                fn validate(&self, value: &$t) -> crate::error::OpmResult<()> {
                    if $func(&value) {
                        Ok(())
                    } else {
                        Err(crate::error::OpossumError::Other(format!("Value must satisfy {}", stringify!($func))))
                    }
                }
            }
        )*
    };

}
// macro_rules! impl_validate_numeric {
//     ($validator:ident, $check:expr) => {
//         impl Validate<f64> for $validator {
//             fn validate(&self, value: &f64) -> OpmResult<()> {
//                 $check(*value)
//             }
//         }

//         impl Validate<Length> for $validator {
//             fn validate(&self, value: &Length) -> OpmResult<()> {
//                 $check(*value)
//             }
//         }

//         impl Validate<Point2<f64>> for $validator {
//             fn validate(&self, value: &Point2<f64>) -> OpmResult<()> {
//                 $check(value.x)?;
//                 $check(value.y)
//             }
//         }

//         impl Validate<Point2<Length>> for $validator {
//             fn validate(&self, value: &Point2<Length>) -> OpmResult<()> {
//                 $check(value.x)?;
//                 $check(value.y)
//             }
//         }
//     };
// }
