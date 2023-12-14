use opossum::error::OpmResult;
use uom::si::{f64::Length, length::meter};

fn uom_format(length: Length) {
    let base_value = length.get::<meter>();
    let mut exponent = (f64::log10(base_value).floor()) as i32;
    if exponent.is_negative() {
        exponent -= 2;
    }
    let exponent = (exponent / 3) * 3;
    let prefix = match exponent {
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "u",
        -3 => "m",
        0 => "",
        3 => "k",
        6 => "M",
        9 => "G",
        _ => "?",
    };
    println!("{:8.3} {prefix}m", base_value / f64::powi(10.0, exponent));
}
fn main() -> OpmResult<()> {
    uom_format(Length::new::<meter>(0.000000123456789));
    uom_format(Length::new::<meter>(0.00000123456789));
    uom_format(Length::new::<meter>(0.0000123456789));
    uom_format(Length::new::<meter>(0.000123456789));
    uom_format(Length::new::<meter>(0.00123456789));
    uom_format(Length::new::<meter>(0.0123456789));
    uom_format(Length::new::<meter>(0.123456789));
    uom_format(Length::new::<meter>(1.23456789));
    uom_format(Length::new::<meter>(12.3456789));
    uom_format(Length::new::<meter>(123.456789));
    uom_format(Length::new::<meter>(1234.56789));

    uom_format(Length::new::<meter>(0.00001));
    uom_format(Length::new::<meter>(0.0001));
    uom_format(Length::new::<meter>(0.001));
    uom_format(Length::new::<meter>(0.01));
    uom_format(Length::new::<meter>(0.1));
    uom_format(Length::new::<meter>(1.));
    uom_format(Length::new::<meter>(10.));
    uom_format(Length::new::<meter>(100.));
    uom_format(Length::new::<meter>(1000.));
    Ok(())
}
