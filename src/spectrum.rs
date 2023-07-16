use crate::error::OpossumError;
use ndarray::Array1;
use std::fmt::Display;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::length::meter;
use uom::si::{f64::Length, length::nanometer};
type Result<T> = std::result::Result<T, OpossumError>;

pub struct Spectrum {
    data: Array1<f64>,
    lambdas: Array1<f64>,
}

impl Spectrum {
    pub fn new(range: Range<Length>, resolution: Length) -> Result<Self> {
        if resolution <= Length::zero() {
            return Err(OpossumError::Spectrum("resolution must be positive".into()));
        }
        if range.start >= range.end {
            return Err(OpossumError::Spectrum(
                "wavelength range must be in ascending order".into(),
            ));
        }
        if range.start <= Length::zero() || range.end <= Length::zero() {
            return Err(OpossumError::Spectrum(
                "wavelength range limits must both be positive".into(),
            ));
        }
        let l = Array1::range(
            range.start.get::<meter>(),
            range.end.get::<meter>(),
            resolution.get::<meter>(),
        );
        let length = l.len();
        Ok(Self {
            lambdas: l,
            data: Array1::zeros(length),
        })
    }
    pub fn set_single_peak(&mut self, wavelength: Length, value: f64) -> Result<()> {
        let spectrum_range = self.lambdas.first().unwrap()..self.lambdas.last().unwrap();
        if !spectrum_range.contains(&&wavelength.get::<meter>()) {
            return Err(OpossumError::Spectrum(
                "wavelength is not in spectrum range".into(),
            ));
        }
        if value < 0.0 {
            return Err(OpossumError::Spectrum("energy must be positive".into()));
        }
        let wavelength_in_meters = wavelength.get::<meter>();
        let idx = self
            .lambdas
            .clone()
            .into_iter()
            .position(|w| w >= wavelength_in_meters);
        if let Some(idx) = idx {
            if idx == 0 {
                self.data[0] = value;
            } else {
                let lower_lambda = self.lambdas.get(idx - 1).unwrap();
                let upper_lambda = self.lambdas.get(idx).unwrap();
                let delta = upper_lambda - lower_lambda;
                self.data[idx - 1] = value * (1.0 - (wavelength_in_meters - lower_lambda) / delta);
                self.data[idx] = value * (wavelength_in_meters - lower_lambda) / delta;
            }
            Ok(())
        } else {
            Err(OpossumError::Spectrum("insertion point not found".into()))
        }
    }
    pub fn total_energy(&self) -> f64 {
        let mut total_energy = 0.0;
        for data in self.data.iter() {
            total_energy += *data;
        }
        total_energy
    }
    pub fn scale_vertical(&mut self, factor: f64) -> Result<()> {
        if factor < 0.0 {
            return Err(OpossumError::Spectrum(
                "scaling factor mus be >= 0.0".into(),
            ));
        }
        self.data = &self.data * factor;
        Ok(())
    }
    pub fn resample(&mut self, spectrum: &Spectrum) -> Result<()> {
        Ok(())
    }
    pub fn filter(&mut self, spectrum: &Spectrum) -> Result<()> {
        Ok(())
    }
}

impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in self.data.iter().enumerate() {
            write!(
                f,
                "{:7.2} -> {}\n",
                fmt_length.with(Length::new::<meter>(self.lambdas[value.0])),
                *value.1
            )
            .unwrap();
        }
        write!(f, "\nTotal energy: {}", self.total_energy())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ndarray::array;
    #[test]
    fn new() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        );
        assert_eq!(s.is_ok(), true);
        assert_eq!(
            s.as_ref().unwrap().lambdas,
            array![1.0, 1.5, 2.0, 2.5, 3.0, 3.5]
        );
        assert_eq!(s.unwrap().data, array![0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }
    #[test]
    fn new_negative_resolution() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(-0.5),
        );
        assert_eq!(s.is_ok(), false);
    }
    #[test]
    fn new_wrong_range() {
        let s = Spectrum::new(
            Length::new::<meter>(4.0)..Length::new::<meter>(1.0),
            Length::new::<meter>(0.5),
        );
        assert_eq!(s.is_ok(), false);
    }
    #[test]
    fn new_negative_range() {
        let s = Spectrum::new(
            Length::new::<meter>(-1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        );
        assert_eq!(s.is_ok(), false);
    }
    #[test]
    fn set_single_peak() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(2.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[1], 1.0);
    }
    #[test]
    fn set_single_peak_interpolated() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(2.5), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[1], 0.5);
        assert_eq!(s.data[2], 0.5);
    }
    #[test]
    fn set_single_peak_lower_bound() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(1.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[0], 1.0);
    }
    #[test]
    fn set_single_peak_out_of_limit() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(0.5), 1.0).is_ok(),
            false
        );
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(4.0), 1.0).is_ok(),
            false
        );
    }
    #[test]
    fn set_single_peak_ngative_energy() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(1.5), -1.0).is_ok(),
            false
        );
    }
    #[test]
    fn total_energy() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s.set_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn scale_vertical() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s.set_single_peak(Length::new::<meter>(2.5), 1.0).unwrap();
        assert_eq!(s.scale_vertical(0.5).is_ok(), true);
        assert_eq!(s.data, array![0.0, 0.25, 0.25, 0.0]);
    }
    #[test]
    fn scale_vertical_negative() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s.set_single_peak(Length::new::<meter>(2.5), 1.0).unwrap();
        assert_eq!(s.scale_vertical(-0.5).is_ok(), false);
    }
}
