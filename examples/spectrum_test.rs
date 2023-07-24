use opossum::error::OpossumError;
use opossum::spectrum::{Spectrum, create_visible_spectrum};
use uom::si::{f64::Length, length::nanometer};

fn main() -> Result<(), OpossumError> {
    let mut s = Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(0.1),
    )?;
    s.add_lorentzian_peak(Length::new::<nanometer>(415.0), Length::new::<nanometer>(3.2), 2.0)?;

    let mut s2=Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(2.1),
    )?;
    s2.add_lorentzian_peak(Length::new::<nanometer>(430.0), Length::new::<nanometer>(1.2), 0.5)?;
    s.add(&s2);

    let mut s3=Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(0.05),
    )?;
    s3.add_lorentzian_peak(Length::new::<nanometer>(420.0), Length::new::<nanometer>(0.3), 0.02)?;
    s.sub(&s3);
    s.to_plot("spectrum.svg");

    let s4=Spectrum::from_csv("NE03B.csv")?;
    s4.to_plot("ne03b_raw.svg");
    let mut s5=create_visible_spectrum();
    s5.resample(&s4);
    s5.to_plot("ne03b.svg");
    Ok(())
} 
