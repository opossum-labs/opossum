use crate::error::OpossumError;
use ndarray::Array1;
use std::f64::consts::PI;
use std::fmt::Display;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::length::meter;
use uom::si::{f64::Length, length::nanometer};
type Result<T> = std::result::Result<T, OpossumError>;

/// Structure for handling spectral data.
///
/// This structure handles an array of values over a given wavelength range. Although the interface
/// is still limited. The structure is prepared for handling non-equidistant wavelength slots.  
#[derive(Clone)]
pub struct Spectrum {
    data: Array1<f64>,    // data in 1/meters
    lambdas: Array1<f64>, // wavelength in meters
}

impl Spectrum {
    /// Create a new (empty) spectrum of a given wavelength range and (equidistant) resolution.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the wavelength range is not in ascending order
    ///   - the wavelength limits are not both positive
    ///   - the resolution is not positive
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
    /// Add a single peak to the given [`Spectrum`].
    ///
    /// This functions adds a single (resolution limited) peak to the [`Spectrum`] at the given wavelength and
    /// the given energy / intensity. If the given wavelength does not exactly match a spectrum slot the energy is distributed
    /// over neighboring slots such that the total energy matches the given energy.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the wavelength i s outside the spectrum range
    ///   - the energy is negative
    pub fn add_single_peak(&mut self, wavelength: Length, value: f64) -> Result<()> {
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
                self.data[0] += value / delta;
            } else {
                let lower_lambda = self.lambdas.get(idx - 1).unwrap();
                let upper_lambda = self.lambdas.get(idx).unwrap();
                let delta = upper_lambda - lower_lambda;
                self.data[idx - 1] +=
                    value * (1.0 - (wavelength_in_meters - lower_lambda) / delta) / delta;
                self.data[idx] += value * (wavelength_in_meters - lower_lambda) / delta / delta;
            }
            Ok(())
        } else {
            Err(OpossumError::Spectrum("insertion point not found".into()))
        }
    }

    /// Adds an emission line to this [`Spectrum`].
    ///
    /// This function adds a laser line (following a [Lorentzian](https://en.wikipedia.org/wiki/Cauchy_distribution) function) with a given 
    /// center wavelength, width and energy to the spectrum. **Note**: Due to rounding errors (discrete wavelength bins, upper/lower spectrum
    /// limits) the total energy is not exactly the given value.  
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the center wavelength in negative
    ///   - the width is negative
    ///   - the energy is negative
    pub fn add_lorentzian_peak(
        &mut self,
        center: Length,
        width: Length,
        energy: f64,
    ) -> Result<()> {
        if center.is_sign_negative() {
            return Err(OpossumError::Spectrum(
                "center wavelength must be positive".into(),
            ));
        }
        if width.is_sign_negative() {
            return Err(OpossumError::Spectrum("line width must be positive".into()));
        }
        if energy < 0.0 {
            return Err(OpossumError::Spectrum("energy must be positive".into()));
        }
        let wavelength_in_meters = center.get::<meter>();
        let width_in_meters = width.get::<meter>();
        let spectrum_data: Array1<f64> = self
            .lambdas
            .iter_mut()
            .map(|x| lorentz(wavelength_in_meters, width_in_meters, *x))
            .collect();
        self.data = &self.data + (spectrum_data * energy);
        Ok(())
    }
    /// Returns the total energy of this [`Spectrum`].
    ///
    /// This function sums the values over all wavelength slots weighted with the individual slot widths. This
    /// way it also works for non-equidistant spectra.
    pub fn total_energy(&self) -> f64 {
        let lambda_deltas: Vec<f64> = self
            .lambdas
            .windows(2)
            .into_iter()
            .map(|l| l[1] - l[0])
            .collect();
        let total_energy = lambda_deltas
            .into_iter()
            .zip(self.data.iter())
            .map(|d| d.0 * *d.1)
            .sum();
        total_energy
    }
    /// Scale the spectrum by a constant factor.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if the scaling factor is < 0.0.
    pub fn scale_vertical(&mut self, factor: f64) -> Result<()> {
        if factor < 0.0 {
            return Err(OpossumError::Spectrum(
                "scaling factor mus be >= 0.0".into(),
            ));
        }
        self.data = &self.data * factor;
        Ok(())
    }
    fn enclosing_interval(&self, x_lower: f64, x_upper: f64) -> Vec<Option<usize>> {
        let mut res = self
            .clone()
            .lambdas
            .into_iter()
            .enumerate()
            .filter(|x| x.1 < x_upper && x.1 > x_lower)
            .map(|x| Some(x.0))
            .collect::<Vec<Option<usize>>>();
        if res.is_empty() {
            let mut lower_idx = self.clone().lambdas.into_iter().position(|x| x >= x_lower);
            let upper_idx = self.clone().lambdas.into_iter().position(|x| x >= x_upper);
            if let Some(l_i) = lower_idx {
                if l_i > 0 && self.lambdas[l_i] > x_lower {
                    lower_idx = Some(l_i - 1);
                }
            }
            if lower_idx == upper_idx {
                res = vec![None, None];
            } else {
                res = vec![lower_idx, upper_idx];
            }
        } else {
            let res_cp = res.clone();
            let first = res_cp.first().unwrap();
            let last = res_cp.last().unwrap();
            if let Some(first) = first {
                if *first > 0 {
                    res.insert(0, Some(first - 1));
                }
            }
            if let Some(last) = last {
                if *last < self.lambdas.len() - 1 {
                    res.push(Some(last + 1))
                }
            }
        }
        res
    }

    /// Resample a provided [`Spectrum`] to match the given one.
    ///
    /// This function maps values and wavelengths of a provided spectrum to the structure of self. This function conserves the toal
    /// energy if the the given interval is fully contained in self. This does not necessarily conserve peak widths or positions.  
    ///
    /// # Panics
    ///
    /// Panics if ???.
    pub fn resample(&mut self, spectrum: &Spectrum) {
        let _data = spectrum.data.clone();
        let _x = spectrum.lambdas.clone();
        let max_idx = self.data.len() - 1;
        for x in self.data.iter_mut().enumerate().filter(|x| x.0 < max_idx) {
            let lower_bound = *self.lambdas.get(x.0).unwrap();
            let upper_bound = *self.lambdas.get(x.0 + 1).unwrap();
            let interval = spectrum.enclosing_interval(lower_bound, upper_bound);
            let mut bucket_value = 0.0;
            for src_idx in interval.windows(2) {
                if (*src_idx)[0].is_some() && (*src_idx)[1].is_some() {
                    let source_left = spectrum.lambdas[src_idx[0].unwrap()];
                    let source_right = spectrum.lambdas[src_idx[1].unwrap()];
                    let ratio = calc_ratio(lower_bound, upper_bound, source_left, source_right);
                    bucket_value +=
                        spectrum.data[src_idx[0].unwrap()] * ratio * (source_right - source_left)
                            / (upper_bound - lower_bound);
                }
            }
            *x.1 = bucket_value;
        }
    }
    /// Filter the spectrum with another given spectrum by multiplying the data values. The given spectrum is resampled before the multiplication.
    pub fn filter(&mut self, filter_spectrum: &Spectrum) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(filter_spectrum);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| d.0 * d.1)
            .collect();
    }
}
impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in self.data.iter().enumerate() {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(Length::new::<meter>(self.lambdas[value.0])),
                *value.1
            )
            .unwrap();
        }
        write!(f, "\nTotal energy: {}", self.total_energy())
    }
}

fn calc_ratio(bucket_left: f64, bucket_right: f64, source_left: f64, source_right: f64) -> f64 {
    if bucket_left < source_left && bucket_right > source_left && bucket_right < source_right {
        // bucket is left partly outside source
        return (bucket_right - source_left) / (source_right - source_left);
    }
    if bucket_left <= source_left && bucket_right >= source_right {
        // bucket contains source
        return 1.0;
    }
    if bucket_left >= source_left && bucket_right <= source_right {
        // bucket is part of source
        return (bucket_right - bucket_left) / (source_right - source_left);
    }
    if bucket_left > source_left && bucket_left < source_right && bucket_right > source_right {
        // bucket is right partly outside source
        return (source_right - bucket_left) / (source_right - source_left);
    }
    0.0
}

fn lorentz(center: f64, width: f64, x: f64) -> f64 {
    0.5 / PI * width / ((0.25 * width * width) + (x - center) * (x - center))
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
            s.add_single_peak(Length::new::<meter>(2.0), 1.0).is_ok(),
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
            s.add_single_peak(Length::new::<meter>(2.25), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2], 1.0);
        assert_eq!(s.data[3], 1.0);
    }
    #[test]
    fn set_single_peak_additive() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        assert_eq!(s.data[2], 4.0);
    }
    #[test]
    fn set_single_peak_interp_additive() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.25), 1.0).unwrap();
        assert_eq!(s.data[2], 3.0);
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
            s.add_single_peak(Length::new::<meter>(1.0), 1.0).is_ok(),
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
            s.add_single_peak(Length::new::<meter>(0.5), 1.0).is_ok(),
            false
        );
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(4.0), 1.0).is_ok(),
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
            s.add_single_peak(Length::new::<meter>(1.5), -1.0).is_ok(),
            false
        );
    }
    #[test]
    fn add_lorentzian() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(50.0),
            Length::new::<meter>(0.1),
        )
        .unwrap();
        assert!(s.add_lorentzian_peak(Length::new::<meter>(25.0), Length::new::<meter>(0.5), 2.0).is_ok());
        assert!(f64::abs(s.total_energy() - 2.0) < 0.1)
    }
    #[test]
    fn add_lorentzian_neg_center() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(50.0),
            Length::new::<meter>(0.1),
        )
        .unwrap();
        assert!(s.add_lorentzian_peak(Length::new::<meter>(-25.0), Length::new::<meter>(0.5), 2.0).is_err());
    }
    #[test]
    fn add_lorentzian_neg_width() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(50.0),
            Length::new::<meter>(0.1),
        )
        .unwrap();
        assert!(s.add_lorentzian_peak(Length::new::<meter>(25.0), Length::new::<meter>(-0.5), 2.0).is_err());
    }
    #[test]
    fn add_lorentzian_neg_energy() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(50.0),
            Length::new::<meter>(0.1),
        )
        .unwrap();
        assert!(s.add_lorentzian_peak(Length::new::<meter>(25.0), Length::new::<meter>(0.5), -2.0).is_err());
    }
    #[test]
    fn total_energy() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn total_energy2() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s.add_single_peak(Length::new::<meter>(1.5), 1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn scale_vertical() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s.add_single_peak(Length::new::<meter>(2.5), 1.0).unwrap();
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
        s.add_single_peak(Length::new::<meter>(2.5), 1.0).unwrap();
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
        assert_eq!(v, vec![Some(1), Some(2), Some(3)]);
    }
    #[test]
    fn enclosing_interval_exact() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.0, 4.0);
        assert_eq!(v, vec![Some(1), Some(2), Some(3)]);
    }
    #[test]
    fn enclosing_interval_upper_bound() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(3.0, 5.0);
        assert_eq!(v, vec![Some(2), Some(3)]); // ranges are exclusive upper bound !
    }
    #[test]
    fn enclosing_interval_lower_bound() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(1.0, 3.0);
        assert_eq!(v, vec![Some(0), Some(1), Some(2)]);
    }
    #[test]
    fn enclosing_interval_small_range() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.6, 2.9);
        assert_eq!(v, vec![Some(1), Some(2)]);
    }
    #[test]
    fn enclosing_interval_right_outside() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(5.1, 7.0);
        assert_eq!(v, vec![None, None]);
    }
    #[test]
    fn enclosing_interval_right_partly_outside() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.1, 7.0);
        assert_eq!(v, vec![Some(1), Some(2), Some(3)]);
    }
    #[test]
    fn enclosing_interval_left_outside() {
        let s = Spectrum::new(
            Length::new::<meter>(5.0)..Length::new::<meter>(10.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(1.0, 4.9);
        assert_eq!(v, vec![None, None]);
    }
    #[test]
    fn enclosing_interval_left_partly_outside() {
        let s = Spectrum::new(
            Length::new::<meter>(5.0)..Length::new::<meter>(10.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let v = s.enclosing_interval(2.1, 6.5);
        assert_eq!(v, vec![Some(0), Some(1), Some(2)]);
    }
    #[test]
    fn calc_ratio_test() {
        assert_eq!(calc_ratio(1.0, 2.0, 3.0, 4.0), 0.0); // bucket completely outside
        assert_eq!(calc_ratio(1.0, 4.0, 2.0, 3.0), 1.0); // bucket contains source
        assert_eq!(calc_ratio(2.0, 3.0, 0.0, 4.0), 0.25); // bucket is part of source
        assert_eq!(calc_ratio(0.0, 1.0, 0.0, 2.0), 0.5); // bucket is part of source (matching left)
        assert_eq!(calc_ratio(1.0, 2.0, 0.0, 2.0), 0.5); // bucket is part of source (matching right)
        assert_eq!(calc_ratio(0.0, 2.0, 1.0, 3.0), 0.5); // bucket is left outside source
        assert_eq!(calc_ratio(0.0, 2.0, 1.0, 2.0), 1.0); // bucket is left outside source (matching)
        assert_eq!(calc_ratio(2.0, 4.0, 1.0, 3.0), 0.5); // bucket is right outside source
        assert_eq!(calc_ratio(1.0, 4.0, 1.0, 3.0), 1.0); // bucket is right outside source (matching)
        assert_eq!(calc_ratio(1.0, 2.0, 1.0, 2.0), 1.0); // bucket matches source
    }
    #[test]
    fn resample() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s1.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        let s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_interp() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(6.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, array![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_interp2() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(6.0),
            Length::new::<meter>(0.5),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, array![0.0, 1.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_right_outside() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(4.0)..Length::new::<meter>(6.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(4.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, array![0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
    }
    #[test]
    fn resample_left_outside() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(4.0)..Length::new::<meter>(6.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, array![0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
    }
}
