use crate::error::OpossumError;
use ndarray::Array1;
use ndarray_interp::Interp1DBuilder;
use ndarray_interp::Interp1DStrategy::Linear;
use std::fmt::Display;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::length::meter;
use uom::si::{f64::Length, length::nanometer};
type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Clone)]
pub struct Spectrum {
    data: Array1<f64>,    // data in 1/meters
    lambdas: Array1<f64>, // wavelength in meters
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
                let delta = self.lambdas.get(1).unwrap() - self.lambdas.get(0).unwrap();
                self.data[0] = value / delta;
            } else {
                let lower_lambda = self.lambdas.get(idx - 1).unwrap();
                let upper_lambda = self.lambdas.get(idx).unwrap();
                let delta = upper_lambda - lower_lambda;
                self.data[idx - 1] =
                    value * (1.0 - (wavelength_in_meters - lower_lambda) / delta) / delta;
                self.data[idx] = value * (wavelength_in_meters - lower_lambda) / delta / delta;
            }
            Ok(())
        } else {
            Err(OpossumError::Spectrum("insertion point not found".into()))
        }
    }
    pub fn total_energy(&self) -> f64 {
        let mut total_energy = 0.0;
        for data in self.data.iter().enumerate() {
            if data.0 < self.data.len() - 1 {
                let delta =
                    self.lambdas.get(data.0 + 1).unwrap() - self.lambdas.get(data.0).unwrap();
                total_energy += *data.1 * delta;
            }
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
    fn enclosing_interval(&self, x_lower: f64, x_upper: f64) -> Vec<usize> {
        let mut res = self
            .clone()
            .lambdas
            .into_iter()
            .enumerate()
            .filter(|x| x.1 < x_upper && x.1 > x_lower)
            .map(|x| x.0)
            .collect::<Vec<usize>>();
        if res.is_empty() {
            let mut lower_idx = self.clone().lambdas.into_iter().position(|x| x >= x_lower).unwrap();
            let upper_idx = self.clone().lambdas.into_iter().position(|x| x >= x_upper).unwrap();
            if lower_idx>0 && self.lambdas[lower_idx]> x_lower {
                lower_idx-=1;
            }
            res = vec![lower_idx, upper_idx];
        } else {
            let first = *res.first().unwrap();
            let last = *res.last().unwrap();
            if first > 0 {
                res.insert(0, first - 1);
            }
            if last < self.lambdas.len() - 1 {
                res.push(last + 1)
            }
        }
        res
    }
    pub fn resample(&mut self, spectrum: &Spectrum) -> Result<()> {
        let _data = spectrum.data.clone();
        let _x = spectrum.lambdas.clone();
        let max_idx = self.data.len() - 1;
        for x in self.data.iter_mut().enumerate().filter(|x| x.0 < max_idx) {
            let lower_bound = *self.lambdas.get(x.0).unwrap();
            let upper_bound = *self.lambdas.get(x.0 + 1).unwrap();
            let interval = spectrum.enclosing_interval(lower_bound, upper_bound);
        }
        Ok(())
    }
    pub fn filter(&mut self, _spectrum: &Spectrum) -> Result<()> {
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
            Length::new::<meter>(0.5),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(2.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2], 2.0);
    }
    #[test]
    fn set_single_peak_interpolated() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(2.25), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2], 1.0);
        assert_eq!(s.data[3], 1.0);
    }
    #[test]
    fn set_single_peak_lower_bound() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        assert_eq!(
            s.set_single_peak(Length::new::<meter>(1.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[0], 2.0);
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
    fn set_single_peak_negative_energy() {
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
            Length::new::<meter>(0.5),
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
    #[test]
    fn enclosing_interval() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.1, 3.9);
        let l = v.into_iter().map(|i| s.lambdas[i]).collect::<Vec<f64>>();
        assert_eq!(l, vec![2.0, 3.0, 4.0]);
    }
    #[test]
    fn enclosing_interval_exact() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.0, 4.0);
        let l = v.into_iter().map(|i| s.lambdas[i]).collect::<Vec<f64>>();
        assert_eq!(l, vec![2.0, 3.0, 4.0]);
    }
    #[test]
    fn enclosing_interval_upper_bound() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(3.0, 5.0);
        let l = v.into_iter().map(|i| s.lambdas[i]).collect::<Vec<f64>>();
        assert_eq!(l, vec![3.0, 4.0]); // ranges are exclusive upper bound !
    }
    #[test]
    fn enclosing_interval_lower_bound() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(1.0, 3.0);
        let l = v.into_iter().map(|i| s.lambdas[i]).collect::<Vec<f64>>();
        assert_eq!(l, vec![1.0, 2.0, 3.0]);
    }
    #[test]
    fn enclosing_interval_small_range() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.6, 2.9);
        let l = v.into_iter().map(|i| s.lambdas[i]).collect::<Vec<f64>>();
        assert_eq!(l, vec![2.0, 3.0]);
    }
    // #[test]
    // fn resample() {
    //     let mut s1 = Spectrum::new(
    //         Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
    //         Length::new::<meter>(1.0),
    //     )
    //     .unwrap();
    //     s1.set_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
    //     let s2 = Spectrum::new(
    //         Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
    //         Length::new::<meter>(1.0),
    //     )
    //     .unwrap();
    //     s1.resample(&s2).unwrap();
    //     assert_eq!(s1.data, s2.data);
    // }
    #[test]
    fn resample_interp() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(0.9)..Length::new::<meter>(6.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        s2.set_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        println!("s2: {}", s2);
        s1.resample(&s2).unwrap();
        assert_eq!(s1.data, array![0.0, 0.5, 0.5, 0.0]);
    }
}
