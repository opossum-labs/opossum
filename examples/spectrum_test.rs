use opossum::error::OpossumError;
use opossum::spectrum::Spectrum;
use uom::si::{f64::Length, length::nanometer};

fn main() -> Result<(), OpossumError> {
    let mut s = Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(410.0),
        Length::new::<nanometer>(1.0),
    )?;
    s.set_single_peak(Length::new::<nanometer>(409.0), 1.0)?;
    println!("{}", s);
    Ok(())
}
