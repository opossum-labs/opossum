pub mod input_components;
use approx::relative_eq;
use dioxus::prelude::*;
use opossum_core::utils::geom_transformation::{RotationAxis, TranslationAxis};
use regex::Regex;
use std::{fmt::Display, str::FromStr};
use strum::IntoEnumIterator;

const EXP_NOTATION_MIN: i32 = -30;
const EXP_NOTATION_MAX: i32 = 30;
const ZERO_BELOW_EXP: i32 = -44;

pub trait IntoInputData<T, D, B: 'static>: Into<InputParam>
where
    D: Into<B> + Clone + 'static,
    T: Clone + FromStr + 'static,
    Self: IntoEnumIterator + IntoInputDataStrings<D> + Copy + 'static,
{
    fn setter_from_obj(&self) -> impl FnMut(&mut D, T);

    // Callback für Standard-Events (Checkboxen, Selects)
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

    // NEU: Callback für String-Events (FlushableTextInput)
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
    let re = Regex::new(
        r"^\s*(?P<num>[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?)\s*(?P<unit>[a-zA-Zµ]*)?\s*$",
    )
    .unwrap();

    let Some(caps) = re.captures(input) else {
        return false;
    };

    let num = caps.name("num").map_or("", |m| m.as_str());
    let unit = caps.name("unit").map_or("", |m| m.as_str());

    let num_re = Regex::new(r"^[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?$").unwrap();
    if !num_re.is_match(num) {
        return false;
    } else {
        true
    }

    // if unit == base_unit {
    //     return true;
    // }

    // let Some(prefix) = unit.strip_suffix(base_unit) else {
    //     return false;
    // };

    // if prefix.chars().count() != 1 {
    //     return false;
    // }

    // "qryzafpnµumkMGTPEZYRQ".contains(prefix)
}

fn is_permissive_exp_input(input: &str) -> bool {
    let regex = Regex::new(r"^[+-]?\d*(?:[.,]?\d*)?(?:[eE][+-]?\d*)?$").unwrap();
    let mut split = input.split_whitespace();
    let num = split.next().unwrap_or("");
    regex.is_match(num)
}

pub fn parse_exp_input_strict(input: &str) -> Result<String, ()> {
    let regex = Regex::new(r"^[+-]?(?:\d*(?:[.,]\d*)?|[.,]\d+)(?:[eE][+-]?\d*)?$").unwrap();
    let mut split_input = input.split_whitespace();
    let value_str = split_input.next().ok_or(())?;

    if regex.is_match(value_str) {
        return Ok(value_str.to_string());
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
        r"^(?P<value>[+-]?(?:\d*(?:[.,]\d*)?|[.,]\d+)(?:[eE][+-]?\d*)?)\s*(?P<unit>[a-zA-Zµ]+)$",
    )
    .unwrap();

    let valid_prefixes = [
        'q', 'r', 'y', 'z', 'a', 'f', 'p', 'n', 'µ', 'u', 'm', 'k', 'M', 'G', 'T', 'P', 'E', 'Z',
        'Y', 'R', 'Q',
    ];

    let caps = regex.captures(input).ok_or(())?;

    let value = caps.name("value").unwrap().as_str();
    let unit = caps.name("unit").unwrap().as_str();

    if unit == base_unit {
        return Ok((value.to_string(), String::new()));
    }

    if let Some(prefix_part) = unit.strip_suffix(base_unit)
        && prefix_part.chars().count() == 1
    {
        let prefix_char = prefix_part.chars().next().unwrap();
        if valid_prefixes.contains(&prefix_char) {
            return Ok((value.to_string(), prefix_char.to_string()));
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

    format!("{} {prefix}", mantissa.to_string())
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
        format!("{}e{exponent}", mantissa.to_string())
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
