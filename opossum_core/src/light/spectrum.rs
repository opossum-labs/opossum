#![warn(missing_docs)]
//! Module for creation and handling of optical spectra
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllNormal, AllNotEmpty, AllPositive, SecondLarger, XNormal, YFinite},
    light::lightdata::energy_data_builder::EnergyDataBuilder,
    micrometer,
    prelude::EnergyLaserLines,
    properties::Proptype,
    reporting::plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    utils::{to_f64, try_f64_to_usize},
    validated, validated_type, validated_vec, validated_vec_type,
};
use kahan::KahanSummator;
use log::warn;
use nalgebra::MatrixXx2;
use opm_macros_lib::EnsureValidated;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::PI,
    fmt::{Debug, Display},
    fs::File,
    io::{BufRead, BufReader},
    ops::Range,
    path::Path,
};
use uom::si::{f64::Length, length::micrometer, length::nanometer};
use uom::{fmt::DisplayStyle::Abbreviation, si::energy::joule};

/// Structure for handling spectral data.
///
/// This structure handles an array of values over a given wavelength range. Although the interface
/// is still limited, the structure is prepared for handling also non-equidistant wavelength slots.  
#[derive(Clone, Serialize, Deserialize, PartialEq, EnsureValidated)]
pub struct Spectrum {
    data: validated_vec_type!(
        Vec<(f64, f64)>,
        AllPositive && XNormal && YFinite,
        AllNotEmpty
    ), // (wavelength in micrometers, data in 1/micrometers)
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            data: validated_vec!(
                vec![(1054., 1.)],
                AllPositive && XNormal && YFinite,
                AllNotEmpty
            )
            .unwrap(),
        }
    }
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
        let resolution = Self::validated_resolution(resolution)?;
        let range = Self::validated_range(range)?;
        let Some(number_of_steps) = try_f64_to_usize(
            ((range.get().end - range.get().start) / *resolution.get())
                .value
                .round(),
        ) else {
            return Err(OpossumError::Spectrum(
                "cannot determine number of wavelength slots".into(),
            ));
        };
        let start = range.get().start.get::<micrometer>();
        let step = resolution.get().get::<micrometer>();
        let mut lambdas: Vec<f64> = Vec::new();
        for i in 0..=number_of_steps {
            lambdas.push(to_f64(i).mul_add(step, start));
        }
        let data = lambdas.iter().map(|lambda| (*lambda, 0.0)).collect();
        let mut spec = Self::default();
        spec.set_data(data)?;
        Ok(spec)
    }

    #[allow(clippy::type_complexity)]
    fn validated_resolution(
        resolution: Length,
    ) -> OpmResult<validated_type!(Length, AllNormal && AllPositive)> {
        validated!(resolution, AllNormal && AllPositive)
    }

    #[allow(clippy::type_complexity)]
    fn validated_range(
        range: Range<Length>,
    ) -> OpmResult<validated_type!(Range<Length>, AllNormal && AllPositive && SecondLarger)> {
        validated!(range, AllNormal && AllPositive && SecondLarger)
    }

    /// Set the data Vector of this [`Spectrum`]
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_data(&mut self, data: Vec<(f64, f64)>) -> OpmResult<()> {
        self.data.set(data)?;
        Ok(())
    }
    /// Create a new [`Spectrum`] from a text-based file (CSV, TSV, or space-separated).
    ///
    /// The file must contain exactly two columns per line. Columns can be separated by
    /// semicolons, commas, tabs, or spaces (even mixed). The first column corresponds to
    /// the wavelength in nm, the second column represents values in percent.
    /// The decimal separator must strictly be a dot `.`.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Spectrum`] if
    ///   - the file path is not found or could not be read.
    ///   - the file contains lines that do not resolve to exactly two columns.
    ///   - the values within the columns cannot be parsed into 64-bit floating-point numbers.
    ///   - no valid data was found in the file.
    pub fn from_csv(path: &Path) -> OpmResult<Self> {
        // Open the file and wrap it in a BufReader for efficient line-by-line reading
        let file = File::open(path).map_err(|e| OpossumError::Spectrum(e.to_string()))?;
        let reader = BufReader::new(file);
        let mut datas: Vec<(f64, f64)> = Vec::new();

        for (index, line_result) in reader.lines().enumerate() {
            let line = line_result.map_err(|e| OpossumError::Spectrum(e.to_string()))?;
            let line = line.trim();

            // Gracefully skip completely empty lines (e.g., at the end of the file)
            if line.is_empty() {
                continue;
            }

            // Split the line using an array of allowed delimiter characters
            // and filter out empty strings caused by consecutive delimiters.
            let tokens: Vec<&str> = line
                .split([';', ',', '\t', ' '])
                .filter(|s| !s.is_empty())
                .collect();

            // Enforce the strict rule of having exactly two columns
            if tokens.len() != 2 {
                return Err(OpossumError::Spectrum(format!(
                    "Invalid format at line {}: expected exactly 2 columns, found {}",
                    index + 1,
                    tokens.len()
                )));
            }

            // Parse the wavelength (first column)
            let lambda = tokens[0].parse::<f64>().map_err(|e| {
                OpossumError::Spectrum(format!(
                    "Line {}: Failed to parse wavelength ({})",
                    index + 1,
                    e
                ))
            })?;

            // Parse the data value (second column)
            let data = tokens[1].parse::<f64>().map_err(|e| {
                OpossumError::Spectrum(format!(
                    "Line {}: Failed to parse data value ({})",
                    index + 1,
                    e
                ))
            })?;

            // Convert units: nanometers -> micrometers, percent -> transmission (0.0 to 1.0)
            datas.push((lambda * 1.0E-3, data * 0.01));
        }

        // Ensure we actually collected some data
        if datas.is_empty() {
            return Err(OpossumError::Spectrum(
                "No valid data was found in the file".into(),
            ));
        }

        let mut spec = Self::default();
        spec.set_data(datas)?;
        Ok(spec)
    }

    ///Normalizes a spectrum such that its maximum value corresponds to 1
    /// # Errors
    /// This function errors if the maximum value is smaller or equal to zero
    pub fn normalize_to_max(&mut self) -> OpmResult<()> {
        let max_value = self
            .data
            .iter()
            .fold(0., |init, (_, val)| if *val > init { *val } else { init });
        if max_value > 0. {
            self.data.for_each(|(_, val)| *val /= max_value)
        } else {
            Err(OpossumError::Other(
                "Cannot normalize spectrum to its maximum value with a maximum value of zero!"
                    .into(),
            ))
        }
    }

    ///Normalizes a spectrum such that its sum corresponds to 1
    /// # Errors
    /// This function errors if the sum is smaller or equal to zero
    pub fn normalize_to_sum(&mut self) -> OpmResult<()> {
        let sum = self.data.iter().fold(0., |init, (_, val)| init + *val);
        if sum > 0. {
            self.data.for_each(|(_, val)| *val /= sum)
        } else {
            Err(OpossumError::Other(
                "Cannot normalize spectrum to its sum value with a sum value of zero!".into(),
            ))
        }
    }

    /// Generate a spectrum from a list of narrow [`EnergyLaserLines`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the resolution is not positive
    /// - the wavelength is negative
    /// - the energy is negative
    /// - the list of lines is empty
    pub fn from_laser_lines(lines: &EnergyLaserLines) -> OpmResult<Self> {
        //EnergyLaserLines is already a validated struct, no validation necessary
        let resolution = lines.spectral_resolution();
        let lines = lines.lines();
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
        let mut s = Self::new(min_lambda..max_lambda + 2.0 * *resolution, *resolution)?;
        for line in lines {
            s.add_single_peak(line.0, line.1.get::<joule>())?;
        }
        Ok(s)
    }
    /// Get the data vector
    ///
    /// This is a convenience function for testing.
    #[must_use]
    pub const fn data(&self) -> &Vec<(f64, f64)> {
        self.data.get()
    }

    /// Get a 1D vector of all wavelength values.
    #[must_use]
    pub fn lambda_vec(&self) -> Vec<f64> {
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
        bandwidth / (to_f64(self.data.len()) - 1.0)
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
                let mut dat1 = *self.data.get_at_index(idx)?;
                dat1.1 += value / delta;
                self.data.replace(idx, dat1)?;
            } else {
                let lower_lambda = lambdas[idx - 1];
                let upper_lambda = lambdas[idx];
                let delta = upper_lambda - lower_lambda;
                let energy_per_micrometer = value / delta;
                let energy_part =
                    energy_per_micrometer * (wavelength_in_micrometers - lower_lambda) / delta;
                let mut dat1 = *self.data.get_at_index(idx)?;
                dat1.1 += energy_part;
                self.data.replace(idx, dat1)?;
                let mut dat2 = *self.data.get_at_index(idx - 1)?;
                dat2.1 += energy_per_micrometer - energy_part;
                self.data.replace(idx - 1, dat2)?;
            }
            Ok(())
        } else {
            Err(OpossumError::Spectrum("insertion point not found".into()))
        }
    }
    /// Check if the [`Spectrum`] could serve as a transmission spectrum.
    ///
    /// This functions checks if all values are in the range (0.0..=1.0)
    #[must_use]
    pub fn is_transmission_spectrum(&self) -> bool {
        self.data.iter().all(|d| (0.0..=1.0).contains(&d.1))
    }

    /// Check if the [`Spectrum`] values are in a specific range.
    ///
    /// This functions checks if all values are in the range (0.0..=1.0)
    #[must_use]
    pub fn values_are_in_range(&self, min: f64, max: f64) -> bool {
        self.data.iter().all(|d| (min..=max).contains(&d.1))
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
        self.data.for_each(|data| {
            data.1 = energy.mul_add(
                lorentz(wavelength_in_micrometers, width_in_micrometers, data.0),
                data.1,
            );
        })?;
        Ok(())
    }
    /// Returns the total energy of this [`Spectrum`].
    ///
    /// This function sums the values over all wavelength slots weighted with the individual slot widths. This
    /// way it also works for non-equidistant spectra.
    #[must_use]
    pub fn total_energy(&self) -> f64 {
        let lambda_deltas = self.data.get().windows(2).map(|l| l[1].0 - l[0].0);
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
        for bin in self.data.get().windows(2) {
            let bin_width = bin[1].0 - bin[0].0;
            let bin_center = bin[0].0;
            let bin_weight = bin[0].1 * bin_width;
            weighted_sum = bin_center.mul_add(bin_weight, weighted_sum);
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
        let last = self.data.last()?;
        #[allow(clippy::float_cmp)]
        if wvl_in_micrometer == last.0 {
            return Some(last.1);
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
                "scaling factor must be >= 0.0".into(),
            ));
        }
        let spectrum = self
            .data
            .iter()
            .map(|data| (data.0, data.1 * factor))
            .collect::<Vec<(f64, f64)>>();
        self.set_data(spectrum)?;
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
        let mut src_it = spectrum.data.get().windows(2);
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

        // Initial reset of the first bucket
        self.data
            .replace(bucket_idx, (self.data[bucket_idx].0, 0.0))
            .unwrap();

        // Skip source intervals that end before the first bucket starts
        // Use a small epsilon to avoid skipping intervals that just barely touch/overlap
        #[allow(clippy::while_float)]
        while src_upper <= bucket_lower + f64::EPSILON {
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

            // Add contribution to current bucket
            self.data
                .replace(
                    bucket_idx,
                    (
                        self.data[bucket_idx].0,
                        self.data[bucket_idx].1 + bucket_value,
                    ),
                )
                .unwrap();

            // Logic to advance Source or Bucket
            // If source ends before (or at) bucket end, we are done with this source bin -> Advance Source
            if src_upper < bucket_upper + f64::EPSILON {
                if let Some(src_interval) = src_it.next() {
                    src_lower = src_interval[0].0;
                    src_upper = src_interval[1].0;
                    src_idx += 1;
                    continue;
                }
                break; // No more source
            }
            // If source extends beyond bucket, we are done with this bucket -> Advance Bucket
            else if let Some(bucket_interval) = bucket_it.next() {
                bucket_lower = bucket_interval[0];
                bucket_upper = bucket_interval[1];
                bucket_idx += 1;

                // Reset the NEW bucket before adding to it
                self.data
                    .replace(bucket_idx, (self.data[bucket_idx].0, 0.0))
                    .unwrap();
                continue;
            }
            break; // No more buckets
        }
    }
    /// Filter the spectrum with another given spectrum by multiplying the data values. The given spectrum is resampled before the multiplication.
    pub fn filter(&mut self, filter_spectrum: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(filter_spectrum);
        let _ = self.set_data(
            self.data
                .iter()
                .zip(resampled_spec.data.iter())
                .map(|d| (d.0.0, d.0.1 * d.1.1))
                .collect::<Vec<(f64, f64)>>(),
        );
    }
    /// Filter a spectrum with a given filter type.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn filter_with_type(&mut self, filter_type: &crate::nodes::FilterType) -> OpmResult<()> {
        match filter_type {
            crate::nodes::FilterType::Constant(t) => {
                self.scale_vertical(&t.transmission().value)?;
            }
            crate::nodes::FilterType::Spectrum(s2) => {
                self.filter(s2);
            }
        }
        Ok(())
    }
    /// Modify and generate spectrum for a beamsplitter.
    ///
    /// Returns the reflected/split part as a new Spectrum.
    /// Self is modified to represent the transmitted part.
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if the calculated spectrum values
    /// do not pass the validation.
    #[must_use]
    pub fn split_by_spectrum(&mut self, filter_spectrum: &Self) -> Self {
        // Resample to match the incoming spectrum
        let mut transmission_factors = self.clone();
        transmission_factors.resample(filter_spectrum);
        let mut transmitted_data = Vec::with_capacity(self.data.len());
        let mut reflected_data = Vec::with_capacity(self.data.len());

        for (input_bin, filter_bin) in self.data.iter().zip(transmission_factors.data.iter()) {
            let wavelength = input_bin.0;
            let input_energy = input_bin.1;
            let transmission = filter_bin.1.clamp(0.0, 1.0);
            let reflection = 1.0 - transmission;
            let transmitted_energy = input_energy * transmission;
            let reflected_energy = input_energy * reflection;

            transmitted_data.push((wavelength, transmitted_energy));
            reflected_data.push((wavelength, reflected_energy));
        }
        self.set_data(transmitted_data)
            .expect("Validation failed during split_by_spectrum update");
        let mut split_spectrum = Self::default();
        split_spectrum
            .set_data(reflected_data)
            .expect("Validation failed for split spectrum");

        split_spectrum
    }
    /// Add a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self.
    pub fn add(&mut self, spectrum_to_be_added: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_added);
        let _ = self.set_data(
            self.data
                .iter()
                .zip(resampled_spec.data.iter())
                .map(|d| (d.0.0, d.0.1 + d.1.1))
                .collect::<Vec<(f64, f64)>>(),
        );
    }
    /// Subtract a given spectrum.
    ///
    /// The given spectrum might be resampled in order to match self. **Note**: Negative values as result from the subtraction will be
    /// clamped to 0.0 (negative spectrum values are not allowed).
    pub fn sub(&mut self, spectrum_to_be_subtracted: &Self) {
        let mut resampled_spec = self.clone();
        resampled_spec.resample(spectrum_to_be_subtracted);
        let _ = self.set_data(
            self.data
                .iter()
                .zip(resampled_spec.data.iter())
                .map(|d| (d.0.0, (d.0.1 - d.1.1).clamp(0.0, f64::abs(d.0.1 - d.1.1))))
                .collect::<Vec<(f64, f64)>>(),
        );
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
            .set(&PlotArgs::PlotSize((1200, 800)))?
            .set(&PlotArgs::AxisEqual(false))?;

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

impl From<Spectrum> for Proptype {
    fn from(spectrum: Spectrum) -> Self {
        Self::Spectrum(spectrum)
    }
}

impl Display for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_length = Length::format_args(nanometer, Abbreviation);
        for value in self.data() {
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
        for (wavelength, value) in self.data() {
            writeln!(
                f,
                "{:7.2} -> {}",
                fmt_length.with(micrometer!(*wavelength)),
                value
            )?;
        }
        Ok(())
    }
}
fn calc_ratio(bucket_left: f64, bucket_right: f64, source_left: f64, source_right: f64) -> f64 {
    let overlap_start = f64::max(bucket_left, source_left);
    let overlap_end = f64::min(bucket_right, source_right);
    let overlap_width = overlap_end - overlap_start;
    let source_width = source_right - source_left;

    if overlap_width > 0.0 && source_width > 0.0 {
        overlap_width / source_width
    } else {
        0.0
    }
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
    use std::io::Write;

    use super::*;
    use crate::prelude::{EdgeFilter, EdgeFilterType, SpectralFilterBuilder};
    use crate::{joule, nanometer};
    use crate::{
        light::spectrum_helper::{
            create_he_ne_spec, create_nd_glass_spec, create_nir_spec, create_visible_spec,
        },
        utils::test_helper::test_helper::check_logs,
    };
    use approx::{AbsDiffEq, assert_abs_diff_eq, assert_relative_eq};
    use tempfile::NamedTempFile;
    use testing_logger;
    fn prep() -> OpmResult<Spectrum> {
        Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(0.5))
    }
    #[test]
    fn test_merge_spectra() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(0.5))?;
        let mut s2 = Spectrum::new(micrometer!(5.0)..micrometer!(8.0), micrometer!(0.5))?;
        s1.add_single_peak(micrometer!(1.), 1.)?;
        s2.add_single_peak(micrometer!(5.), 1.)?;

        let merged = merge_spectra(Some(s1), Some(s2)).unwrap();

        assert_relative_eq!(merged.average_resolution().value, micrometer!(0.5).value);
        assert_relative_eq!(merged.range().start.value, micrometer!(1.).value);
        assert_relative_eq!(merged.range().end.value, micrometer!(8.).value);
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(0.5));
        assert!(s.is_ok());
        assert_eq!(
            s.as_ref().unwrap().data.get(),
            &vec![
                (1.0, 0.0),
                (1.5, 0.0),
                (2.0, 0.0),
                (2.5, 0.0),
                (3.0, 0.0),
                (3.5, 0.0),
                (4.0, 0.0)
            ]
        );
        Ok(())
    }
    #[test]
    fn from_csv_ok() -> OpmResult<()> {
        let s = Spectrum::from_csv(Path::new(
            "files_for_testing/spectrum/spec_to_csv_test_01.csv",
        ))?;
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
        Ok(())
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
    // Helper function to create a temporary file populated with test data.
    // The file will be automatically deleted when the returned NamedTempFile goes out of scope.
    fn create_temp_spec_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temporary file");
        file.write_all(content.as_bytes())
            .expect("Failed to write to temporary file");
        file
    }

    #[test]
    fn test_from_csv_legacy_semicolon() {
        // Standard semicolon format (regression test)
        let content = "500.0;50.0\n600.0;100.0";
        let file = create_temp_spec_file(content);

        let result = Spectrum::from_csv(file.path());
        assert!(result.is_ok(), "Failed to parse standard semicolon CSV");
    }

    #[test]
    fn test_from_csv_comprehensive_stresstest() {
        // A comprehensive test containing:
        // Line 1: Standard comma with extreme spaces inside
        // Line 2: Tab separator with Windows style line ending (\r\n)
        // Line 3: Mixed delimiters (space and semicolon) with trailing whitespace
        // Line 4: Scientific notation with mixed tabs and spaces
        let content = "500.0                ,         50.0\r\n\
                       600.0\t100.0\n\
                       700.0 ; 85.5   \r\n\
                       8.0E2 \t , \t 9.0e1";

        let file = create_temp_spec_file(content);
        let result = Spectrum::from_csv(file.path());

        assert!(
            result.is_ok(),
            "Failed comprehensive stress test with mixed delimiters, line endings, and whitespace"
        );

        // Verify the content was parsed correctly into 4 distinct data points
        let spec = result.unwrap();
        assert_eq!(spec.data.len(), 4);
    }

    #[test]
    fn test_from_csv_mixed_whitespace_and_empty_lines() {
        // Testing consecutive spaces, mixed delimiters, and trailing empty lines
        let content = "500.0,   50.0\n\n600.0 \t ; 100.0\n   \n";
        let file = create_temp_spec_file(content);

        let result = Spectrum::from_csv(file.path());
        assert!(
            result.is_ok(),
            "Failed to handle mixed whitespace and empty lines gracefully"
        );
    }

    #[test]
    fn test_from_csv_exponential_notation() {
        // Testing scientific/exponential notation for float values
        let content = "5.0E2;5.0E1\n6.0e2;1.0e2";
        let file = create_temp_spec_file(content);

        let result = Spectrum::from_csv(file.path());
        assert!(result.is_ok(), "Failed to parse exponential float notation");
    }

    #[test]
    fn test_from_csv_invalid_columns_fail() {
        // Case 1: Too few columns (only 1)
        let content_too_few = "500.0\n600.0;100.0";
        let file_too_few = create_temp_spec_file(content_too_few);
        let result_too_few = Spectrum::from_csv(file_too_few.path());
        assert!(
            result_too_few.is_err(),
            "Expected error for missing column, but parsed successfully"
        );

        // Case 2: Too many columns (3 columns)
        let content_too_many = "500.0;50.0;extra_token\n600.0;100.0";
        let file_too_many = create_temp_spec_file(content_too_many);
        let result_too_many = Spectrum::from_csv(file_too_many.path());
        assert!(
            result_too_many.is_err(),
            "Expected error for extra column, but parsed successfully"
        );
    }

    #[test]
    fn test_from_csv_invalid_parse_fail() {
        // Testing alphabetical strings that cannot be parsed into f64
        let content = "wavelength;transmission\n500.0;50.0";
        let file = create_temp_spec_file(content);

        let result = Spectrum::from_csv(file.path());
        assert!(
            result.is_err(),
            "Expected parsing error due to text header, but it passed"
        );
    }
    #[test]
    fn from_laser_lines_single() -> OpmResult<()> {
        let s = Spectrum::from_laser_lines(&EnergyLaserLines::new(
            vec![(micrometer!(1.0), joule!(1.0))],
            nanometer!(1.0),
        )?)?;
        assert_eq!(s.total_energy(), 1.0);
        assert_abs_diff_eq!(s.data[0].0, 1.0);
        assert_abs_diff_eq!(s.data[1].0, 1.001);
        assert_abs_diff_eq!(s.data[0].1, 1000.0, epsilon = 1.0E-9);
        assert_abs_diff_eq!(s.data[1].1, 0.0);
        Ok(())
    }
    #[test]
    fn from_laser_lines_double() -> OpmResult<()> {
        let s = Spectrum::from_laser_lines(&EnergyLaserLines::new(
            vec![
                (micrometer!(1.0), joule!(1.0)),
                (micrometer!(1.010), joule!(0.5)),
            ],
            nanometer!(1.0),
        )?)?;
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
        Ok(())
    }
    #[test]
    fn visible_spectrum() -> OpmResult<()> {
        let s = create_visible_spec()?;
        assert_eq!(s.lambda_vec().first().unwrap(), &0.38);
        assert_abs_diff_eq!(s.lambda_vec().last().unwrap(), &0.750);
        Ok(())
    }
    #[test]
    fn nir_spec() -> OpmResult<()> {
        assert_eq!(create_nir_spec()?.lambda_vec().first().unwrap(), &0.8);
        Ok(())
    }
    #[test]
    fn nd_glass_spec() -> OpmResult<()> {
        let s = create_nd_glass_spec(1.0)?;
        assert_eq!(s.lambda_vec().first().unwrap(), &0.8);
        assert!(create_nd_glass_spec(-1.0).is_err());
        Ok(())
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
    fn range() -> OpmResult<()> {
        let s = prep()?;
        assert_eq!(s.range(), micrometer!(1.0)..micrometer!(4.0));
        Ok(())
    }
    #[test]
    fn estimate_resolution() -> OpmResult<()> {
        assert_eq!(prep()?.average_resolution().get::<micrometer>(), 0.5);
        Ok(())
    }
    #[test]
    fn set_single_peak() -> OpmResult<()> {
        let mut s = prep()?;
        assert_eq!(s.add_single_peak(micrometer!(2.0), 1.0).is_ok(), true);
        assert_eq!(s.data[2].1, 2.0);
        Ok(())
    }
    #[test]
    fn set_single_peak_interpolated() -> OpmResult<()> {
        let mut s = prep()?;
        assert_eq!(s.add_single_peak(micrometer!(2.25), 1.0).is_ok(), true);
        assert_eq!(s.data[2].1, 1.0);
        assert_eq!(s.data[3].1, 1.0);
        Ok(())
    }
    #[test]
    fn set_single_peak_additive() -> OpmResult<()> {
        let mut s = prep()?;
        s.add_single_peak(micrometer!(2.0), 1.0)?;
        s.add_single_peak(micrometer!(2.0), 1.0)?;
        assert_eq!(s.data[2].1, 4.0);
        Ok(())
    }
    #[test]
    fn set_single_peak_interp_additive() -> OpmResult<()> {
        let mut s = prep()?;
        s.add_single_peak(micrometer!(2.0), 1.0)?;
        s.add_single_peak(micrometer!(2.25), 1.0)?;
        assert_eq!(s.data[2].1, 3.0);
        assert_eq!(s.data[3].1, 1.0);
        Ok(())
    }
    #[test]
    fn set_single_peak_lower_bound() -> OpmResult<()> {
        let mut s = prep()?;
        assert_eq!(s.add_single_peak(micrometer!(1.0), 1.0).is_ok(), true);
        assert_eq!(s.data[0].1, 2.0);
        Ok(())
    }
    #[test]
    fn set_single_peak_wrong_params() -> OpmResult<()> {
        testing_logger::setup();
        let mut s = prep()?;
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
        Ok(())
    }
    #[test]
    fn add_lorentzian() -> OpmResult<()> {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(50.0), micrometer!(0.1))?;
        assert!(
            s.add_lorentzian_peak(micrometer!(25.0), micrometer!(0.5), 2.0)
                .is_ok()
        );
        assert!(s.total_energy().abs_diff_eq(&2.0, 0.1));
        Ok(())
    }
    #[test]
    fn add_lorentzian_wrong_params() -> OpmResult<()> {
        let mut s = prep()?;
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
        Ok(())
    }
    #[test]
    fn total_energy() -> OpmResult<()> {
        let mut s = prep()?;
        s.add_single_peak(micrometer!(2.0), 1.0)?;
        assert_eq!(s.total_energy(), 1.0);
        Ok(())
    }
    #[test]
    fn total_energy_interpolated_peak() -> OpmResult<()> {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0))?;
        s.add_single_peak(micrometer!(1.5), 1.0)?;
        assert_eq!(s.total_energy(), 1.0);
        Ok(())
    }
    #[test]
    fn get_value() -> OpmResult<()> {
        let mut s = Spectrum::default();
        let data = vec![(1.0, 1.0), (2.0, 2.0), (3.0, 4.0)];
        s.set_data(data)?;
        assert_eq!(s.get_value(&micrometer!(0.9)), None);
        assert_eq!(s.get_value(&micrometer!(1.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(1.2)), Some(1.2));
        assert_eq!(s.get_value(&micrometer!(2.0)), Some(2.0));
        assert_eq!(s.get_value(&micrometer!(2.75)), Some(3.5));
        assert_eq!(s.get_value(&micrometer!(3.0)), Some(4.0));
        assert_eq!(s.get_value(&micrometer!(3.1)), None);
        Ok(())
    }
    #[test]
    fn scale_vertical() -> OpmResult<()> {
        let mut s = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        s.add_single_peak(micrometer!(2.5), 1.0)?;
        assert!(s.scale_vertical(&0.5).is_ok());
        assert_eq!(s.data_vec(), vec![0.0, 0.25, 0.25, 0.0, 0.0]);
        Ok(())
    }
    #[test]
    fn scale_vertical2() -> OpmResult<()> {
        let mut s = create_he_ne_spec(1.0)?;
        let s2 = create_he_ne_spec(0.6)?;
        s.scale_vertical(&0.6)?;
        assert_eq!(s.total_energy(), s2.total_energy());
        Ok(())
    }
    #[test]
    fn he_ne_spectrum() -> OpmResult<()> {
        let s = create_he_ne_spec(1.0)?;
        assert_eq!(s.total_energy(), 1.0);
        Ok(())
    }
    #[test]
    fn scale_vertical_negative() -> OpmResult<()> {
        let mut s = prep()?;
        assert!(s.scale_vertical(&-0.5).is_err());
        Ok(())
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
    fn resample() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        s2.add_single_peak(micrometer!(2.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
        Ok(())
    }
    #[test]
    fn resample_delete_old_data() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        s1.add_single_peak(micrometer!(3.0), 1.0)?;
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        s2.add_single_peak(micrometer!(2.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.data, s2.data);
        assert_eq!(s1.total_energy(), s2.total_energy());
        Ok(())
    }
    #[test]
    fn resample_interp() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(0.5))?;
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(6.0), micrometer!(1.0))?;
        s2.add_single_peak(micrometer!(2.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.total_energy(), s2.total_energy());
        assert!(
            s1.data_vec()
                .iter()
                .zip(vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0])
                .all(|v| (*v.0).abs_diff_eq(&v.1, f64::EPSILON))
        );
        Ok(())
    }
    #[test]
    fn resample_interp2() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(5.0), micrometer!(1.0))?;
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(6.0), micrometer!(0.5))?;
        s2.add_single_peak(micrometer!(2.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), s2.total_energy());
        Ok(())
    }
    #[test]
    fn resample_right_outside() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0))?;
        let mut s2 = Spectrum::new(micrometer!(4.0)..micrometer!(6.0), micrometer!(1.0))?;
        s2.add_single_peak(micrometer!(4.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
        Ok(())
    }
    #[test]
    fn resample_left_outside() -> OpmResult<()> {
        let mut s1 = Spectrum::new(micrometer!(4.0)..micrometer!(6.0), micrometer!(1.0))?;
        let mut s2 = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0))?;
        s2.add_single_peak(micrometer!(2.0), 1.0)?;
        s1.resample(&s2);
        assert_eq!(s1.data_vec(), vec![0.0, 0.0, 0.0]);
        assert_eq!(s1.total_energy(), 0.0);
        Ok(())
    }
    #[test]
    fn add() -> OpmResult<()> {
        let mut s = prep()?;
        s.add_single_peak(micrometer!(1.75), 1.0)?;
        let mut s2 = prep()?;
        s2.add_single_peak(micrometer!(2.25), 0.5)?;
        s.add(&s2);
        assert_eq!(s.data_vec(), vec![0.0, 1.0, 1.5, 0.5, 0.0, 0.0, 0.0]);
        Ok(())
    }
    #[test]
    fn sub() -> OpmResult<()> {
        let mut s = prep()?;
        s.add_single_peak(micrometer!(1.75), 1.0)?;
        let mut s2 = prep()?;
        s2.add_single_peak(micrometer!(2.25), 0.5)?;
        s.sub(&s2);
        assert_eq!(s.data_vec(), vec![0.0, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0]);
        Ok(())
    }
    #[test]
    fn serialize() -> OpmResult<()> {
        let s = prep()?;
        let s_ron =
            ron::ser::to_string_pretty(&s, ron::ser::PrettyConfig::new().new_line("\n")).unwrap();
        assert_eq!(
            s_ron,
            "(
    data: (
        values: [
            (1.0, 0.0),
            (1.5, 0.0),
            (2.0, 0.0),
            (2.5, 0.0),
            (3.0, 0.0),
            (3.5, 0.0),
            (4.0, 0.0),
        ],
    ),
)"
            .to_string()
        );
        Ok(())
    }
    #[test]
    fn deserialize() {
        let s: Spectrum = ron::from_str(
            "(
    data: (
        values: [
            (1.0, 0.1),
            (1.5, 0.2),
            (2.0, 0.3),
            (2.5, 0.4),
            (3.0, 0.5),
            (3.5, 0.6),
        ],
    ),
)",
        )
        .unwrap();
        assert_eq!(
            s.data.get(),
            &vec![
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
    fn debug() -> OpmResult<()> {
        let s = Spectrum::new(micrometer!(1.0)..micrometer!(4.0), micrometer!(1.0))?;
        assert_eq!(
            format!("{:?}", s),
            "1000.00 nm -> 0\n2000.00 nm -> 0\n3000.00 nm -> 0\n4000.00 nm -> 0\n"
        );
        Ok(())
    }
    #[test]
    fn split_by_spectrum() -> OpmResult<()> {
        let edge_filter = EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(1000.0),
            0.0..1.0,
            Some(nanometer!(0.4)),
            nanometer!(900.0)..nanometer!(1100.0),
            nanometer!(0.2),
        )?;
        let longpass = SpectralFilterBuilder::EdgeFilter(edge_filter).build()?;
        let mut input_laser = Spectrum::from_laser_lines(&EnergyLaserLines::new(
            vec![(nanometer!(1050.0), joule!(100.0))],
            nanometer!(5.0),
        )?)?;
        let split_spectrum = input_laser.split_by_spectrum(&longpass);
        assert_abs_diff_eq!(input_laser.total_energy(), 100.0);
        assert_abs_diff_eq!(split_spectrum.total_energy(), 0.0);
        Ok(())
    }
    #[test]
    fn split_by_spectrum_at_longpass_edge() -> OpmResult<()> {
        let edge_filter = EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(1000.0),
            0.0..1.0,
            Some(nanometer!(1.0)),
            nanometer!(900.0)..nanometer!(1100.0),
            nanometer!(0.2),
        )?;
        let longpass = SpectralFilterBuilder::EdgeFilter(edge_filter).build()?;
        let mut input_laser = Spectrum::from_laser_lines(&EnergyLaserLines::new(
            vec![(nanometer!(1000.0), joule!(100.0))],
            nanometer!(0.2),
        )?)?;
        let split_spectrum = input_laser.split_by_spectrum(&longpass);
        assert_abs_diff_eq!(input_laser.total_energy(), 50.0, epsilon = 1.0e-8);
        assert_abs_diff_eq!(split_spectrum.total_energy(), 50.0, epsilon = 1.0e-8);
        Ok(())
    }
}
