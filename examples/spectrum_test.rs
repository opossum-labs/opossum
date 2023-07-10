use opossum::spectrum::Spectrum;
use uom::si::energy::joule;
use uom::si::{f64::Length, length::nanometer};
use uom::si::f64::Energy;

fn main() {
    let mut s=Spectrum::new(Length::new::<nanometer>(400.0)..Length::new::<nanometer>(410.0),Length::new::<nanometer>(1.0));
    s.set_single_peak(Length::new::<nanometer>(405.0), Energy::new::<joule>(1.0));
    println!("{}",s);
}