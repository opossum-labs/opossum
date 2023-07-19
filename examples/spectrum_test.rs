use opossum::error::OpossumError;
use opossum::spectrum::Spectrum;
use uom::si::{f64::Length, length::meter};

fn main() -> Result<(), OpossumError> {
    let mut s = Spectrum::new(
        Length::new::<meter>(400.0)..Length::new::<meter>(450.0),
        Length::new::<meter>(0.1),
    )?;
    s.add_lorentzian_peak(Length::new::<meter>(415.0), Length::new::<meter>(3.2), 2.0)?;

    let mut s2=Spectrum::new(
        Length::new::<meter>(400.0)..Length::new::<meter>(450.0),
        Length::new::<meter>(2.1),
    )?;
    s2.add_lorentzian_peak(Length::new::<meter>(430.0), Length::new::<meter>(1.2), 0.5)?;
    s.add(&s2);

    let mut s3=Spectrum::new(
        Length::new::<meter>(400.0)..Length::new::<meter>(450.0),
        Length::new::<meter>(0.05),
    )?;
    s3.add_lorentzian_peak(Length::new::<meter>(420.0), Length::new::<meter>(0.3), 0.02)?;
    s.sub(&s3);
    s.to_plot("spectrum.svg");
    Ok(())
} 
