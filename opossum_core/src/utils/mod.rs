//! Module for additional computational capabilities
pub mod default_from_name;
pub mod file_utils;
pub mod filter_data;
pub mod geom_transformation;
pub mod griddata;
pub mod lock_ext;
pub mod math_distribution_functions;
pub mod math_utils;
pub mod test_helper;
pub mod unit_format;
pub mod uom_macros;
pub use lock_ext::LockExt;
pub use math_utils::{to_f64, try_f64_to_u8, try_f64_to_usize};
