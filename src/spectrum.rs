#![warn(missing_docs)]
//! Module for handling optical spectra
use crate::error::{OpmResult, OpossumError};
use csv::ReaderBuilder;
use ndarray::Array1;
use ndarray_stats::QuantileExt;
use plotters::prelude::*;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use std::f64::consts::PI;
use std::fmt::{Debug, Display};
use std::fs::File;
use std::ops::Range;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::num_traits::Zero;
use uom::si::length::meter;
use uom::si::{f64::Length, length::nanometer};

/// Structure for handling spectral data.
///
/// This structure handles an array of values over a given wavelength range. Although the interface
/// is still limited, the structure is prepared for handling also non-equidistant wavelength slots.  
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Spectrum {
    data: Array1<(f64, f64)>, // (wavelength in meters, data in 1/meters)
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
    pub fn new(range: Range<Length>, resolution: Length) -> OpmResult<Self> {
        if resolution <= Length::zero() {
            return Err(OpossumError::Spectrum("resolution must be positive".into()));
        }
        if range.start >= range.end {
            return Err(OpossumError::Spectrum(
                "wavelength range must be in ascending order and not empty".into(),
            ));
        }
        if range.start <= Length::zero() || range.end <= Length::zero() {
            return Err(OpossumError::Spectrum(
                "wavelength range limits must both be positive".into(),
            ));
        }
        let lambdas = Array1::range(
            range.start.get::<meter>(),
            range.end.get::<meter>(),
            resolution.get::<meter>(),
        );
        let data = lambdas.map(|lambda| (*lambda, 0.0));
        Ok(Self { data })
    }
    /// Create a new [`Spectrum`] from a CSV (comma-separated values) file.
    ///
    /// Currently this function is relatively limited. The CSV file must have a specific format in
    /// order to be successfully parsed. It must be a file with two columns and `;` as separator.
    /// The first column corresponds to the wavelength in nm, the second columns represent values in
    /// percent. This file format corresponds to the CSV export format from an transmission (Excel) file
    /// as provided by Thorlabs.
    /// # Panics
    ///
    /// Panics if ???
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the file path is not found or could not be read.
    ///   - the file is empty.
    ///   - the file could not be parsed.
    pub fn from_csv(path: &str) -> OpmResult<Self> {
        let file = File::open(path).map_err(|e| OpossumError::Spectrum(e.to_string()))?;
        let mut reader = ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .from_reader(file);
        let mut datas: Vec<(f64, f64)> = Vec::new();
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
            datas.push((lambda * 1.0E-9, data * 0.01)); // (nanometers -> meters, percent -> transmisison)
        }
        if datas.is_empty() {
            return Err(OpossumError::Spectrum(
                "no csv data was found in file".into(),
            ));
        }
        Ok(Self {
            data: Array1::from_vec(datas),
        })
    }
    /// Returns the wavelength range of this [`Spectrum`].
    pub fn range(&self) -> Range<Length> {
        Length::new::<meter>(self.data.first().unwrap().0)
            ..Length::new::<meter>(self.data.last().unwrap().0)
    }
    /// Returns the average wavelenth resolution of this [`Spectrum`].
    ///
    /// The function estimates the spectral resolution from the bandwidth divided by the number of points.
    pub fn average_resolution(&self) -> Length {
        let r = self.range();
        let bandwidth = r.end - r.start;
        bandwidth / (self.data.len() as f64 - 1.0)
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
    pub fn add_single_peak(&mut self, wavelength: Length, value: f64) -> OpmResult<()> {
        let spectrum_range = self.data.first().unwrap().0..self.data.last().unwrap().0;
        if !spectrum_range.contains(&wavelength.get::<meter>()) {
            return Err(OpossumError::Spectrum(
                "wavelength is not in spectrum range".into(),
            ));
        }
        if value < 0.0 {
            return Err(OpossumError::Spectrum("energy must be positive".into()));
        }
        let wavelength_in_meters = wavelength.get::<meter>();
        let lambdas = self.data.map(|data| data.0);
        let idx = lambdas.iter().position(|w| *w >= wavelength_in_meters);
        if let Some(idx) = idx {
            if idx == 0 {
                let delta = lambdas.get(1).unwrap() - lambdas.get(0).unwrap();
                self.data[0].1 += value / delta;
            } else {
                let lower_lambda = lambdas.get(idx - 1).unwrap();
                let upper_lambda = lambdas.get(idx).unwrap();
                let delta = upper_lambda - lower_lambda;
                self.data[idx - 1].1 +=
                    value * (1.0 - (wavelength_in_meters - lower_lambda) / delta) / delta;
                self.data[idx].1 += value * (wavelength_in_meters - lower_lambda) / delta / delta;
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
    ) -> OpmResult<()> {
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
        self.data.mapv_inplace(|data| {
            (
                data.0,
                data.1 + energy * lorentz(wavelength_in_meters, width_in_meters, data.0),
            )
        });
        Ok(())
    }
    /// Returns the total energy of this [`Spectrum`].
    ///
    /// This function sums the values over all wavelength slots weighted with the individual slot widths. This
    /// way it also works for non-equidistant spectra.
    pub fn total_energy(&self) -> f64 {
        let lambda_deltas: Vec<f64> = self
            .data
            .windows(2)
            .into_iter()
            .map(|l| l[1].0 - l[0].0)
            .collect();
        let total_energy = lambda_deltas
            .into_iter()
            .zip(self.data.iter())
            .map(|d| d.0 * d.1 .1)
            .sum();
        total_energy
    }
    /// Scale the spectrum by a constant factor.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if the scaling factor is < 0.0.
    pub fn scale_vertical(&mut self, factor: f64) -> OpmResult<()> {
        if factor < 0.0 {
            return Err(OpossumError::Spectrum(
                "scaling factor mus be >= 0.0".into(),
            ));
        }
        self.data.mapv_inplace(|data| (data.0, data.1 * factor));
        Ok(())
    }
    /// Resample a provided [`Spectrum`] to match the given one.
    ///
    /// This function maps values and wavelengths of a provided spectrum to the structure of self. This function conserves the total
    /// energy if the the given interval is fully contained in self. This does not necessarily conserve peak widths or positions.  
    ///
    /// # Panics
    ///
    /// Panics if ???.
    pub fn resample(&mut self, spectrum: &Spectrum) {
        let mut src_it = spectrum.data.windows(2).into_iter();
        let src_interval = src_it.next();
        if src_interval.is_none() {
            return;
        }
        let mut src_lower = src_interval.unwrap()[0].0;
        let mut src_upper = src_interval.unwrap()[1].0;
        let mut src_idx: usize = 0;
        let lambdas_s = self.data.map(|data| data.0);
        let mut bucket_it = lambdas_s.windows(2).into_iter();
        let bucket_interval = bucket_it.next();
        if bucket_interval.is_none() {
            return;
        }
        let mut bucket_lower = bucket_interval.unwrap()[0];
        let mut bucket_upper = bucket_interval.unwrap()[1];
        let mut bucket_idx: usize = 0;
        self.data[bucket_idx].1 = 0.0;
        while src_upper < bucket_lower {
            if let Some(src_interval) = src_it.next() {
                src_lower = src_interval[0].0;
                src_upper = src_interval[1].0;
                src_idx += 1;
            } else {
                break;
            }
        }
        loop {
            let ratio = calc_ratio(bucket_lower, bucket_upper, src_lower, src_upper);
            let bucket_value = spectrum.data[src_idx].1 * ratio * (src_upper - src_lower)
                / (bucket_upper - bucket_lower);
            self.data[bucket_idx].1 += bucket_value;
            if src_upper < bucket_upper {
                if let Some(src_interval) = src_it.next() {
                    src_lower = src_interval[0].0;
                    src_upper = src_interval[1].0;
                    src_idx += 1;
                    continue;
                } else {
                    break;
                }
            } else if let Some(bucket_interval) = bucket_it.next() {
                bucket_lower = bucket_interval[0];
                bucket_upper = bucket_interval[1];
                bucket_idx += 1;
                self.data[bucket_idx].1 = 0.0;
                continue;
            } else {
                break;
            }
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
            .map(|d| (d.0 .0, d.0 .1 * d.1 .1))
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
            .map(|d| (d.0 .0, d.0 .1 + d.1 .1))
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
            .map(|d| {
                (
                    d.0 .0,
                    (d.0 .1 - d.1 .1).clamp(0.0, f64::abs(d.0 .1 - d.1 .1)),
                )
            })
            .collect();
    }
    /// Generate a plot of this [`Spectrum`].
    ///
    /// Generate a x/y spectrum plot as SVG graphics with the given filename. This function is meant mainly for debugging purposes.
    ///
    /// # Panics
    ///
    /// ???
    pub fn to_plot(&self, file_path: &std::path::Path) {
        let root = SVGBackend::new(file_path, (800, 600)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        let x_left = self.data.first().unwrap().0;
        let x_right = self.data.last().unwrap().0;
        let y_top = *self.data.map(|data| data.1).max_skipnan();
        let mut chart = ChartBuilder::on(&root)
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(x_left * 1.0E9..x_right * 1.0E9, 0.0..y_top * 1E-9)
            .unwrap();

        chart
            .configure_mesh()
            .x_desc("wavelength (nm)")
            .y_desc("value (1/nm)")
            .draw()
            .unwrap();
        chart
            .draw_series(LineSeries::new(
                self.data.map(|data| (data.0 * 1.0E9, data.1 * 1.0E-9)),
                &RED,
            ))
            .unwrap();
        root.present().unwrap();
    }
    /// Generate JSON representation.
    ///
    /// Generate a JSON representation of this [`Spectrum`]. This function is mainly used for generating reports.
    pub fn to_json(&self) -> serde_json::Value {
        let data_as_vec = self.data.to_vec();
        json!(data_as_vec)
    }
}
impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in self.data.iter() {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(Length::new::<meter>(value.0)),
                value.1
            )
            .unwrap();
        }
        write!(f, "\nTotal energy: {}", self.total_energy())
    }
}

impl Debug for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in self.data.iter() {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(Length::new::<meter>(value.0)),
                value.1
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
pub fn create_nd_glass_spectrum(energy: f64) -> Spectrum {
    let mut s = create_nir_spectrum();
    s.add_lorentzian_peak(
        Length::new::<nanometer>(1054.0),
        Length::new::<nanometer>(0.5),
        energy,
    )
    .unwrap();
    s
}
/// Helper function for adding two spectra.
///
/// This function allows for adding two (maybe non-existing = None) spectra with different bandwidth.
/// The resulting spectum is created such that both spectra are contained. The resolution corresponds
/// to the highest (average) resolution of both spectra. If one spectrum is `None` the other spectrum is
/// returned respectively. If both spectra a `None` then also `None`is returned.
pub fn merge_spectra(s1: Option<Spectrum>, s2: Option<Spectrum>) -> Option<Spectrum> {
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
            .average_resolution()
            .min(s2.as_ref().unwrap().average_resolution());
        let mut s_out = Spectrum::new(minimum..maximum, resolution).unwrap();
        s_out.resample(&s1.unwrap());
        s_out.add(&s2.unwrap());
        Some(s_out)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use approx::AbsDiffEq;
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
            s.as_ref().unwrap().data,
            array![
                (1.0, 0.0),
                (1.5, 0.0),
                (2.0, 0.0),
                (2.5, 0.0),
                (3.0, 0.0),
                (3.5, 0.0)
            ]
        );
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
        let s = Spectrum::from_csv("files_for_testing/spectrum/spec_to_csv_test_01.csv");
        assert!(s.is_ok());
        let lambdas = s.clone().unwrap().data.map(|data| data.0);
        assert!(lambdas
            .into_iter()
            .zip(array![500.0E-9, 501.0E-9, 502.0E-9, 503.0E-9, 504.0E-9, 505.0E-9].iter())
            .all(|x| x.0.abs_diff_eq(x.1, f64::EPSILON)));
        let datas = s.unwrap().data.map(|data| data.1);
        assert!(datas
            .into_iter()
            .zip(array![5.0E-01, 4.981E-01, 4.982E-01, 4.984E-01, 4.996E-01, 5.010E-01].iter())
            .all(|x| x.0.abs_diff_eq(x.1, f64::EPSILON)));
    }
    #[test]
    fn from_csv_err() {
        assert!(Spectrum::from_csv("wrong_path.csv").is_err());
        assert!(Spectrum::from_csv("files_for_testing/spectrum/spec_to_csv_test_02.csv").is_err());
        assert!(Spectrum::from_csv("files_for_testing/spectrum/spec_to_csv_test_03.csv").is_err());
        assert!(Spectrum::from_csv("files_for_testing/spectrum/spec_to_csv_test_04.csv").is_err());
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
        assert_eq!(prep().average_resolution().get::<meter>(), 0.5);
    }
    #[test]
    fn set_single_peak() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(2.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2].1, 2.0);
    }
    #[test]
    fn set_single_peak_interpolated() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(2.25), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[2].1, 1.0);
        assert_eq!(s.data[3].1, 1.0);
    }
    #[test]
    fn set_single_peak_additive() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        assert_eq!(s.data[2].1, 4.0);
    }
    #[test]
    fn set_single_peak_interp_additive() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s.add_single_peak(Length::new::<meter>(2.25), 1.0).unwrap();
        assert_eq!(s.data[2].1, 3.0);
        assert_eq!(s.data[3].1, 1.0);
    }
    #[test]
    fn set_single_peak_lower_bound() {
        let mut s = prep();
        assert_eq!(
            s.add_single_peak(Length::new::<meter>(1.0), 1.0).is_ok(),
            true
        );
        assert_eq!(s.data[0].1, 2.0);
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
        assert!(s.total_energy().abs_diff_eq(&2.0, 0.1));
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
        let data = s.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 0.25, 0.25, 0.0]);
    }
    #[test]
    fn scale_vertical_negative() {
        let mut s = prep();
        assert!(s.scale_vertical(-0.5).is_err());
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
        let mut s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_delete_old_data() {
        let mut s1 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s1.add_single_peak(Length::new::<meter>(3.0), 1.0).unwrap();
        let mut s2 = Spectrum::new(
            Length::new::<meter>(1.0)..Length::new::<meter>(5.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        s2.add_single_peak(Length::new::<meter>(2.0), 1.0).unwrap();
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
        let data = s1.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
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
        let data = s1.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 1.0, 0.0, 0.0]);
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
        let data = s1.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 0.0, 0.0]);
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
        let data = s1.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
    }
    #[test]
    fn add() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(Length::new::<meter>(2.25), 0.5).unwrap();
        s.add(&s2);
        let data = s.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 1.0, 1.5, 0.5, 0.0, 0.0]);
    }
    #[test]
    fn sub() {
        let mut s = prep();
        s.add_single_peak(Length::new::<meter>(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(Length::new::<meter>(2.25), 0.5).unwrap();
        s.sub(&s2);
        let data = s.data.map(|data| data.1);
        assert_eq!(data, array![0.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }
    #[test]
    fn serialize() {
        let s = prep();
        let s_yaml = serde_yaml::to_string(&s);
        assert!(s_yaml.is_ok());
        assert_eq!(s_yaml.unwrap(),
        "data:\n  v: 1\n  dim:\n  - 6\n  data:\n  - - 1.0\n    - 0.0\n  - - 1.5\n    - 0.0\n  - - 2.0\n    - 0.0\n  - - 2.5\n    - 0.0\n  - - 3.0\n    - 0.0\n  - - 3.5\n    - 0.0\n".to_string());
    }
    #[test]
    fn deserialize() {
        let s: std::result::Result<Spectrum, serde_yaml::Error>=serde_yaml::from_str("data:\n  v: 1\n  dim:\n  - 6\n  data:\n  - - 1.0\n    - 0.1\n  - - 1.5\n    - 0.2\n  - - 2.0\n    - 0.3\n  - - 2.5\n    - 0.4\n  - - 3.0\n    - 0.5\n  - - 3.5\n    - 0.6\n");
        assert!(s.is_ok());
        assert_eq!(
            s.unwrap().data,
            array![
                (1.0, 0.1),
                (1.5, 0.2),
                (2.0, 0.3),
                (2.5, 0.4),
                (3.0, 0.5),
                (3.5, 0.6)
            ]
        );
    }
}
