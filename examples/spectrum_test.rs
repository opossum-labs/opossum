use opossum::error::OpossumError;
use opossum::spectrum::Spectrum;
use uom::si::{f64::Length, length::{nanometer,meter}};

fn main() -> Result<(), OpossumError> {
    let mut s = Spectrum::new(
        Length::new::<meter>(400.0)..Length::new::<meter>(450.0),
        Length::new::<meter>(0.1),
    )?;
    //s.add_single_peak(Length::new::<meter>(405.0), 1.0)?;
    s.add_lorentzian_peak(Length::new::<meter>(410.0), Length::new::<meter>(0.2), 1.0)?;

    let mut s2=Spectrum::new(
        Length::new::<meter>(400.0)..Length::new::<meter>(450.0),
        Length::new::<meter>(2.1),
    )?;
    s2.add_lorentzian_peak(Length::new::<meter>(430.0), Length::new::<meter>(1.2), 0.5)?;
    s.add(&s2);
    s.to_plot("spectrum.svg");
    Ok(())
} 
