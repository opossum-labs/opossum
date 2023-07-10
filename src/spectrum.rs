use std::fmt::Display;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::energy::joule;
use uom::si::{
    f64::{Energy, Length},
    length::nanometer,
};
pub struct Spectrum {
    start: Length,
    dlambda: Length,
    data: Vec<Energy>,
}

impl Spectrum {
    pub fn new(range: Range<Length>, resolution: Length) -> Self {
        Self {
            start: range.start,
            dlambda: resolution,
            data: vec![Energy::zero(); ((range.end-range.start)/resolution).value as usize]
        }
    }
    pub fn set_single_peak(&mut self, wavelength: Length, energy: Energy) {
        let index=((wavelength-self.start) / self.dlambda).value as usize;
        self.data[index]=energy; 
    }
    pub fn total_energy(&self) -> Energy {
        let mut total_energy=Energy::zero();
        for data in self.data.iter() {
            total_energy+=*data;
        }
        total_energy
    }
}

impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        let fmt_energy = Energy::format_args(joule, Abbreviation);
        for value in self.data.iter().enumerate() {
            let wavelength=self.start + value.0 as f64 * self.dlambda;
            write!(
                f,
                "{:7.2} -> {}\n",
                fmt_length.with(wavelength),
                fmt_energy.with(*value.1)
            ).unwrap();
        }
        write!(f, "\nTotal energy: {}",fmt_energy.with(self.total_energy()))
    }
}
