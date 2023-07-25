//! Module for handling optical spectra
use crate::error::OpossumError;
use csv::ReaderBuilder;
use ndarray::Array1;
use ndarray_stats::QuantileExt;
use std::f64::consts::PI;
use std::fmt::{Debug, Display};
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::length::meter;
use uom::si::{f64::Length, length::nanometer};
type Result<T> = std::result::Result<T, OpossumError>;
use plotters::prelude::*;
use std::fs::File;

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
    pub fn from_csv(path: &str) -> Result<Self> {
        let file = File::open(path).map_err(|e| OpossumError::Spectrum(e.to_string()))?;
        let mut reader = ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .from_reader(file);
        let mut lambdas: Vec<f64> = Vec::new();
        let mut datas: Vec<f64> = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| OpossumError::Spectrum(e.to_string()))?;
            let lambda = record
                .get(0)
                .unwrap()
                .parse::<f64>()
                .map_err(|e| OpossumError::Spectrum(e.to_string()))?;
            let data = record
                .get(1)
                .unwrap()
                .parse::<f64>()
                .map_err(|e| OpossumError::Spectrum(e.to_string()))?;
            lambdas.push(lambda * 1.0E-9); // nanometers -> meters
            datas.push(data * 0.01); // percent -> transmisison
        }
        if lambdas.is_empty() {
            return Err(OpossumError::Spectrum(
                "no csv data was found in file".into(),
            ));
        }
        Ok(Self {
            data: Array1::from_vec(datas),
            lambdas: Array1::from_vec(lambdas),
        })
    }
    /// Returns the wavelength range of this [`Spectrum`].
    pub fn range(&self) -> Range<Length> {
        Length::new::<meter>(*self.lambdas.first().unwrap())
            ..Length::new::<meter>(*self.lambdas.last().unwrap())
    }
    pub fn estimate_resolution(&self) -> Length {
        let r = self.range();
        let bandwidth = r.end - r.start;
        bandwidth / (self.lambdas.len() as f64)
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
    /// Add a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self.
    pub fn add(&mut self, spectrum_to_be_added: &Spectrum) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_added);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| d.0 + d.1)
            .collect();
    }
    /// Subtract a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self. **Note**: Negative values as result from the subtraction will be
    /// clamped to 0.0 (negative spectrum values are not allowed).
    pub fn sub(&mut self, spectrum_to_be_subtracted: &Spectrum) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_subtracted);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0 - d.1).clamp(0.0, f64::abs(d.0 - d.1)))
            .collect();
    }
    pub fn to_plot(&self, filename: &str) {
        let root = SVGBackend::new(filename, (640, 480)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        let x_left = *self.lambdas.first().unwrap();
        let x_right = *self.lambdas.last().unwrap();
        let y_top = *self.data.max_skipnan();
        let mut chart = ChartBuilder::on(&root)
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(x_left * 1.0E9..x_right * 1.0E9, 0.0..y_top)
            .unwrap();

        chart
            .configure_mesh()
            .x_desc("wavelength (nm)")
            .draw()
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.lambdas
                    .iter()
                    .zip(self.data.iter())
                    .map(|x| (*x.0 * 1.0E9, *x.1)),
                &RED,
            ))
            .unwrap();
        root.present().unwrap();
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

impl Debug for Spectrum {
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
        Ok(())
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

/// Helper function for generating a visible spectrum.
///
/// This function generates an empty spectrum in the visible range (350 - 750 nm) with a resolution
/// of 0.1 nm.
pub fn create_visible_spectrum() -> Spectrum {
    Spectrum::new(
        Length::new::<nanometer>(380.0)..Length::new::<nanometer>(750.0),
        Length::new::<nanometer>(0.1),
    )
    .unwrap()
}
/// Helper function for generating a near infrared spectrum.
///
/// This function generates an empty spectrum in the near infrared range (800 - 2500 nm) with a resolution
/// of 0.1 nm.
pub fn create_nir_spectrum() -> Spectrum {
    Spectrum::new(
        Length::new::<nanometer>(800.0)..Length::new::<nanometer>(2500.0),
        Length::new::<nanometer>(0.1),
    )
    .unwrap()
}
/// Helper function for generating a spectrum of a narrow-band HeNe laser.
///
/// This function generates an spectrum in the visible range (350 - 750 nm) with a resolution
/// of 0.1 nm and a (spectrum resolution limited) laser line at 632.816 nm.
pub fn create_he_ne_spectrum(energy: f64) -> Spectrum {
    let mut s = create_visible_spectrum();
    s.add_single_peak(Length::new::<nanometer>(632.816), energy)
        .unwrap();
    s
}
/// Helper function for generating a spectrum of a narrow-band Nd:glass laser.
///
/// This function generates an spectrum in the near infrared range (800 - 2500 nm) with a resolution
/// of 0.1 nm and a (Lorentzian) laser line at 1054 nm with a width of 0.5 nm.
pub fn create_yb_yag_spectrum(energy: f64) -> Spectrum {
    let mut s = create_nir_spectrum();
    s.add_lorentzian_peak(
        Length::new::<nanometer>(1054.0),
        Length::new::<nanometer>(0.5),
        energy,
    )
    .unwrap();
    s
}
pub fn unify_spectrum(s1: Option<Spectrum>, s2: Option<Spectrum>) -> Option<Spectrum> {
    if s1.is_none() && s2.is_none() {
        None
    } else if s1.is_some() && s2.is_none() {
        s1
    } else if s1.is_none() && s2.is_some() {
        s2
    } else {
        let s1_range = s1.as_ref().unwrap().range();
        let s2_range = s2.as_ref().unwrap().range();
        let minimum = s1_range.start.min(s2_range.start);
        let maximum = s1_range.end.max(s2_range.end);
        let resolution = s1
            .as_ref()
            .unwrap()
            .estimate_resolution()
            .min(s2.as_ref().unwrap().estimate_resolution());
        let mut s_out = Spectrum::new(minimum..maximum, resolution).unwrap();
        s_out.resample(&s1.unwrap());
        s_out.add(&s2.unwrap());
        Some(s_out)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use ndarray::array;
    fn prep() -> Spectrum {
        Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        )
        .unwrap()
    }
    #[test]
    fn new() {
        let s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        );
        assert!(s.is_ok());
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
        assert!(s.is_err());
    }
    #[test]
    fn new_wrong_range() {
        let s = Spectrum::new(
            Length::new::<meter>(4.0)..Length::new::<meter>(1.0),
            Length::new::<meter>(0.5),
        );
        assert!(s.is_err());
    }
    #[test]
    fn new_negative_range() {
        let s = Spectrum::new(
            Length::new::<meter>(-1.0)..Length::new::<meter>(4.0),
            Length::new::<meter>(0.5),
        );
        assert!(s.is_err());
    }
    #[test]
    fn from_csv_ok() {
        let s = Spectrum::from_csv("spectrum_test/spec_to_csv_test_01.csv");
        assert!(s.is_ok());
        let lambdas = s.unwrap().lambdas;
        assert!(lambdas
            .into_iter()
            .zip(array![500.0E-9, 501.0E-9, 502.0E-9, 503.0E-9, 504.0E-9, 505.0E-9].iter())
            .all(|x| f64::abs(x.0 - *x.1) < 1.0E-16));
        let s = Spectrum::from_csv("spectrum_test/spec_to_csv_test_01.csv");
        let datas = s.unwrap().data;
        assert!(datas
            .into_iter()
            .zip(array![5.0E-01, 4.981E-01, 4.982E-01, 4.984E-01, 4.996E-01, 5.010E-01].iter())
            .all(|x| f64::abs(x.0 - *x.1) < 1.0E-16))
    }
    #[test]
    fn from_csv_err() {
        assert!(Spectrum::from_csv("wrong_path.csv").is_err());
        assert!(Spectrum::from_csv("spectrum_test/spec_to_csv_test_02.csv").is_err());
        assert!(Spectrum::from_csv("spectrum_test/spec_to_csv_test_03.csv").is_err());
        assert!(Spectrum::from_csv("spectrum_test/spec_to_csv_test_04.csv").is_err());
    }
    #[test]
    fn range() {
        let s = prep();
        assert_eq!(
            s.range(),
            Length::new::<meter>(1.0)..Length::new::<meter>(3.5)
        )
    }
    #[test]
    fn estimate_resolution() {
        todo!()
    }
    #[test]
    fn set_single_peak() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(2.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2], 2.0);
    }
    #[test]
    fn set_single_peak_interpolated() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(2.25), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2], 1.0);
        assert_eq!(s.data[3], 1.0);
    }
    #[test]
    fn set_single_peak_additive() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        assert_eq!(s.data[2], 4.0);
    }
    #[test]
    fn set_single_peak_interp_additive() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.25), 1.0).unwrap();
        assert_eq!(s.data[2], 3.0);
        assert_eq!(s.data[3], 1.0);
    }
    #[test]
    fn set_single_peak_lower_bound() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(1.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[0], 2.0);
    }
    #[test]
    fn set_single_peak_wrong_params() {
        let mut s = prep();
        assert!(s.add_single_peak(Length::new::<meter>(0.5), 1.0).is_err());
        assert!(s.add_single_peak(Length::new::<meter>(4.0), 1.0).is_err());
        assert!(s.add_single_peak(Length::new::<meter>(1.5), -1.0).is_err());
    }
    #[test]
    fn add_lorentzian() {
        let mut s = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(50.0),
            Length::new::<meter>(0.1),
        )
        .unwrap();
        assert!(s
            .add_lorentzian_peak(Length::new::<meter>(25.0), Length::new::<meter>(0.5), 2.0)
            .is_ok());
        assert!(f64::abs(s.total_energy() - 2.0) < 0.1)
    }
    #[test]
    fn add_lorentzian_wrong_params() {
        let mut s = prep();
        assert!(s
            .add_lorentzian_peak(Length::new::<meter>(-5.0), Length::new::<meter>(0.5), 2.0)
            .is_err());
        assert!(s
            .add_lorentzian_peak(Length::new::<meter>(2.0), Length::new::<meter>(-0.5), 2.0)
            .is_err());
        assert!(s
            .add_lorentzian_peak(Length::new::<meter>(2.0), Length::new::<meter>(0.5), -2.0)
            .is_err());
    }
    #[test]
    fn total_energy() {
        let mut s = prep();
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
        assert!(s.scale_vertical(0.5).is_ok());
        assert_eq!(s.data, array![0.0, 0.25, 0.25, 0.0]);
    }
    #[test]
    fn scale_vertical_negative() {
        let mut s = prep();
        assert!(s.scale_vertical(-0.5).is_err());
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
    #[test]
    fn add() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(Length::new::<meter>(2.25), 0.5).unwrap();
        s.add(&s2);
        assert_eq!(s.data, array![0.0, 1.0, 1.5, 0.5, 0.0, 0.0]);
    }
    #[test]
    fn sub() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(Length::new::<meter>(2.25), 0.5).unwrap();
        s.sub(&s2);
        assert_eq!(s.data, array![0.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }
}
