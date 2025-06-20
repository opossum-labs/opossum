#![warn(missing_docs)]
//! Module for creation and handling of optical spectra
use crate::{
    error::{OpmResult, OpossumError},
    lightdata::energy_data_builder::EnergyDataBuilder,
    micrometer,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    utils::{f64_to_usize, usize_to_f64},
};
use csv::ReaderBuilder;
use kahan::KahanSummator;
use log::warn;
use nalgebra::MatrixXx2;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::PI,
    fmt::{Debug, Display},
    fs::File,
    ops::Range,
    path::Path,
};
use uom::num_traits::Zero;
use uom::si::{f64::Length, length::micrometer, length::nanometer};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{energy::joule, f64::Energy},
};

/// Structure for handling spectral data.
///
/// This structure handles an array of values over a given wavelength range. Although the interface
/// is still limited, the structure is prepared for handling also non-equidistant wavelength slots.  
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Spectrum {
    data: Vec<(f64, f64)>, // (wavelength in micrometers, data in 1/micrometers)
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
        if range.start.is_sign_negative() || range.end.is_sign_negative() {
            return Err(OpossumError::Spectrum(
                "wavelength range limits must both be positive".into(),
            ));
        }
        let number_of_elements =
            f64_to_usize(((range.end - range.start) / resolution).value.round());
        let start = range.start.get::<micrometer>();
        let step = resolution.get::<micrometer>();
        let mut lambdas: Vec<f64> = Vec::new();
        for i in 0..number_of_elements {
            lambdas.push(usize_to_f64(i).mul_add(step, start));
        }
        let data = lambdas.iter().map(|lambda| (*lambda, 0.0)).collect();
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
    pub fn from_csv(path: &Path) -> OpmResult<Self> {
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
            datas.push((lambda * 1.0E-3, data * 0.01)); // (nanometers -> micrometers, percent -> transmisison)
        }
        if datas.is_empty() {
            return Err(OpossumError::Spectrum(
                "no csv data was found in file".into(),
            ));
        }
        Ok(Self { data: datas })
    }
    /// Generate a spectrum from a list of narrow laser lines (center wavelength, Energy) and a spectrum resolution.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the resolution is not positive
    /// - the wavelength is negative
    /// - the energy is negative
    /// - the list of lines is empty
    pub fn from_laser_lines(lines: Vec<(Length, Energy)>, resolution: Length) -> OpmResult<Self> {
        if lines.is_empty() {
            return Err(OpossumError::Spectrum("no laser lines provided".into()));
        }
        if resolution <= Length::zero() {
            return Err(OpossumError::Spectrum("resolution must be positive".into()));
        }
        let mut min_lambda = lines[0].0;
        let mut max_lambda = lines[0].0;
        for line in &lines {
            if line.0 < min_lambda {
                min_lambda = line.0;
            }
            if line.0 > max_lambda {
                max_lambda = line.0;
            }
        }
        let mut s = Self::new(min_lambda..max_lambda + 2.0 * resolution, resolution)?;
        for line in lines {
            s.add_single_peak(line.0, line.1.get::<joule>())?;
        }
        Ok(s)
    }
    fn lambda_vec(&self) -> Vec<f64> {
        self.data.iter().map(|data| data.0).collect()
    }
    /// Get a 1D vector of all y values.
    ///
    /// This is a convenience function for testing.
    #[must_use]
    pub fn data_vec(&self) -> Vec<f64> {
        self.data.iter().map(|data| data.1).collect()
    }
    /// Returns the wavelength range of this [`Spectrum`].
    ///
    /// # Panics
    ///
    /// This functions panics if the spectrum consists of only one single wavelength.
    #[must_use]
    pub fn range(&self) -> Range<Length> {
        micrometer!(self.data.first().unwrap().0)..micrometer!(self.data.last().unwrap().0)
    }
    /// Returns the average wavelenth resolution of this [`Spectrum`].
    ///
    /// The function estimates the spectral resolution from the bandwidth divided by the number of points.
    #[must_use]
    pub fn average_resolution(&self) -> Length {
        let r = self.range();
        let bandwidth = r.end - r.start;
        bandwidth / (usize_to_f64(self.data.len()) - 1.0)
    }
    /// Add a single peak to the given [`Spectrum`].
    ///
    /// This functions adds a single (resolution limited) peak to the [`Spectrum`] at the given wavelength and
    /// the given energy / intensity. If the given wavelength does not exactly match a spectrum slot the energy is distributed
    /// over neighboring slots such that the total energy matches the given energy.
    ///
    /// # Warnings
    ///
    /// This function emits a warning log entry if the peak wavelenth is not within the spectrum range. In this case the spectrum
    /// is unmodified.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the wavelength is outside the spectrum range
    ///   - the energy is negative
    pub fn add_single_peak(&mut self, wavelength: Length, value: f64) -> OpmResult<()> {
        let spectrum_range = self.range();
        if !spectrum_range.contains(&wavelength) {
            warn!("peak wavelength is not in spectrum range. Spectrum unmodified.");
            return Ok(());
        }
        if value < 0.0 {
            return Err(OpossumError::Spectrum("energy must be positive".into()));
        }
        let wavelength_in_micrometers = wavelength.get::<micrometer>();
        let lambdas: Vec<f64> = self.lambda_vec();
        if lambdas.len() < 2 {
            return Err(OpossumError::Spectrum("spectrum size is too small".into()));
        }
        let idx = lambdas.iter().position(|w| *w >= wavelength_in_micrometers);
        if let Some(idx) = idx {
            if idx == 0 {
                let delta = lambdas[1] - lambdas[0];
                self.data[0].1 += value / delta;
            } else {
                let lower_lambda = lambdas[idx - 1];
                let upper_lambda = lambdas[idx];
                let delta = upper_lambda - lower_lambda;
                let energy_per_micrometer = value / delta;
                let energy_part =
                    energy_per_micrometer * (wavelength_in_micrometers - lower_lambda) / delta;
                self.data[idx].1 += energy_part;
                self.data[idx - 1].1 += energy_per_micrometer - energy_part;
            }
            Ok(())
        } else {
            Err(OpossumError::Spectrum("insertion point not found".into()))
        }
    }
    /// Check if the [`Spectrum`] could server as a transmission spectrum.
    ///
    /// This functions checks if all values are in the range (0.0..=1.0)
    #[must_use]
    pub fn is_transmission_spectrum(&self) -> bool {
        self.data.iter().all(|d| (0.0..=1.0).contains(&d.1))
    }
    /// Returns the iterator of this [`Spectrum`].
    pub fn iter(&self) -> std::slice::Iter<'_, (f64, f64)> {
        self.data.iter()
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
        let wavelength_in_micrometers = center.get::<micrometer>();
        let width_in_micrometers = width.get::<micrometer>();
        let spectrum: Vec<(f64, f64)> = self
            .data
            .iter()
            .map(|data| {
                (
                    data.0,
                    energy.mul_add(
                        lorentz(wavelength_in_micrometers, width_in_micrometers, data.0),
                        data.1,
                    ),
                )
            })
            .collect();
        self.data = spectrum;
        Ok(())
    }
    /// Returns the total energy of this [`Spectrum`].
    ///
    /// This function sums the values over all wavelength slots weighted with the individual slot widths. This
    /// way it also works for non-equidistant spectra.
    #[must_use]
    pub fn total_energy(&self) -> f64 {
        let lambda_deltas = self.data.windows(2).map(|l| l[1].0 - l[0].0);
        let energies: Vec<f64> = lambda_deltas
            .zip(self.data.iter())
            .map(|d| d.0 * d.1.1)
            .collect();
        let kahan_sum: kahan::KahanSum<f64> = energies.iter().kahan_sum();
        kahan_sum.sum()
    }

    /// Returns the center wavelength of this [`Spectrum`].
    ///
    /// This function calculates the first moment of the spectral distribution.
    /// The calculated value represents the average wavelength and is therefore returned as the "center wavelength" of this [`Spectrum`].
    #[must_use]
    pub fn center_wavelength(&self) -> Length {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        for bin in self.data.windows(2) {
            let bin_width = bin[1].0 - bin[0].0;
            let bin_center = bin[0].0;
            let bin_weight = bin[0].1 * bin_width;
            weighted_sum += bin_center * bin_weight;
            total_weight += bin_weight;
        }
        micrometer!(weighted_sum / total_weight)
    }
    /// Return the value at a given wavelength.
    ///
    /// This function returns the spectrum value (y value) for a given wavelength. The value will be linear interpolated if the wavelength does not correspond
    /// to a defined wavelength bin. If the wavelength is outside the spectrum range `None` is returned.
    #[must_use]
    pub fn get_value(&self, wavelength: &Length) -> Option<f64> {
        let wvl_in_micrometer = wavelength.get::<micrometer>();
        if let Some(last) = self.data.last() {
            #[allow(clippy::float_cmp)]
            if wvl_in_micrometer == last.0 {
                return Some(last.1);
            }
        } else {
            return None;
        }

        let spectrum_range = self.range();
        if !spectrum_range.contains(wavelength) {
            return None;
        }
        let idx = self
            .lambda_vec()
            .iter()
            .position(|w| *w >= wvl_in_micrometer);
        idx.map(|idx| {
            let (data_left, data_right) = if idx == 0 {
                (self.data[idx], self.data[idx + 1])
            } else {
                (self.data[idx - 1], self.data[idx])
            };
            let ratio = (wvl_in_micrometer - data_left.0) / (data_right.0 - data_left.0);
            data_left.1.mul_add(1.0 - ratio, data_right.1 * ratio)
        })
    }
    /// Scale the spectrum by a constant factor.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if the scaling factor is < 0.0.
    pub fn scale_vertical(&mut self, factor: &f64) -> OpmResult<()> {
        if factor < &0.0 {
            return Err(OpossumError::Spectrum(
                "scaling factor mus be >= 0.0".into(),
            ));
        }
        let spectrum = self
            .data
            .iter()
            .map(|data| (data.0, data.1 * factor))
            .collect();
        self.data = spectrum;
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
    pub fn resample(&mut self, spectrum: &Self) {
        let mut src_it = spectrum.data.windows(2);
        let src_interval = src_it.next();
        if src_interval.is_none() {
            return;
        }
        let mut src_lower = src_interval.unwrap()[0].0;
        let mut src_upper = src_interval.unwrap()[1].0;
        let mut src_idx: usize = 0;
        let lambdas_s: Vec<f64> = self.lambda_vec();
        let mut bucket_it = lambdas_s.windows(2);
        let bucket_interval = bucket_it.next();
        if bucket_interval.is_none() {
            return;
        }
        let mut bucket_lower = bucket_interval.unwrap()[0];
        let mut bucket_upper = bucket_interval.unwrap()[1];
        let mut bucket_idx: usize = 0;
        self.data[bucket_idx].1 = 0.0;
        #[allow(clippy::while_float)]
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
                }
                break;
            } else if let Some(bucket_interval) = bucket_it.next() {
                bucket_lower = bucket_interval[0];
                bucket_upper = bucket_interval[1];
                bucket_idx += 1;
                self.data[bucket_idx].1 = 0.0;
                continue;
            }
            break;
        }
    }
    /// Filter the spectrum with another given spectrum by multiplying the data values. The given spectrum is resampled before the multiplication.
    pub fn filter(&mut self, filter_spectrum: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(filter_spectrum);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0.0, d.0.1 * d.1.1))
            .collect();
    }
    /// Filter a spectrum with a given filter type.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn filter_with_type(&mut self, filter_type: &crate::nodes::FilterType) -> OpmResult<()> {
        match filter_type {
            crate::nodes::FilterType::Constant(t) => self.scale_vertical(t)?,
            crate::nodes::FilterType::Spectrum(s2) => {
                self.filter(s2);
            }
        }
        Ok(())
    }
    /// Modify and generate spectrum for a beamsplitter.
    #[must_use]
    pub fn split_by_spectrum(&mut self, filter_spectrum: &Self) -> Self {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(filter_spectrum);
        let mut split_data = self.data.clone();
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0.0, d.0.1 * d.1.1))
            .collect();
        split_data = split_data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0.0, d.0.1 * (1.0 - d.1.1)))
            .collect();
        Self { data: split_data }
    }
    /// Modify the spectrum by a given function or closure.
    pub fn map_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut (f64, f64)) -> (f64, f64),
    {
        self.data = self.data.iter_mut().map(f).collect();
    }
    /// Add a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self.
    pub fn add(&mut self, spectrum_to_be_added: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_added);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0.0, d.0.1 + d.1.1))
            .collect();
    }
    /// Subtract a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self. **Note**: Negative values as result from the subtraction will be
    /// clamped to 0.0 (negative spectrum values are not allowed).
    pub fn sub(&mut self, spectrum_to_be_subtracted: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_subtracted);
        self.data = self
            .data
            .iter()
            .zip(resampled_spec.data.iter())
            .map(|d| (d.0.0, (d.0.1 - d.1.1).clamp(0.0, f64::abs(d.0.1 - d.1.1))))
            .collect();
    }
}

impl Plottable for Spectrum {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let data = self.data.clone();
        let mut spec_mat = MatrixXx2::zeros(data.len());
        for (i, s) in data.iter().enumerate() {
            spec_mat[(i, 0)] = s.0 * 1000.0; // micrometer -> nanometer
            spec_mat[(i, 1)] = s.1;
        }
        match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) | PlotType::Histogram2D(_) => {
                let plt_series = PlotSeries::new(
                    &PlotData::Dim2 { xy_data: spec_mat },
                    RGBAColor(255, 0, 0, 1.),
                    None,
                );
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("wavelength in nm".into()))?
            .set(&PlotArgs::YLabel("spectrum in arb. units".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Histogram2D(plt_params.clone())
    }
}

impl<'a> IntoIterator for &'a Spectrum {
    type IntoIter = std::slice::Iter<'a, (f64, f64)>;
    type Item = &'a (f64, f64);
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl From<Spectrum> for EnergyDataBuilder {
    fn from(spectrum: Spectrum) -> Self {
        Self::Raw(spectrum)
    }
}
impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in &self.data {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(micrometer!(value.0)),
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
        for value in &self.data {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(micrometer!(value.0)),
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
    0.5 / PI * width / (0.25 * width).mul_add(width, (x - center) * (x - center))
}

/// Helper function for adding two spectra.
///
/// This function allows for adding two (maybe non-existing = None) spectra with different bandwidth.
/// The resulting spectum is created such that both spectra are contained. The resolution corresponds
/// to the highest (average) resolution of both spectra. If one spectrum is `None` the other spectrum is
/// returned respectively. If both spectra a `None` then also `None`is returned.
///
/// # Panics
/// This function panics if a new spectrum cannot be created because of invalid resulting range or other internal errors.
#[must_use]
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
    use crate::{joule, nanometer};
    use crate::{
        spectrum_helper::{
            create_he_ne_spec, create_nd_glass_spec, create_nir_spec, create_visible_spec,
        },
        utils::test_helper::test_helper::check_logs,
    };
    use approx::{AbsDiffEq, assert_abs_diff_eq};
    use testing_logger;
    fn prep() -> Spectrum {
        Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(0.5)).unwrap()
    }
    #[test]
    fn new() {
        let s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(0.5));
        assert!(s.is_ok());
        assert_eq!(
            s.as_ref().unwrap().data,
            vec![
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
    fn from_csv_ok() {
        let s = Spectrum::from_csv(Path::new(
            "files_for_testing/spectrum/spec_to_csv_test_01.csv",
        ));
        assert!(s.is_ok());
        let s = s.unwrap();
        let lambdas = s.lambda_vec();
        assert!(
            lambdas
                .into_iter()
                .zip(vec![500.0E-3, 501.0E-3, 502.0E-3, 503.0E-3, 504.0E-3, 505.0E-3].iter())
                .all(|x| x.0.abs_diff_eq(x.1, f64::EPSILON))
        );
        let datas = s.data_vec();
        assert!(
            datas
                .into_iter()
                .zip(
                    vec![
                        5.0E-01, 4.981E-01, 4.982E-01, 4.984E-01, 4.996E-01, 5.010E-01
                    ]
                    .iter()
                )
                .all(|x| x.0.abs_diff_eq(x.1, f64::EPSILON))
        );
    }
    #[test]
    fn from_csv_err() {
        assert!(Spectrum::from_csv(Path::new("wrong_path.csv")).is_err());
        assert!(
            Spectrum::from_csv(Path::new(
                "files_for_testing/spectrum/spec_to_csv_test_02.csv"
            ))
            .is_err()
        );
        assert!(
            Spectrum::from_csv(Path::new(
                "files_for_testing/spectrum/spec_to_csv_test_03.csv"
            ))
            .is_err()
        );
        assert!(
            Spectrum::from_csv(Path::new(
                "files_for_testing/spectrum/spec_to_csv_test_04.csv"
            ))
            .is_err()
        );
    }
    #[test]
    fn from_laser_lines_single() {
        let s = Spectrum::from_laser_lines(vec![(micrometer!(1.0), joule!(1.0))], nanometer!(1.0))
            .unwrap();
        assert_eq!(s.total_energy(), 1.0);
        assert_abs_diff_eq!(s.data[0].0, 1.0);
        assert_abs_diff_eq!(s.data[1].0, 1.001);
        assert_abs_diff_eq!(s.data[0].1, 1000.0, epsilon = 1.0E-9);
        assert_abs_diff_eq!(s.data[1].1, 0.0);
    }
    #[test]
    fn from_laser_lines_double() {
        let s = Spectrum::from_laser_lines(
            vec![
                (micrometer!(1.0), joule!(1.0)),
                (micrometer!(1.010), joule!(0.5)),
            ],
            nanometer!(1.0),
        )
        .unwrap();
        assert_abs_diff_eq!(s.total_energy(), 1.5, epsilon = 1.0E-9);
        assert_abs_diff_eq!(s.data[0].0, 1.0);
        assert_abs_diff_eq!(s.data[0].1, 1000.0, epsilon = 1.0E-9);
        assert_abs_diff_eq!(s.data[1].0, 1.001);
        assert_abs_diff_eq!(s.data[1].1, 0.0);
        assert_abs_diff_eq!(s.data[2].0, 1.002);
        assert_abs_diff_eq!(s.data[2].1, 0.0);
        assert_abs_diff_eq!(s.data[10].0, 1.010);
        assert_abs_diff_eq!(s.data[10].1, 500.0, epsilon = 1.0E-9);
        assert_abs_diff_eq!(s.data[11].0, 1.011);
        assert_abs_diff_eq!(s.data[11].1, 0.0);
    }
    #[test]
    fn visible_spectrum() {
        let s = create_visible_spec();
        assert_eq!(s.lambda_vec().first().unwrap(), &0.38);
        assert_abs_diff_eq!(s.lambda_vec().last().unwrap(), &0.7499);
    }
    #[test]
    fn nir_spec() {
        assert_eq!(create_nir_spec().lambda_vec().first().unwrap(), &0.8);
    }
    #[test]
    fn nd_glass_spec() {
        let s = create_nd_glass_spec(1.0);
        assert!(s.is_ok());
        let s = s.unwrap();
        assert_eq!(s.lambda_vec().first().unwrap(), &0.8);
        assert!(create_nd_glass_spec(-1.0).is_err());
    }
    #[test]
    fn new_negative_resolution() {
        let s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(-0.5));
        assert!(s.is_err());
    }
    #[test]
    fn new_wrong_range() {
        let s = Spectrum::new(micrometer!(4.0)..micrometer!(1.0), micrometer!(0.5));
        assert!(s.is_err());
    }
    #[test]
    fn new_negative_range() {
        let s = Spectrum::new(micrometer!(-1.0)..micrometer!(4.0), micrometer!(0.5));
        assert!(s.is_err());
    }
    #[test]
    fn range() {
        let s = prep();
        assert_eq!(s.range(), micrometer!(1.0)..micrometer!(3.5))
    }
    #[test]
    fn estimate_resolution() {
        assert_eq!(prep().average_resolution().get::<micrometer>(), 0.5);
    }
    #[test]
    fn set_single_peak() {
        let mut s = prep();
        assert_eq!(s.add_single_peak(micrometer!(2.0), 1.0).is_ok(), true);
        assert_eq!(s.data[2].1, 2.0);
    }
    #[test]
    fn set_single_peak_interpolated() {
        let mut s = prep();
        assert_eq!(s.add_single_peak(micrometer!(2.25), 1.0).is_ok(), true);
        assert_eq!(s.data[2].1, 1.0);
        assert_eq!(s.data[3].1, 1.0);
    }
    #[test]
    fn set_single_peak_additive() {
        let mut s = prep();
        s.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        assert_eq!(s.data[2].1, 4.0);
    }
    #[test]
    fn set_single_peak_interp_additive() {
        let mut s = prep();
        s.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s.add_single_peak(micrometer!(2.25), 1.0).unwrap();
        assert_eq!(s.data[2].1, 3.0);
        assert_eq!(s.data[3].1, 1.0);
    }
    #[test]
    fn set_single_peak_lower_bound() {
        let mut s = prep();
        assert_eq!(s.add_single_peak(micrometer!(1.0), 1.0).is_ok(), true);
        assert_eq!(s.data[0].1, 2.0);
    }
    #[test]
    fn set_single_peak_wrong_params() {
        testing_logger::setup();
        let mut s = prep();
        assert!(s.add_single_peak(micrometer!(0.5), 1.0).is_ok());
        check_logs(
            log::Level::Warn,
            vec!["peak wavelength is not in spectrum range. Spectrum unmodified."],
        );
        assert!(s.add_single_peak(micrometer!(4.0), 1.0).is_ok());
        check_logs(
            log::Level::Warn,
            vec!["peak wavelength is not in spectrum range. Spectrum unmodified."],
        );
        assert!(s.add_single_peak(micrometer!(1.5), -1.0).is_err());
    }
    #[test]
    fn add_lorentzian() {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(50.0), micrometer!(0.1)).unwrap();
        assert!(
            s.add_lorentzian_peak(micrometer!(25.0), micrometer!(0.5), 2.0)
                .is_ok()
        );
        assert!(s.total_energy().abs_diff_eq(&2.0, 0.1));
    }
    #[test]
    fn add_lorentzian_wrong_params() {
        let mut s = prep();
        assert!(
            s.add_lorentzian_peak(micrometer!(-5.0), micrometer!(0.5), 2.0)
                .is_err()
        );
        assert!(
            s.add_lorentzian_peak(micrometer!(2.0), micrometer!(-0.5), 2.0)
                .is_err()
        );
        assert!(
            s.add_lorentzian_peak(micrometer!(2.0), micrometer!(0.5), -2.0)
                .is_err()
        );
    }
    #[test]
    fn total_energy() {
        let mut s = prep();
        s.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn total_energy_interpolated_peak() {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0)).unwrap();
        s.add_single_peak(micrometer!(1.5), 1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn get_value() {
        let s = Spectrum {
            data: vec![(1.0, 1.0), (2.0, 2.0), (3.0, 4.0)],
        };
        assert_eq!(s.get_value(&micrometer!(0.9)), None);
        assert_eq!(s.get_value(&micrometer!(1.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(1.2)), Some(1.2));
        assert_eq!(s.get_value(&micrometer!(2.0)), Some(2.0));
        assert_eq!(s.get_value(&micrometer!(2.75)), Some(3.5));
        assert_eq!(s.get_value(&micrometer!(3.0)), Some(4.0));
        assert_eq!(s.get_value(&micrometer!(3.1)), None);
    }
    #[test]
    fn get_value_empty() {
        let s = Spectrum { data: vec![] };
        assert_eq!(s.get_value(&micrometer!(1.0)), None);
        let s = Spectrum {
            data: vec![(1.0, 1.0)],
        };
        assert_eq!(s.get_value(&micrometer!(0.9)), None);
        assert_eq!(s.get_value(&micrometer!(1.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(1.1)), None);
    }
    #[test]
    fn scale_vertical() {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        s.add_single_peak(micrometer!(2.5), 1.0).unwrap();
        assert!(s.scale_vertical(&0.5).is_ok());
        assert_eq!(s.data_vec(), vec![0.0, 0.25, 0.25, 0.0]);
    }
    #[test]
    fn scale_vertical2() {
        let mut s = create_he_ne_spec(1.0).unwrap();
        let s2 = create_he_ne_spec(0.6).unwrap();
        s.scale_vertical(&0.6).unwrap();
        assert_eq!(s.total_energy(), s2.total_energy());
        // let mut expected_spectrum = s2.iter();
        // for value in s.iter() {
        //     assert_abs_diff_eq!(
        //         value.1,
        //         expected_spectrum.next().unwrap().1,
        //         epsilon = f64::EPSILON
        //     );
        // }
    }
    #[test]
    fn he_ne_spectrum() {
        let s = create_he_ne_spec(1.0).unwrap();
        assert_eq!(s.total_energy(), 1.0);
    }
    #[test]
    fn scale_vertical_negative() {
        let mut s = prep();
        assert!(s.scale_vertical(&-0.5).is_err());
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
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        s2.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_delete_old_data() {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        s1.add_single_peak(micrometer!(3.0), 1.0).unwrap();
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        s2.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_interp() {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(0.5)).unwrap();
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(6.0), micrometer!(1.0)).unwrap();
        s2.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.total_energy(), s2.total_energy());
        assert!(
            s1.data_vec()
                .iter()
                .zip(vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0])
                .all(|v| (*v.0).abs_diff_eq(&v.1, f64::EPSILON))
        );
    }
    #[test]
    fn resample_interp2() {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0)).unwrap();
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(6.0), micrometer!(0.5)).unwrap();
        s2.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 1.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), s2.total_energy());
    }
    #[test]
    fn resample_right_outside() {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0)).unwrap();
        let mut s2 = Spectrum::new(micrometer!(4.0)..micrometer!(6.0), micrometer!(1.0)).unwrap();
        s2.add_single_peak(micrometer!(4.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
    }
    #[test]
    fn resample_left_outside() {
        let mut s1 = Spectrum::new(micrometer!(4.0)..micrometer!(6.0), micrometer!(1.0)).unwrap();
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0)).unwrap();
        s2.add_single_peak(micrometer!(2.0), 1.0).unwrap();
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
    }
    #[test]
    fn add() {
        let mut s = prep();
        s.add_single_peak(micrometer!(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(micrometer!(2.25), 0.5).unwrap();
        s.add(&s2);
        assert_eq!(s.data_vec(), vec![0.0, 1.0, 1.5, 0.5, 0.0, 0.0]);
    }
    #[test]
    fn sub() {
        let mut s = prep();
        s.add_single_peak(micrometer!(1.75), 1.0).unwrap();
        let mut s2 = prep();
        s2.add_single_peak(micrometer!(2.25), 0.5).unwrap();
        s.sub(&s2);
        assert_eq!(s.data_vec(), vec![0.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }
    #[test]
    fn serialize() {
        let s = prep();
        let s_ron =
            ron::ser::to_string_pretty(&s, ron::ser::PrettyConfig::new().new_line("\n")).unwrap();
        assert_eq!(s_ron,
        "(\n    data: [\n        (1.0, 0.0),\n        (1.5, 0.0),\n        (2.0, 0.0),\n        (2.5, 0.0),\n        (3.0, 0.0),\n        (3.5, 0.0),\n    ],\n)".to_string());
    }
    #[test]
    fn deserialize() {
        let s: Spectrum =
            ron::from_str("(data:[(1.0, 0.1),(1.5,0.2),(2.0,0.3),(2.5,0.4),(3.0,0.5),(3.5,0.6)])")
                .unwrap();
        assert_eq!(
            s.data,
            vec![
                (1.0, 0.1),
                (1.5, 0.2),
                (2.0, 0.3),
                (2.5, 0.4),
                (3.0, 0.5),
                (3.5, 0.6)
            ]
        );
    }
    #[test]
    fn debug() {
        let s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0)).unwrap();
        assert_eq!(
            format!("{:?}", s),
            "1000.00 nm -> 0\n2000.00 nm -> 0\n3000.00 nm -> 0\n"
        );
    }
}
