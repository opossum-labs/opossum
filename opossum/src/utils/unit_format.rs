//! various functions for dealing with SI notation of (physical) values (e.g. prefix, etc.)
use num::Zero;
use uom::si::{
    f64::Length,
    length::{
        attometer, exameter, femtometer, gigameter, kilometer, megameter, meter, micrometer,
        millimeter, nanometer, petameter, picometer, terameter, zeptometer, zettameter,
    },
};
/// Return an SI unit prefix for a given value.
///
/// # Example
/// ```
/// use opossum::utils::unit_format::get_prefix_for_base_unit;
///
/// assert_eq!(get_prefix_for_base_unit(2.5), ""); // no prefix
/// assert_eq!(get_prefix_for_base_unit(2_500.0), "k"); // could be written as 2.5k
/// assert_eq!(get_prefix_for_base_unit(0.25), "m"); //  could be written as 250m
/// ```
#[must_use]
pub fn get_prefix_for_base_unit(base_unit_value: f64) -> String {
    let exponent = get_exponent_for_base_unit_in_e3_steps(base_unit_value);
    match exponent {
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "\u{03BC}", // greek mu as unicode code point
        -3 => "m",
        0 => "",
        3 => "k",
        6 => "M",
        9 => "G",
        12 => "T",
        15 => "P",
        18 => "E",
        21 => "Z",
        _ => "?",
    }
    .to_owned()
}
/// Get the SI prefix exponent of a given value.
///
/// # Example
/// ```
/// use opossum::utils::unit_format::get_exponent_for_base_unit_in_e3_steps;
///
/// assert_eq!(get_exponent_for_base_unit_in_e3_steps(0.0), 0);
/// assert_eq!(get_exponent_for_base_unit_in_e3_steps(0.1), -3); // could be written as 1.e-3
/// assert_eq!(get_exponent_for_base_unit_in_e3_steps(1010.0), 3); // could be written as 1.01e3
/// ```
#[must_use]
pub fn get_exponent_for_base_unit_in_e3_steps(base_unit_value: f64) -> i32 {
    if base_unit_value.is_zero() {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let mut exponent = (f64::log10(base_unit_value.abs()).floor()) as i32;
    if exponent.is_negative() {
        exponent -= 2;
    }
    (exponent / 3) * 3
}
#[must_use]
pub fn get_unit_value_as_length_with_format_by_exponent(
    val: Length,
    val_exponent_in_base_unit: i32,
) -> f64 {
    match val_exponent_in_base_unit {
        -21 => val.get::<zeptometer>(),
        -18 => val.get::<attometer>(),
        -15 => val.get::<femtometer>(),
        -12 => val.get::<picometer>(),
        -9 => val.get::<nanometer>(),
        -6 => val.get::<micrometer>(),
        -3 => val.get::<millimeter>(),
        0 => val.get::<meter>(),
        3 => val.get::<kilometer>(),
        6 => val.get::<megameter>(),
        9 => val.get::<gigameter>(),
        12 => val.get::<terameter>(),
        15 => val.get::<petameter>(),
        18 => val.get::<exameter>(),
        21 => val.get::<zettameter>(),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;

    use crate::{
        meter,
        utils::unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    };
    #[test]
    fn test_get_prefix_for_base_unit() {
        assert_eq!(
            get_prefix_for_base_unit(0.000_000_000_000_000_000_000_25),
            "?"
        );
        assert_eq!(
            get_prefix_for_base_unit(0.000_000_000_000_000_000_002_5),
            "z"
        );
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_000_000_025), "z");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_000_000_25), "z");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_000_002_5), "a");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_000_025), "a");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_000_25), "a");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_002_5), "f");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_025), "f");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_000_25), "f");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_002_5), "p");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_025), "p");
        assert_eq!(get_prefix_for_base_unit(0.000_000_000_25), "p");
        assert_eq!(get_prefix_for_base_unit(0.000_000_002_5), "n");
        assert_eq!(get_prefix_for_base_unit(0.000_000_025), "n");
        assert_eq!(get_prefix_for_base_unit(0.000_000_25), "n");
        assert_eq!(get_prefix_for_base_unit(0.000_002_5), "\u{03BC}");
        assert_eq!(get_prefix_for_base_unit(0.000_025), "\u{03BC}");
        assert_eq!(get_prefix_for_base_unit(0.000_25), "\u{03BC}");
        assert_eq!(get_prefix_for_base_unit(0.002_5), "m");
        assert_eq!(get_prefix_for_base_unit(0.025), "m");
        assert_eq!(get_prefix_for_base_unit(0.25), "m");
        assert_eq!(get_prefix_for_base_unit(0.0), "");
        assert_eq!(get_prefix_for_base_unit(2.5), "");
        assert_eq!(get_prefix_for_base_unit(25.0), "");
        assert_eq!(get_prefix_for_base_unit(250.0), "");
        assert_eq!(get_prefix_for_base_unit(2_500.0), "k");
        assert_eq!(get_prefix_for_base_unit(25_000.0), "k");
        assert_eq!(get_prefix_for_base_unit(250_000.0), "k");
        assert_eq!(get_prefix_for_base_unit(2_500_000.0), "M");
        assert_eq!(get_prefix_for_base_unit(25_000_000.0), "M");
        assert_eq!(get_prefix_for_base_unit(250_000_000.0), "M");
        assert_eq!(get_prefix_for_base_unit(2_500_000_000.0), "G");
        assert_eq!(get_prefix_for_base_unit(25_000_000_000.0), "G");
        assert_eq!(get_prefix_for_base_unit(250_000_000_000.0), "G");
        assert_eq!(get_prefix_for_base_unit(2_500_000_000_000.0), "T");
        assert_eq!(get_prefix_for_base_unit(25_000_000_000_000.0), "T");
        assert_eq!(get_prefix_for_base_unit(250_000_000_000_000.0), "T");
        assert_eq!(get_prefix_for_base_unit(2_500_000_000_000_000.0), "P");
        assert_eq!(get_prefix_for_base_unit(25_000_000_000_000_000.0), "P");
        assert_eq!(get_prefix_for_base_unit(250_000_000_000_000_000.0), "P");
        assert_eq!(get_prefix_for_base_unit(2_500_000_000_000_000_000.0), "E");
        assert_eq!(get_prefix_for_base_unit(25_000_000_000_000_000_000.0), "E");
        assert_eq!(get_prefix_for_base_unit(250_000_000_000_000_000_000.0), "E");
        assert_eq!(
            get_prefix_for_base_unit(2_500_000_000_000_000_000_000.0),
            "Z"
        );
        assert_eq!(
            get_prefix_for_base_unit(25_000_000_000_000_000_000_000.0),
            "Z"
        );
        assert_eq!(
            get_prefix_for_base_unit(250_000_000_000_000_000_000_000.0),
            "Z"
        );
        assert_eq!(
            get_prefix_for_base_unit(2_500_000_000_000_000_000_000_000.0),
            "?"
        );
    }
    #[test]
    fn test_get_exponent_for_base_unit_in_e3_steps() {
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(0.0), 0);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(0.1), -3);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(-0.1), -3);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(101.0), 0);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(-101.0), 0);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(1010.0), 3);
        assert_eq!(get_exponent_for_base_unit_in_e3_steps(-1010.0), 3);
    }
    #[test]
    fn test_get_unit_value_as_length_with_format_by_exponent() {
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 21),
            0.000_000_000_000_000_000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 18),
            0.000_000_000_000_000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 15),
            0.000_000_000_000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 12),
            0.000_000_000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 9),
            0.000_000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 6),
            0.000_001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 3),
            0.001_234
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 0),
            1.234
        );
        assert!(get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 1).is_nan());
        assert!(get_unit_value_as_length_with_format_by_exponent(meter!(1.234), 2).is_nan());
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -3),
            1_234.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -6),
            1_234_000.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -9),
            1_234_000_000.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -12),
            1_234_000_000_000.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -15),
            1_234_000_000_000_000.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -18),
            1_234_000_000_000_000_000.
        );
        assert_relative_eq!(
            get_unit_value_as_length_with_format_by_exponent(meter!(1.234), -21),
            1_234_000_000_000_000_000_000.
        );
    }
}
