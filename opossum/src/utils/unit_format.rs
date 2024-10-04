use num::Zero;
use uom::si::{
    f64::Length,
    length::{
        attometer, exameter, femtometer, gigameter, kilometer, megameter, meter, micrometer,
        millimeter, nanometer, petameter, picometer, terameter, zeptometer, zettameter,
    },
};

#[must_use]
pub fn get_prefix_for_base_unit(base_unit_value: f64) -> String {
    let exponent = get_exponent_for_base_unit_in_e3_steps(base_unit_value);
    match exponent {
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "μ",
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
