pub mod input_components;
use approx::relative_eq;
use dioxus::prelude::*;
use opossum_core::utils::geom_transformation::{RotationAxis, TranslationAxis};
use regex::Regex;
use std::{fmt::Display, str::FromStr};
use strum::IntoEnumIterator;

const EXP_NOTATION_MIN: i32 = -30;
const EXP_NOTATION_MAX: i32 = 30;

pub trait IntoInputData<T, D, B: 'static>: Into<InputParam>
where
    D: Into<B> + Clone + 'static,
    T: Clone + FromStr + 'static,
    Self: IntoEnumIterator + IntoInputDataStrings<D> + Copy + 'static,
{
    fn setter_from_obj(&self) -> impl FnMut(&mut D, T);

    fn create_callback(
        &self,
        mut obj: D,
        handler: EventHandler<B>,
    ) -> EventHandler<Event<FormData>> {
        let this = *self;
        EventHandler::new(move |e: Event<FormData>| {
            if let Some(value) = this.parse_value(e) {
                let mut setter = this.setter_from_obj();
                setter(&mut obj, value);
                handler.call(obj.clone().into());
            }
        })
    }

    fn create_callback_str(&self, mut obj: D, handler: EventHandler<B>) -> EventHandler<String> {
        let this = *self;
        EventHandler::new(move |val_str: String| {
            if let Ok(value) = val_str.parse::<T>() {
                let mut setter = this.setter_from_obj();
                setter(&mut obj, value);
                handler.call(obj.clone().into());
            }
        })
    }

    fn parse_value(&self, e: Event<FormData>) -> Option<T> {
        let e_value = e.value();
        e_value.parse::<T>().ok()
    }

    fn to_input_data(&self, obj: D, handler: EventHandler<B>) -> InputData {
        let value_str = self.create_value_string(&obj);
        InputData::new(
            Into::<InputParam>::into(*self),
            self.create_id_string().as_str(),
            self.create_callback(obj.clone(), handler), // Für Legacy/Events
            self.create_callback_str(obj, handler),     // Für Flushable
            value_str,
        )
    }

    fn to_input_data_vec(obj: &D, handler: EventHandler<B>) -> Vec<InputData> {
        let mut input_data = Vec::<InputData>::new();
        for enum_variant in Self::iter() {
            input_data.push(enum_variant.to_input_data(obj.clone(), handler));
        }
        input_data
    }
}

pub trait IntoInputDataStrings<D> {
    fn create_value_string(&self, obj: &D) -> String;
    fn create_id_string(&self) -> String;
}

#[derive(Clone, PartialEq, Eq)]
pub enum InputParam {
    Usize(String),
    U8(String),
    F64(String),
    Selection(String, Vec<(bool, String)>),
    Bool(String),
    SIUnit(String, String), // (Label, BaseUnit)
    FilePath(String, String),
}

impl InputParam {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Usize(label)
            | Self::U8(label)
            | Self::Selection(label, _)
            | Self::F64(label)
            | Self::Bool(label)
            | Self::SIUnit(label, _)
            | Self::FilePath(label, _) => label.clone(),
        }
    }
    #[must_use]
    pub const fn rtype(&self) -> &'static str {
        match self {
            Self::Usize(_) | Self::U8(_) | Self::F64(_) | Self::SIUnit(_, _) => "number",
            Self::Bool(_) => "checkbox",
            Self::FilePath(_, _) => "file",
            Self::Selection(_, _) => "select",
        }
    }

    #[must_use]
    pub fn id_str(&self) -> String {
        let mut label = self.label();
        label.retain(|c| !c.is_whitespace());
        label
    }
}

impl From<TranslationAxis> for InputParam {
    fn from(axis: TranslationAxis) -> Self {
        match axis {
            TranslationAxis::X => Self::SIUnit("X translation".into(), "m".into()),
            TranslationAxis::Y => Self::SIUnit("Y translation".into(), "m".into()),
            TranslationAxis::Z => Self::SIUnit("Z translation".into(), "m".into()),
        }
    }
}

impl From<RotationAxis> for InputParam {
    fn from(axis: RotationAxis) -> Self {
        match axis {
            RotationAxis::Roll => Self::SIUnit("Roll".into(), "deg".into()),
            RotationAxis::Pitch => Self::SIUnit("Pitch".into(), "deg".into()),
            RotationAxis::Yaw => Self::SIUnit("Yaw".into(), "deg".into()),
        }
    }
}

pub fn select_options_from_enum_iterator<T: IntoEnumIterator + Display>(
    active_selection: &T,
    exclude: Option<&[&T]>,
) -> Vec<(bool, String)> {
    let mut options = Vec::<(bool, String)>::new();

    for enum_variant in T::iter() {
        if std::mem::discriminant(&enum_variant) == std::mem::discriminant(active_selection) {
            options.push((true, format!("{enum_variant}")));
        } else if !exclude.is_some_and(|ex| {
            ex.iter()
                .any(|e| std::mem::discriminant(*e) == std::mem::discriminant(&enum_variant))
        }) {
            options.push((false, format!("{enum_variant}")));
        }
    }
    options
}

#[derive(Clone, PartialEq)]
pub struct InputData {
    pub value: String,
    pub id: String,
    pub input_param: InputParam,
    pub callback: EventHandler<Event<FormData>>,
    pub callback_str: EventHandler<String>,
    pub readonly: bool,
}

impl InputData {
    pub fn new(
        input_param: InputParam,
        id_str_add_on: &str,
        callback: EventHandler<Event<FormData>>,
        callback_str: EventHandler<String>,
        value: String,
    ) -> Self {
        Self {
            value,
            id: format!("{}{}", id_str_add_on, input_param.id_str()),
            input_param,
            callback,
            callback_str,
            readonly: false,
        }
    }
}

/// Parses a numeric string together with an SI prefix into a floating-point value.
///
/// The numeric string may use either `.` or `,` as the decimal separator.
/// The parsed value is scaled according to the given SI prefix.
///
/// # Arguments
///
/// * `num_str` - The numeric part of the input (e.g. `"1.23"` or `"4,7"`).
/// * `prefix_str` - The SI prefix as a string (e.g. `"m"`, `"k"`, `"µ"`).
///
/// # Returns
///
/// `Some(f64)` containing the scaled value if parsing succeeds, or `None` if
/// the numeric string is invalid.
pub fn parse_si_number(num_str: &str, prefix_str: &str, reciprocal: bool) -> Option<f64> {
    let factor = si_prefix_to_exponent(prefix_str, reciprocal);

    let normalized = num_str.replace(',', ".");
    normalized
        .parse::<f64>()
        .map_or(None, |parsed| Some(parsed * 10f64.powi(factor)))
}

/// Checks whether an input string resembles a valid, permissive unit input.
///
/// This function is designed for interactive user input and allows partially
/// entered numbers (e.g. `"1."`, `"-"`, `"3e"`). It validates the numeric part
/// loosely
///
/// # Arguments
///
/// * `input` - The full user input string (number and unit).
///
/// # Returns
///
/// `true` if the input is syntactically acceptable in a permissive context,
/// otherwise `false`.
fn is_permissive_unit_input(input: &str) -> bool {
    let re = Regex::new(
        r"^\s*(?P<num>[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?)\s*(?P<unit>[a-zA-Zµ]*)?\s*$",
    )
    .unwrap();

    let Some(caps) = re.captures(input) else {
        return false;
    };

    let num = caps.name("num").map_or("", |m| m.as_str());

    let num_re = Regex::new(r"^[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?$").unwrap();
    num_re.is_match(num)
}

fn is_permissive_exp_input(input: &str) -> bool {
    let regex = Regex::new(r"^[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?$").unwrap();
    let trimmed = input.trim();
    regex.is_match(trimmed)
}

pub fn parse_exp_input_strict(input: &str) -> Result<String, ()> {
    let regex = Regex::new(r"^[+-]?(?:(\d+([.,]\d*)?)|([.,]\d+))([eE][+-]?\d+)?$").unwrap();
    let trimmed = input.trim().replace(',', ".");
    if regex.is_match(&trimmed) {
        return Ok(trimmed);
    }
    Err(())
}

/// Strictly parses a unit input consisting of a numeric value and an SI-prefixed unit.
///
/// Unlike permissive parsing, this function requires a complete and valid
/// numeric representation observed and rejects malformed inputs. The unit
/// must match the given base unit, optionally preceded by a single valid
/// SI prefix.
///
/// # Arguments
///
/// * `input` - The input string containing a value and unit.
/// * `base_unit` - The expected base unit (e.g. `"A"`, `"Ω"`).
///
/// # Returns
///
/// `Ok((value, prefix))` where `value` is the numeric string and `prefix`
/// is the extracted SI prefix (or empty if none is present).
/// Returns `Err(())` if the input does not strictly conform to the expected format.
pub fn parse_unit_input_strict(input: &str, base_unit: &str) -> Result<(String, String), ()> {
    let regex = Regex::new(
        r"^(?P<value>[+-]?(?:\d*(?:[.,]\d*)?|[.,]\d+)(?:[eE][+-]?\d*)?)\s*(?P<unit>[\p{L}°µ]+)$",
    )
    .unwrap();

    let valid_prefixes = [
        'q', 'r', 'y', 'z', 'a', 'f', 'p', 'n', 'µ', 'u', 'm', 'k', 'M', 'G', 'T', 'P', 'E', 'Z',
        'Y', 'R', 'Q',
    ];

    let caps = regex.captures(input).ok_or(())?;

    let value = caps.name("value").unwrap().as_str().replace(',', ".");
    if value.is_empty() {
        return Err(());
    }

    let unit = caps.name("unit").unwrap().as_str();
    if unit == base_unit {
        return Ok((value, String::new()));
    }

    if let Some(prefix_part) = unit.strip_suffix(base_unit)
        && prefix_part.chars().count() == 1
    {
        let prefix_char = prefix_part.chars().next().unwrap();
        if valid_prefixes.contains(&prefix_char) {
            return Ok((value, prefix_char.to_string()));
        }
    }

    Err(())
}

/// Formats a floating-point value using SI engineering notation.
///
/// The number is expressed as a mantissa multiplied by a power of ten that
/// corresponds to an SI prefix (powers of 10³). Infinite values are rendered
/// as `"∞"`.
///
/// This function is intended for human-readable display of physical quantities.
///
/// # Arguments
///
/// * `x` - The value to format.
///
/// # Returns
///
/// A `String` containing the formatted value and SI prefix.
pub fn format_si_notation(x: f64, reciprocal: bool) -> String {
    if x.is_infinite() {
        return "∞".into();
    }

    let (mantissa, exponent) = get_mantissa_and_exponent(x);

    let prefix = if reciprocal {
        si_prefix_from_exponent(-exponent)
    } else {
        si_prefix_from_exponent(exponent)
    };

    if relative_eq!(mantissa, 0.0) {
        return "0.0 ".into();
    }

    format!("{mantissa} {prefix}")
}

/// Formats a floating-point value in scientific notation with an exponent.
///
/// The value is expressed as a mantissa multiplied by 10 raised to an integer
/// exponent, where the exponent changes in steps of 3. Infinite values are rendered as `"∞"`.
/// This function is intended for display of numeric values where a fixed exponent
/// format is preferred over SI engineering notation.
pub fn format_exp_number_notation(x: f64) -> String {
    if x.is_infinite() {
        return "∞".into();
    }

    let (mantissa, exponent) = get_mantissa_and_exponent(x);

    if relative_eq!(mantissa, 0.0) {
        return "0.0 ".into();
    }

    if exponent == 0 {
        mantissa.to_string()
    } else {
        format!("{mantissa}e{exponent}")
    }
}

/// Formats a floating-point value with an SI prefix and a base unit.
///
/// The value is converted to engineering notation with an appropriate SI prefix,
/// and the specified base unit is appended. The `reciprocal` flag indicates
/// whether the SI prefix should be inverted (e.g. "m" becomes "k").
/// Infinite values are rendered as `"∞"` followed by the base unit.
pub fn format_si_with_base_unit(value: f64, base_unit: &str, reciprocal: bool) -> String {
    format!("{}{}", format_si_notation(value, reciprocal), base_unit,)
}

/// Splits a floating-point value into an engineering mantissa and exponent.
///
/// The exponent is always a multiple of three, suitable for use with SI prefixes.
/// The mantissa retains the original sign of the input.
///
/// # Arguments
///
/// * `x` - The value to decompose.
///
/// # Returns
///
/// A tuple `(mantissa, exponent)` where `x = mantissa × 10^exponent`.
fn get_mantissa_and_exponent(x: f64) -> (f64, i32) {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();

    let exp3 = get_exponent(x_abs);
    if exp3 < EXP_NOTATION_MIN {
        (
            normalize_f64_noise(sign * (x_abs / 10f64.powi(EXP_NOTATION_MIN))),
            EXP_NOTATION_MIN,
        )
    } else if exp3 > EXP_NOTATION_MAX {
        (
            normalize_f64_noise(sign * (x_abs / 10f64.powi(EXP_NOTATION_MAX))),
            EXP_NOTATION_MAX,
        )
    } else {
        (normalize_f64_noise(sign * (x_abs / 10f64.powi(exp3))), exp3)
    }
}

fn normalize_f64_noise(v: f64) -> f64 {
    if v == 0.0 {
        return 0.0;
    }

    let exp = v.abs().log10().floor();
    let scale = 10f64.powf(14.0 - exp);

    (v * scale).round() / scale
}

/// Computes the engineering exponent (multiple of three) for a positive value.
///
/// The returned exponent corresponds to the largest power of ten divisible
/// by three that does not exceed the magnitude of the input.
///
/// # Arguments
///
/// * `x_abs` - The absolute value of the number.
///
/// # Returns
///
/// The engineering exponent as a multiple of three.
fn get_exponent(x_abs: f64) -> i32 {
    if x_abs < 1e-60 {
        return 0;
    }
    let exp10 = x_abs.log10();
    #[allow(clippy::cast_possible_truncation)]
    let exp3 = (exp10 / 3.0).floor() as i32;
    exp3 * 3
}

/// Converts an engineering exponent into its corresponding SI prefix.
///
/// Only exponents that map to standard SI prefixes are supported. Unsupported
/// exponents result in an empty string.
///
/// # Arguments
///
/// * `exponent` - A power-of-ten exponent (multiple of three).
///
/// # Returns
///
/// A `String` containing the SI prefix, or an empty string if none applies.
fn si_prefix_from_exponent(exponent: i32) -> String {
    let prefix = match exponent {
        -30 => "q",
        -27 => "r",
        -24 => "y",
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "µ",
        -3 => "m",
        3 => "k",
        6 => "M",
        9 => "G",
        12 => "T",
        15 => "P",
        18 => "E",
        21 => "Z",
        24 => "Y",
        27 => "R",
        30 => "Q",
        _ => "",
    };
    prefix.into()
}

/// Converts an SI prefix into its corresponding power-of-ten exponent.
///
/// Both `"µ"` and `"u"` are accepted for micro. Unknown prefixes map to `0`.
///
/// # Arguments
///
/// * `prefix` - The SI prefix string.
///
/// # Returns
///
/// The corresponding exponent as a power of ten.
fn si_prefix_to_exponent(prefix: &str, reciprocal: bool) -> i32 {
    let exp = match prefix {
        "q" => -30,
        "r" => -27,
        "y" => -24,
        "z" => -21,
        "a" => -18,
        "f" => -15,
        "p" => -12,
        "n" => -9,
        "µ" | "u" => -6,
        "m" => -3,
        "k" => 3,
        "M" => 6,
        "G" => 9,
        "T" => 12,
        "P" => 15,
        "E" => 18,
        "Z" => 21,
        "Y" => 24,
        "R" => 27,
        "Q" => 30,
        _ => 0,
    };
    if reciprocal { -exp } else { exp }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_si_prefix_from_exponent_all_valid() {
        let cases = vec![
            (-30, "q"),
            (-27, "r"),
            (-24, "y"),
            (-21, "z"),
            (-18, "a"),
            (-15, "f"),
            (-12, "p"),
            (-9, "n"),
            (-6, "µ"),
            (-3, "m"),
            (3, "k"),
            (6, "M"),
            (9, "G"),
            (12, "T"),
            (15, "P"),
            (18, "E"),
            (21, "Z"),
            (24, "Y"),
            (27, "R"),
            (30, "Q"),
        ];

        for (exp, expected) in cases {
            assert_eq!(
                si_prefix_from_exponent(exp),
                expected,
                "Exponent {} should map to '{}'",
                exp,
                expected
            );
        }
    }

    #[test]
    fn test_si_prefix_from_exponent_invalid_exponents() {
        let invalid_exponents = vec![0, 1, 2, -1, -2, 4, 5, -31, 31, 100, -100];

        for exp in invalid_exponents {
            assert_eq!(
                si_prefix_from_exponent(exp),
                "",
                "Exponent {} should return empty string",
                exp
            );
        }
    }

    #[test]
    fn test_si_prefix_from_exponent_edge_cases() {
        let edge_cases = vec![
            (-33, ""), // below supported range
            (-29, ""), // between supported exponents
            (2, ""),   // just below first positive prefix
            (32, ""),  // above supported range
        ];

        for (exp, expected) in edge_cases {
            assert_eq!(
                si_prefix_from_exponent(exp),
                expected,
                "Exponent {} edge case should return '{}'",
                exp,
                expected
            );
        }
    }

    #[test]
    fn test_valid_inputs_is_permissive_unit_input() {
        let valid_inputs = vec![
            "123",     // simple number
            "+123",    // positive sign
            "-123",    // negative number
            "123.45",  // decimal number
            "-123.45", // negative decimal
            "1e10",    // scientific notation
            "-1E-10",  // scientific notation with sign
            "  42  ",  // leading and trailing spaces
            "42kg",    // number with unit
            "3.14 m",  // number with unit and space
            "2,718µs", // comma as decimal separator and µ unit
        ];

        for input in valid_inputs {
            assert!(
                is_permissive_unit_input(input),
                "Input '{}' should be valid",
                input
            );
        }
    }

    #[test]
    fn test_invalid_inputs_is_permissive_unit_input() {
        let invalid_inputs = vec![
            "12.3.4",    // invalid decimal format
            "1e10.5",    // invalid scientific notation
            "123..45kg", // double dot
            "1,2,3",     // multiple commas
            "12 34",     // space inside number
            "--123",     // double minus
            "++123",     // double plus
        ];

        for input in invalid_inputs {
            assert!(
                !is_permissive_unit_input(input),
                "Input '{}' should be invalid",
                input
            );
        }
    }

    #[test]
    fn test_standard_prefixes() {
        let cases = vec![
            ("q", false, -30),
            ("r", false, -27),
            ("y", false, -24),
            ("z", false, -21),
            ("a", false, -18),
            ("f", false, -15),
            ("p", false, -12),
            ("n", false, -9),
            ("µ", false, -6),
            ("u", false, -6),
            ("m", false, -3),
            ("k", false, 3),
            ("M", false, 6),
            ("G", false, 9),
            ("T", false, 12),
            ("P", false, 15),
            ("E", false, 18),
            ("Z", false, 21),
            ("Y", false, 24),
            ("R", false, 27),
            ("Q", false, 30),
        ];

        for (prefix, reciprocal, expected) in cases {
            assert_eq!(
                si_prefix_to_exponent(prefix, reciprocal),
                expected,
                "Prefix '{}' with reciprocal={} failed",
                prefix,
                reciprocal
            );
        }
    }

    #[test]
    fn test_valid_without_prefix_parse_unit_input_strict() {
        let base_unit = "A";
        let cases = vec![
            ("123A", "123", ""),
            ("+123A", "+123", ""),
            ("-123A", "-123", ""),
            ("0.456A", "0.456", ""),
            ("7,89A", "7,89", ""),
            ("1e3A", "1e3", ""),
            ("-2E-3A", "-2E-3", ""),
        ];

        for (input, expected_value, expected_prefix) in cases {
            let result = parse_unit_input_strict(input, base_unit).unwrap();
            assert_eq!(
                result.0,
                expected_value.replace(",", "."),
                "Input '{}'",
                input
            );
            assert_eq!(result.1, expected_prefix, "Input '{}'", input);
        }
    }

    #[test]
    fn test_valid_with_prefix_parse_unit_input_strict() {
        let base_unit = "A";
        let cases = vec![
            ("3.5kA", "3.5", "k"),
            ("-1.2mA", "-1.2", "m"),
            ("7µA", "7", "µ"),
            ("9uA", "9", "u"),
            ("1GA", "1", "G"),
        ];

        for (input, expected_value, expected_prefix) in cases {
            let result = parse_unit_input_strict(input, base_unit).unwrap();
            assert_eq!(result.0, expected_value, "Input '{}'", input);
            assert_eq!(result.1, expected_prefix, "Input '{}'", input);
        }
    }

    #[test]
    fn parse_unit_input_strict_valid_without_prefix() {
        let cases = vec![
            ("123A", "123", "A"),
            ("+123A", "+123", "A"),
            ("-123A", "-123", "A"),
            ("0.456A", "0.456", "A"),
            ("25°C", "25", "°C"),
            ("220Ω", "220", "Ω"),
        ];

        for (input, expected_value, base_unit) in cases {
            let result = parse_unit_input_strict(input, base_unit).unwrap();
            assert_eq!(result.0, expected_value, "Input '{}'", input);
            assert_eq!(result.1, "", "Input '{}'", input); // no prefix
        }
    }

    #[test]
    fn parse_unit_input_strict_valid_with_prefix() {
        let cases = vec![
            ("3.5kA", "3.5", "A", "k"),
            ("-1.2mA", "-1.2", "A", "m"),
            ("7µA", "7", "A", "µ"),
            ("9uA", "9", "A", "u"),
            ("1GA", "1", "A", "G"),
            ("2.5mΩ", "2.5", "Ω", "m"),
            ("1k°C", "1", "°C", "k"),
        ];

        for (input, expected_value, base_unit, expected_prefix) in cases {
            let result = parse_unit_input_strict(input, base_unit).unwrap();
            assert_eq!(result.0, expected_value, "Input '{}'", input);
            assert_eq!(result.1, expected_prefix, "Input '{}'", input);
        }
    }

    #[test]
    fn parse_unit_input_strict_invalid_inputs() {
        let base_unit = "A";
        let invalid_cases = vec![
            "",       // empty
            "123",    // missing unit
            "A",      // missing value
            "12.3AA", // multiple units
            "12.3xA", // invalid prefix
            "12..3A", // malformed number
            "1,2,3A", // malformed number with multiple commas
            "123AB",  // unit mismatch
            "--123A", // invalid sign
            "++123A", // invalid sign
        ];

        for input in invalid_cases {
            assert!(
                parse_unit_input_strict(input, base_unit).is_err(),
                "Input '{}' should fail strict parsing",
                input
            );
        }
    }

    #[test]
    fn test_unknown_prefix_si_prefix_to_exponent() {
        let unknowns = vec!["", "x", "abc", "1", "#", " "];

        for prefix in unknowns {
            assert_eq!(
                si_prefix_to_exponent(prefix, false),
                0,
                "Unknown prefix '{}' should return 0",
                prefix
            );
            assert_eq!(
                si_prefix_to_exponent(prefix, true),
                0,
                "Unknown prefix '{}' with reciprocal should return 0",
                prefix
            );
        }
    }

    #[test]
    fn test_all_prefixes_reciprocal_si_prefix_to_exponent() {
        let cases = vec![
            ("q", true, 30),
            ("r", true, 27),
            ("y", true, 24),
            ("z", true, 21),
            ("a", true, 18),
            ("f", true, 15),
            ("p", true, 12),
            ("n", true, 9),
            ("µ", true, 6),
            ("u", true, 6),
            ("m", true, 3),
            ("k", true, -3),
            ("M", true, -6),
            ("G", true, -9),
            ("T", true, -12),
            ("P", true, -15),
            ("E", true, -18),
            ("Z", true, -21),
            ("Y", true, -24),
            ("R", true, -27),
            ("Q", true, -30),
        ];

        for (prefix, reciprocal, expected) in cases {
            assert_eq!(
                si_prefix_to_exponent(prefix, reciprocal),
                expected,
                "Prefix '{}' with reciprocal={} failed",
                prefix,
                reciprocal
            );
        }
    }

    #[test]
    fn test_very_small_values_get_exponent() {
        // Anything smaller than 1e-60 should return 0
        let small_values = vec![0.0, 1e-100, 1e-61];
        for &x in &small_values {
            assert_eq!(get_exponent(x), 0, "x = {} should return 0", x);
        }
    }

    #[test]
    fn test_exact_powers_of_ten_get_exponent() {
        let cases = vec![
            (1e-30, -30),
            (1e-3, -3),
            (1.0, 0),
            (1e3, 3),
            (1e6, 6),
            (1e9, 9),
            (1e12, 12),
        ];

        for (x, expected) in cases {
            assert_eq!(get_exponent(x), expected, "x = {}", x);
        }
    }

    #[test]
    fn test_numbers_between_powers_of_ten_get_exponent() {
        let cases = vec![
            (5e-2, -3), // between 1e-3 and 1e0
            (7e1, 0),   // between 1e0 and 1e3
            (3e5, 3),   // between 1e3 and 1e6
            (9e8, 6),   // between 1e6 and 1e9
            (2e-5, -6), // between 1e-6 and 1e-3
        ];

        for (x, expected) in cases {
            assert_eq!(get_exponent(x), expected, "x = {}", x);
        }
    }

    #[test]
    fn test_edge_cases_near_multiple_of_three_get_exponent() {
        let cases = vec![
            (1e-3, -3),
            (1e-2, -3),
            (9e-4, -6),
            (1e0, 0),
            (9e0, 0),
            (1e1, 0),
            (1e3, 3),
            (5e3, 3),
            (9.99e3, 3),
        ];

        for (x, expected) in cases {
            assert_eq!(get_exponent(x), expected, "x = {}", x);
        }
    }

    #[test]
    fn test_large_numbers_get_exponent() {
        let cases = vec![(1e15, 15), (5e18, 18), (7e21, 21), (9e24, 24), (1e30, 30)];

        for (x, expected) in cases {
            assert_eq!(get_exponent(x), expected, "x = {}", x);
        }
    }
    #[test]
    fn test_is_permissive_exp_input_valid() {
        let valid_inputs = vec![
            "123", "+123", "-123", "0.456", "7,89", "1e10", "-1E-10", "+3.14E+2", ".5", "-.75",
            "42 ", // trailing space
            " 42", // leading space
        ];

        for input in valid_inputs {
            assert!(
                is_permissive_exp_input(input),
                "Input '{}' should be valid (permissive)",
                input
            );
        }
    }

    #[test]
    fn test_is_permissive_exp_input_invalid() {
        let invalid_inputs = vec!["abc", "12.3.4", "1e10.5", "1,2,3", "--123", "++123"];

        for input in invalid_inputs {
            assert!(
                !is_permissive_exp_input(input),
                "Input '{}' should be invalid (permissive)",
                input
            );
        }
    }

    #[test]
    fn test_parse_exp_input_strict_valid() {
        let valid_cases = vec![
            "123", "+123", "-123", "0.456", "7,89", "1e10", "-1E-10", "+3.14E+2", ".5", "-.75",
            "42 ", // trailing space
            " 42", // leading space
        ];

        for input in valid_cases {
            let result = parse_exp_input_strict(input).unwrap();
            assert_eq!(
                result,
                input.trim().replace(",", "."),
                "Input '{}' should parse strictly",
                input
            );
        }
    }

    #[test]
    fn test_parse_exp_input_strict_invalid() {
        let invalid_cases = vec![
            "", "abc", "12.3.4", "1e10.5", "1,2,3", "--123", "++123", "+", "-",
            "123 456", // extra tokens not allowed strictly
        ];

        for input in invalid_cases {
            assert!(
                parse_exp_input_strict(input).is_err(),
                "Input '{}' should fail strict parsing",
                input
            );
        }
    }

    #[test]
    fn test_parse_exp_input_strict_edge_cases() {
        let edge_cases = vec!["0", "0.0", "-0", "+0", "1E0", "-1E0"];

        for input in edge_cases {
            let result = parse_exp_input_strict(input).unwrap();
            assert_eq!(result, input, "Edge case '{}' should parse strictly", input);
        }
    }
    #[test]
    fn test_format_si_notation_normal() {
        let cases = vec![
            (1.0, false, "1 "),
            (1e3, false, "1 k"),
            (1e6, false, "1 M"),
            (1e-3, false, "1 m"),
            (2.5e-6, false, "2.5 µ"),
            (-3.0e9, false, "-3 G"),
            (7.89e-12, false, "7.89 p"),
        ];

        for (x, reciprocal, expected) in cases {
            let result = format_si_notation(x, reciprocal);
            assert_eq!(result, expected, "x={} reciprocal={}", x, reciprocal);
        }
    }

    #[test]
    fn test_format_si_notation_reciprocal() {
        let cases = vec![
            (1e3, true, "1 m"),
            (1e6, true, "1 µ"),
            (1e-3, true, "1 k"),
            (2.5e-6, true, "2.5 M"),
            (-3.0e9, true, "-3 n"),
        ];

        for (x, reciprocal, expected) in cases {
            let result = format_si_notation(x, reciprocal);
            assert_eq!(result, expected, "x={} reciprocal={}", x, reciprocal);
        }
    }

    #[test]
    fn test_format_si_notation_zero() {
        let zeros = vec![0.0, -0.0];

        for &x in &zeros {
            let result = format_si_notation(x, false);
            assert_eq!(result, "0.0 ");
        }
    }

    #[test]
    fn test_format_si_notation_infinite() {
        let infinities = vec![f64::INFINITY, f64::NEG_INFINITY];

        for &x in &infinities {
            let result = format_si_notation(x, false);
            assert_eq!(result, "∞");
        }
    }

    #[test]
    fn test_format_si_notation_edge_cases() {
        let cases = vec![
            (999.0, false, "999 "), // just below 1e3 → no prefix
            (1000.0, false, "1 k"), // exact 1e3
            (999.0, true, "999 "),  // reciprocal should still be same for small numbers
            (1000.0, true, "1 m"),  // reciprocal flips exponent
        ];

        for (x, reciprocal, expected) in cases {
            let result = format_si_notation(x, reciprocal);
            assert_eq!(result, expected, "x={} reciprocal={}", x, reciprocal);
        }
    }
}
