use std::path::Path;

use opossum::plottable::PltBackEnd;
use opossum::spectrum::Spectrum;
use opossum::{error::OpmResult, plottable::Plottable, spectrum_helper::create_visible_spec};
use uom::si::{f64::Length, length::nanometer};

fn main() -> OpmResult<()> {
    let mut s = Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(0.1),
    )?;
    s.add_lorentzian_peak(
        Length::new::<nanometer>(415.0),
        Length::new::<nanometer>(3.2),
        2.0,
    )?;

    let mut s2 = Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(2.1),
    )?;
    s2.add_lorentzian_peak(
        Length::new::<nanometer>(430.0),
        Length::new::<nanometer>(1.2),
        0.5,
    )?;
    s.add(&s2);

    let mut s3 = Spectrum::new(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(450.0),
        Length::new::<nanometer>(0.05),
    )?;
    s3.add_lorentzian_peak(
        Length::new::<nanometer>(420.0),
        Length::new::<nanometer>(0.3),
        0.02,
    )?;
    s.sub(&s3);
    s.to_plot(
        Path::new("./opossum/playground/spectrum.svg"),
        PltBackEnd::SVG,
    )?;

    let s4 = Spectrum::from_csv("./opossum/playground/NE03B.csv")?;
    s4.to_plot(
        Path::new("./opossum/playground/ne03b_raw.svg"),
        PltBackEnd::SVG,
    )?;
    let mut s5 = create_visible_spec();
    s5.resample(&s4);
    s5.to_plot(
        Path::new("./opossum/playground/ne03b.svg"),
        PltBackEnd::SVG,
    )?;
    Ok(())
}
