pub mod input_components;
use approx::relative_eq;
use dioxus::prelude::*;
use opossum_core::utils::geom_transformation::{RotationAxis, TranslationAxis};
use regex::Regex;
use std::{fmt::Display, str::FromStr};
use strum::IntoEnumIterator;

pub trait IntoInputData<T, D, B: 'static>: Into<InputParam>
where
    D: Into<B> + Clone + 'static,
    T: Clone + FromStr + 'static,
    Self: IntoEnumIterator + IntoInputDataStrings<D> + Copy + 'static,
{
    fn setter_from_obj(&self) -> impl FnMut(&mut D, T);

    // Callback für Standard-Events (Checkboxen, Selects)
    fn create_callback(&self, mut obj: D, mut sig: Signal<B>) -> EventHandler<Event<FormData>> {
        let this = *self;
        EventHandler::new(move |e: Event<FormData>| {
            if let Some(value) = this.parse_value(e) {
                let mut setter = this.setter_from_obj();
                setter(&mut obj, value);
                sig.set(obj.clone().into());
            }
        })
    }

    // NEU: Callback für String-Events (FlushableTextInput)
    fn create_callback_str(&self, mut obj: D, mut sig: Signal<B>) -> EventHandler<String> {
        let this = *self;
        EventHandler::new(move |val_str: String| {
            // Wir parsen direkt den String
            if let Ok(value) = val_str.parse::<T>() {
                let mut setter = this.setter_from_obj();
                setter(&mut obj, value);
                sig.set(obj.clone().into());
            }
        })
    }

    fn parse_value(&self, e: Event<FormData>) -> Option<T> {
        let e_value = e.value();
        e_value.parse::<T>().ok()
    }

    fn to_input_data(&self, obj: D, sig: Signal<B>) -> InputData {
        let value_str = self.create_value_string(&obj);
        InputData::new(
            Into::<InputParam>::into(*self),
            self.create_id_string().as_str(),
            self.create_callback(obj.clone(), sig), // Für Legacy/Events
            self.create_callback_str(obj, sig),     // Für Flushable
            value_str,
        )
    }

    fn to_input_data_vec(obj: &D, sig: Signal<B>) -> Vec<InputData> {
        let mut input_data = Vec::<InputData>::new();
        for enum_variant in Self::iter() {
            input_data.push(enum_variant.to_input_data(obj.clone(), sig));
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
    Length(String),
    Selection(String, Vec<(bool, String)>),
    Energy(String),
    Angle(String),
    Bool(String),
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
            | Self::Length(label)
            | Self::Energy(label)
            | Self::Angle(label)
            | Self::Bool(label)
            | Self::FilePath(label, _) => label.clone(),
        }
    }
    #[must_use]
    pub const fn rtype(&self) -> &'static str {
        match self {
            Self::Usize(_)
            | Self::U8(_)
            | Self::F64(_)
            | Self::Length(_)
            | Self::Energy(_)
            | Self::Angle(_) => "number",
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
            TranslationAxis::X => Self::Length("X translation".into()),
            TranslationAxis::Y => Self::Length("Y translation".into()),
            TranslationAxis::Z => Self::Length("Z translation".into()),
        }
    }
}

impl From<RotationAxis> for InputParam {
    fn from(axis: RotationAxis) -> Self {
        match axis {
            RotationAxis::Roll => Self::Angle("Roll".into()),
            RotationAxis::Pitch => Self::Angle("Pitch".into()),
            RotationAxis::Yaw => Self::Angle("Yaw".into()),
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
    pub callback: EventHandler<Event<FormData>>, // Alt (für Checkboxen)
    pub callback_str: EventHandler<String>,      // Neu (für FlushableTextInput)
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

/// Formats a floating-point value with a fixed number of decimal places.
///
/// The value is rounded to `decimals` decimal places. Trailing zeros in the
/// fractional part are removed, but at least one decimal digit is retained
/// if a decimal point is present.
///
/// This function is intended for numeric display purposes where predictable
/// decimal formatting is required without scientific notation.
///
/// # Arguments
///
/// * `v` - The floating-point value to format.
/// * `decimals` - The maximum number of decimal places to retain.
///
/// # Returns
///
/// A `String` containing the formatted decimal representation.
fn format_fixed_decimal(v: f64, decimals: usize) -> String {
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_possible_truncation)]
    let factor = 10f64.powi(decimals as i32);
    let scaled = (v * factor).round();
    #[allow(clippy::cast_possible_truncation)]
    let int_scaled = scaled as i128;

    let s = int_scaled.to_string();

    if decimals == 0 {
        return s;
    }

    if s.len() <= decimals {
        let zeros = decimals - s.len();
        let mut out = String::from("0.");
        out.push_str(&"0".repeat(zeros));
        out.push_str(&s);
        return out;
    }

    let split = s.len() - decimals;
    let (int_part, frac_part) = s.split_at(split);

    let trimmed = frac_part.trim_end_matches('0');
    if trimmed.is_empty() {
        format!("{int_part}.0")
    } else {
        format!("{int_part}.{trimmed}")
    }
}

/// Parses a numeric string together with an SI prefix into a floating-point value.
///
/// The numeric string may use either `.` or `,` as the decimal separator.
/// The parsed value is scaled according to the given SI prefix and clamped
/// to a reasonable numeric range to avoid extreme magnitudes.
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
    let max_num = 1e33 * (1. - 1e-14);
    let min_num = 0.0;
    let factor = si_prefix_to_exponent(prefix_str, reciprocal);

    let normalized = num_str.replace(',', ".");
    normalized.parse::<f64>().map_or(None, |parsed| {
        if (parsed.abs() * 10f64.powi(factor)) > max_num {
            Some(parsed.signum() * max_num)
        } else if (parsed.abs() * 10f64.powi(factor)) < min_num {
            Some(parsed.signum() * min_num)
        } else {
            Some(parsed * 10f64.powi(factor))
        }
    })
}

/// Checks whether an input string resembles a valid, permissive unit input.
///
/// This function is designed for interactive user input and allows partially
/// entered numbers (e.g. `"1."`, `"-"`, `"3e"`). It validates the numeric part
/// loosely and ensures that the unit consists of an optional single SI prefix
/// followed by the given base unit.
///
/// # Arguments
///
/// * `input` - The full user input string (number and unit).
/// * `base_unit` - The expected base unit (e.g. `"V"`, `"Hz"`).
///
/// # Returns
///
/// `true` if the input is syntactically acceptable in a permissive context,
/// otherwise `false`.
fn is_permissive_unit_input(input: &str, base_unit: &str) -> bool {
    let regex = Regex::new(r"^[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?$").unwrap();
    let mut split = input.split_whitespace();
    let num = split.next().unwrap_or("");
    let unit = split.next().unwrap_or("");

    if !regex.is_match(num) {
        return false;
    }

    if unit == base_unit {
        return true;
    }

    let Some(prefix) = unit.strip_suffix(base_unit) else {
        return false;
    };

    let mut chars = prefix.chars();
    let Some(c) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }

    "qryzafpnµumkMGTPEZYRQ".contains(c)
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
    let valid_prefixes: Vec<char> = vec![
        'q', 'r', 'y', 'z', 'a', 'f', 'p', 'n', 'µ', 'u', 'm', 'k', 'M', 'G', 'T', 'P', 'E', 'Z',
        'Y', 'R', 'Q',
    ];
    let regex = Regex::new(r"^[+-]?(?:\d*(?:[.,]\d*)?|[.,]\d+)(?:[eE][+-]?\d*)?$").unwrap();
    let mut split_input = input.split_whitespace();
    let value_str = split_input.next().ok_or(())?;
    let prefix_str = split_input
        .next()
        .ok_or(())?
        .strip_suffix(base_unit)
        .unwrap_or("");

    if prefix_str.is_empty() {
        if regex.is_match(value_str) {
            Ok((value_str.to_string(), String::new()))
        } else {
            Err(())
        }
    } else {
        let mut chars = prefix_str.chars();
        let prefix_char = chars.next_back().unwrap();
        if valid_prefixes.contains(&prefix_char) && regex.is_match(value_str) {
            return Ok((value_str.to_string(), prefix_char.to_string()));
        }
        Err(())
    }
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

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let decimals = 15 - mantissa.abs().log10().abs().ceil() as usize;
    let mantissa_str = format_fixed_decimal(mantissa, decimals);

    format!("{mantissa_str} {prefix}")
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
    if relative_eq!(x, 0.0) {
        return (0.0, 0);
    }

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();

    let exp3 = get_exponent(x_abs);
    let mantissa = sign * (x_abs / 10f64.powi(exp3));

    (mantissa, exp3)
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
    if relative_eq!(x_abs, 0.0) {
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
