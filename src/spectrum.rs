use ndarray::Array1;
use std::fmt::Display;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::energy::joule;
use uom::si::length::meter;
use uom::si::{
    f64::{Energy, Length},
    length::nanometer,
};

use crate::error::OpossumError;
type Result<T> = std::result::Result<T, OpossumError>;

pub struct Spectrum {
    data: Array1<Energy>,
    lambdas: Array1<f64>,
}

impl Spectrum {
    pub fn new(range: Range<Length>, resolution: Length) -> Self {
        let l = Array1::range(
            range.start.get::<meter>(),
            range.end.get::<meter>(),
            resolution.get::<meter>(),
        );
        let length = l.len();
        Self {
            lambdas: l,
            data: Array1::zeros(length),
        }
    }
    pub fn set_single_peak(&mut self, wavelength: Length, energy: Energy) -> Result<()> {
        let spectrum_range = self.lambdas.first().unwrap()..self.lambdas.last().unwrap();
        if !spectrum_range.contains(&&wavelength.get::<meter>()) {
            return Err(OpossumError::Other(
                "wavelength is not in spectrum range".into(),
            ));
        }
        if energy.is_sign_negative() {
            return Err(OpossumError::Other("energy must be positive".into()));
        }
        let wavelength_in_meters = wavelength.get::<meter>();
        let idx = self
            .lambdas
            .clone()
            .into_iter()
            .position(|w| w >= wavelength_in_meters);
        if let Some(idx) = idx {
            if idx == 0 {
                self.data[0] = energy;
            } else {
                let lower_lambda = self.lambdas.get(idx - 1).unwrap();
                let upper_lambda = self.lambdas.get(idx).unwrap();
                let delta = upper_lambda - lower_lambda;
                self.data[idx - 1] = energy * (1.0 - (wavelength_in_meters - lower_lambda) / delta);
                self.data[idx] = energy * (wavelength_in_meters - lower_lambda) / delta;
            }
            Ok(())
        } else {
            Err(OpossumError::Other("insertion point not found".into()))
        }
    }
    pub fn total_energy(&self) -> Energy {
        let mut total_energy = Energy::zero();
        for data in self.data.iter() {
            total_energy += *data;
        }
        total_energy
    }
}

impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        let fmt_energy = Energy::format_args(joule, Abbreviation);
        for value in self.data.iter().enumerate() {
            write!(
                f,
                "{:7.2} -> {}\n",
                fmt_length.with(Length::new::<meter>(self.lambdas[value.0])),
                fmt_energy.with(*value.1)
            )
            .unwrap();
        }
        write!(
            f,
            "\nTotal energy: {}",
            fmt_energy.with(self.total_energy())
        )
    }
}
