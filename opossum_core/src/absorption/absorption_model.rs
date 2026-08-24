//! Module for optical absorption models in `opossum_core`.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Length, LinearNumberDensity},
    length::meter,
    linear_number_density::per_meter,
};

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllPositive},
    light::Spectrum,
    validated, validated_type,
};

/// Defines the absorption model for an optical material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AbsorptionModel {
    /// No absorption (perfectly transparent material).
    None,

    /// Wavelength- and path-length-independent attenuation factor [0.0, 1.0].
    /// Applies a flat intensity reduction upon surface interaction or transit.
    ConstantAttenuation(validated_type!(f64, AllFinite && AllPositive)),

    /// Wavelength-independent absorption coefficient for the Lambert-Beer law.
    /// Represents the coefficient alpha (e.g., in 1/m).
    LambertBeerConstant(validated_type!(LinearNumberDensity, AllFinite && AllPositive)),

    /// Wavelength-dependent absorption coefficient (alpha) for the Lambert-Beer law.
    /// Uses a spectrum mapping wavelengths to absorption coefficients (in 1/m).
    LambertBeerSpectrum(Spectrum),

    /// Tabulated internal transmittance from glass catalogs.
    /// Provides transmittances at specific wavelengths for a given reference thickness.
    CatalogTransmittance {
        /// The reference thickness for the given transmittance data.
        reference_thickness: validated_type!(Length, AllFinite && AllPositive),
        /// Tabulated wavelength-transmittance pairs.
        data: Vec<(Length, validated_type!(f64, AllFinite && AllPositive))>,
    },

    /// Constant extinction coefficient `k` (imaginary part of complex refractive index n + i*k).
    /// The absorption coefficient is calculated via alpha(lambda) = 4 * PI * k / lambda.
    ExtinctionCoefficient(f64),
}

impl Default for AbsorptionModel {
    fn default() -> Self {
        Self::None
    }
}

impl AbsorptionModel {
    /// Creates a constant attenuation model with validation (factor must be positive and finite).
    ///
    /// # Errors
    /// Returns an error if `factor` is non-positive or non-finite.
    pub fn new_constant_attenuation(factor: f64) -> OpmResult<Self> {
        let validated_factor = validated!(factor, AllFinite && AllPositive)?;
        Ok(Self::ConstantAttenuation(validated_factor))
    }

    /// Creates a constant Lambert-Beer absorption model with validation.
    ///
    /// # Errors
    /// Returns an error if `alpha` is non-positive or non-finite.
    pub fn new_lambert_beer_constant(alpha: LinearNumberDensity) -> OpmResult<Self> {
        let validated_alpha = validated!(alpha, AllFinite && AllPositive)?;
        Ok(Self::LambertBeerConstant(validated_alpha))
    }

    /// Creates a catalog internal transmittance model with validation.
    ///
    /// # Errors
    /// Returns an error if `reference_thickness` or any transmittance value fails validation.
    pub fn new_catalog_transmittance(
        reference_thickness: Length,
        raw_data: Vec<(Length, f64)>,
    ) -> OpmResult<Self> {
        let validated_thickness = validated!(reference_thickness, AllFinite && AllPositive)?;
        let mut validated_data = Vec::with_capacity(raw_data.len());

        for (lambda, t_val) in raw_data {
            let val = validated!(t_val, AllFinite && AllPositive)?;
            validated_data.push((lambda, val));
        }

        Ok(Self::CatalogTransmittance {
            reference_thickness: validated_thickness,
            data: validated_data,
        })
    }

    /// Calculates the effective absorption coefficient alpha for a given wavelength.
    ///
    /// # Errors
    /// Returns an error if the wavelength is non-positive or out of bounds for spectral lookups.
    pub fn absorption_coefficient(&self, wavelength: Length) -> OpmResult<LinearNumberDensity> {
        if wavelength.value <= 0.0 {
            return Err(OpossumError::Other(
                "Wavelength must be strictly positive.".into(),
            ));
        }

        match self {
            Self::None | Self::ConstantAttenuation(_) => {
                Ok(LinearNumberDensity::new::<per_meter>(0.0))
            }
            Self::LambertBeerConstant(alpha) => Ok(*alpha.get()),
            Self::LambertBeerSpectrum(spectrum) => {
                let alpha_val = spectrum.get_value(&wavelength).ok_or_else(|| {
                    OpossumError::Spectrum(format!(
                        "Wavelength {} nm is outside absorption spectrum range.",
                        wavelength.get::<uom::si::length::nanometer>()
                    ))
                })?;
                Ok(LinearNumberDensity::new::<per_meter>(alpha_val))
            }
            Self::CatalogTransmittance {
                reference_thickness,
                data,
            } => {
                let tau_i = interpolate_catalog_data(data, wavelength)?;
                let d_ref_m = reference_thickness.get().get::<meter>();

                if tau_i <= 0.0 {
                    Ok(LinearNumberDensity::new::<per_meter>(f64::INFINITY))
                } else {
                    let alpha_m = -tau_i.ln() / d_ref_m;
                    Ok(LinearNumberDensity::new::<per_meter>(alpha_m))
                }
            }
            Self::ExtinctionCoefficient(k) => {
                let lambda_m = wavelength.get::<meter>();
                let alpha_m = (4.0 * PI * k) / lambda_m;
                Ok(LinearNumberDensity::new::<per_meter>(alpha_m))
            }
        }
    }

    /// Calculates the transmission factor T in range [0.0, 1.0] for a given propagation path length.
    ///
    /// # Errors
    /// Returns an error if `path_length` is negative or spectral lookup fails.
    pub fn transmittance(&self, wavelength: Length, path_length: Length) -> OpmResult<f64> {
        if path_length.value < 0.0 {
            return Err(OpossumError::Other(
                "Propagation path length cannot be negative.".into(),
            ));
        }

        match self {
            Self::None => Ok(1.0),
            Self::ConstantAttenuation(factor) => Ok(factor.get().clamp(0.0, 1.0)),
            Self::LambertBeerConstant(_)
            | Self::LambertBeerSpectrum(_)
            | Self::ExtinctionCoefficient(_) => {
                let alpha = self.absorption_coefficient(wavelength)?;
                let alpha_m = alpha.get::<per_meter>();
                let d_m = path_length.get::<meter>();
                Ok((-alpha_m * d_m).exp())
            }
            Self::CatalogTransmittance {
                reference_thickness,
                data,
            } => {
                let tau_i = interpolate_catalog_data(data, wavelength)?;
                let d_ref_m = reference_thickness.get().get::<meter>();
                let d_m = path_length.get::<meter>();
                let exponent = d_m / d_ref_m;
                Ok(tau_i.clamp(0.0, 1.0).powf(exponent))
            }
        }
    }
}

/// Helper function to linearly interpolate tabulated catalog transmittance data.
fn interpolate_catalog_data(
    data: &[(Length, validated_type!(f64, AllFinite && AllPositive))],
    target_wavelength: Length,
) -> OpmResult<f64> {
    if data.is_empty() {
        return Err(OpossumError::Other(
            "Catalog transmittance data table is empty.".into(),
        ));
    }
    if data.len() == 1 {
        return Ok(*data[0].1.get());
    }

    let target_nm = target_wavelength.get::<uom::si::length::nanometer>();
    let first_nm = data[0].0.get::<uom::si::length::nanometer>();
    let last_nm = data.last().unwrap().0.get::<uom::si::length::nanometer>();

    if target_nm < first_nm || target_nm > last_nm {
        return Err(OpossumError::Spectrum(format!(
            "Wavelength {:.2} nm is outside catalog table range [{:.2}, {:.2}] nm.",
            target_nm, first_nm, last_nm
        )));
    }

    // Find the bounding interval for linear interpolation
    for window in data.windows(2) {
        let (lambda_0, val_0) = (
            window[0].0.get::<uom::si::length::nanometer>(),
            *window[0].1.get(),
        );
        let (lambda_1, val_1) = (
            window[1].0.get::<uom::si::length::nanometer>(),
            *window[1].1.get(),
        );

        if (lambda_0..=lambda_1).contains(&target_nm) {
            let ratio = (target_nm - lambda_0) / (lambda_1 - lambda_0);
            return Ok(val_0 + ratio * (val_1 - val_0));
        }
    }

    Ok(*data.last().unwrap().1.get())
}

// --- From Implementations for Infallible Conversions ---

impl From<Spectrum> for AbsorptionModel {
    fn from(spectrum: Spectrum) -> Self {
        Self::LambertBeerSpectrum(spectrum)
    }
}

impl From<&Spectrum> for AbsorptionModel {
    fn from(spectrum: &Spectrum) -> Self {
        Self::LambertBeerSpectrum(spectrum.clone())
    }
}

impl From<validated_type!(LinearNumberDensity, AllFinite && AllPositive)> for AbsorptionModel {
    fn from(alpha: validated_type!(LinearNumberDensity, AllFinite && AllPositive)) -> Self {
        Self::LambertBeerConstant(alpha)
    }
}

impl From<validated_type!(f64, AllFinite && AllPositive)> for AbsorptionModel {
    fn from(factor: validated_type!(f64, AllFinite && AllPositive)) -> Self {
        Self::ConstantAttenuation(factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::{assert_relative_eq};
    use std::f64::consts::PI;
    use uom::si::{
        length::{millimeter, nanometer},
        linear_number_density::per_meter,
    };

    use crate::micrometer;

    #[test]
    fn test_default_is_none() {
        let default_model = AbsorptionModel::default();
        assert_eq!(default_model, AbsorptionModel::None);
    }

    #[test]
    fn test_none_model() -> OpmResult<()> {
        let model = AbsorptionModel::None;
        let wvl = Length::new::<nanometer>(1054.0);
        let path = Length::new::<millimeter>(100.0);

        // Perfectly transparent: alpha = 0, T = 1.0
        let alpha = model.absorption_coefficient(wvl)?;
        assert_relative_eq!(alpha.get::<per_meter>(), 0.0);

        let transmittance = model.transmittance(wvl, path)?;
        assert_relative_eq!(transmittance, 1.0);
        Ok(())
    }

    #[test]
    fn test_constant_attenuation() -> OpmResult<()> {
        let model = AbsorptionModel::new_constant_attenuation(0.75)?;
        let wvl = Length::new::<nanometer>(532.0);
        let path = Length::new::<millimeter>(50.0);

        // Path-length independent attenuation
        let alpha = model.absorption_coefficient(wvl)?;
        assert_relative_eq!(alpha.get::<per_meter>(), 0.0);

        let transmittance = model.transmittance(wvl, path)?;
        assert_relative_eq!(transmittance, 0.75);

        // Validation failure on strictly negative factor
        assert!(AbsorptionModel::new_constant_attenuation(-0.5).is_err());
        assert!(AbsorptionModel::new_constant_attenuation(f64::NAN).is_err());
        Ok(())
    }

    #[test]
    fn test_lambert_beer_constant() -> OpmResult<()> {
        let alpha_input = LinearNumberDensity::new::<per_meter>(100.0);
        let model = AbsorptionModel::new_lambert_beer_constant(alpha_input)?;

        let wvl = Length::new::<nanometer>(1064.0);
        let path = Length::new::<millimeter>(10.0); // 0.01 m

        let alpha = model.absorption_coefficient(wvl)?;
        assert_relative_eq!(alpha.get::<per_meter>(), 100.0);

        // T = exp(-100 m^-1 * 0.01 m) = exp(-1.0)
        let transmittance = model.transmittance(wvl, path)?;
        assert_relative_eq!(transmittance, (-1.0_f64).exp(), epsilon = 1e-12);

        // Validation failure on strictly negative alpha
        let negative_alpha = LinearNumberDensity::new::<per_meter>(-10.0);
        assert!(AbsorptionModel::new_lambert_beer_constant(negative_alpha).is_err());
        Ok(())
    }

    #[test]
    fn test_lambert_beer_spectrum() -> OpmResult<()> {
        let mut spectrum =
            Spectrum::new(micrometer!(0.5)..micrometer!(1.5), micrometer!(0.5))?;
        // Set absorption coefficients (in 1/m): 500 nm -> 10 m^-1, 1000 nm -> 50 m^-1, 1500 nm -> 100 m^-1
        spectrum.set_data(vec![(0.5, 10.0), (1.0, 50.0), (1.5, 100.0)])?;

        let model = AbsorptionModel::from(spectrum);

        // Exact match (with epsilon tolerance for floating-point interpolation)
        let wvl_1000 = Length::new::<nanometer>(1000.0);
        let path = Length::new::<millimeter>(20.0); // 0.02 m
        let alpha = model.absorption_coefficient(wvl_1000)?;
        assert_relative_eq!(alpha.get::<per_meter>(), 50.0, epsilon = 1e-9);

        // T = exp(-50 * 0.02) = exp(-1.0)
        let transmittance = model.transmittance(wvl_1000, path)?;
        assert_relative_eq!(transmittance, (-1.0_f64).exp(), epsilon = 1e-9);

        // Interpolated match: 750 nm -> alpha = 30 m^-1
        let wvl_750 = Length::new::<nanometer>(750.0);
        let alpha_interp = model.absorption_coefficient(wvl_750)?;
        assert_relative_eq!(alpha_interp.get::<per_meter>(), 30.0, epsilon = 1e-9);

        // Out-of-bounds wavelength should error
        let wvl_out = Length::new::<nanometer>(2000.0);
        assert!(model.absorption_coefficient(wvl_out).is_err());
        assert!(model.transmittance(wvl_out, path).is_err());
        Ok(())
    }

    #[test]
    fn test_catalog_transmittance_interpolation_and_scaling() -> OpmResult<()> {
        let d_ref = Length::new::<millimeter>(10.0);
        let raw_data = vec![
            (Length::new::<nanometer>(500.0), 0.90),
            (Length::new::<nanometer>(1000.0), 0.80),
            (Length::new::<nanometer>(1500.0), 0.50),
        ];

        let model = AbsorptionModel::new_catalog_transmittance(d_ref, raw_data)?;

        // Test reference thickness transmittance: T(10 mm) at 1000 nm should be exactly 0.80
        let wvl_1000 = Length::new::<nanometer>(1000.0);
        let t_10mm = model.transmittance(wvl_1000, Length::new::<millimeter>(10.0))?;
        assert_relative_eq!(t_10mm, 0.80, epsilon = 1e-12);

        // Double thickness: T(20 mm) = 0.80^2 = 0.64
        let t_20mm = model.transmittance(wvl_1000, Length::new::<millimeter>(20.0))?;
        assert_relative_eq!(t_20mm, 0.64, epsilon = 1e-12);

        // Half thickness: T(5 mm) = 0.80^0.5
        let t_5mm = model.transmittance(wvl_1000, Length::new::<millimeter>(5.0))?;
        assert_relative_eq!(t_5mm, 0.80_f64.sqrt(), epsilon = 1e-12);

        // Interpolation test: 750 nm -> tau_i = 0.85
        let wvl_750 = Length::new::<nanometer>(750.0);
        let t_interp = model.transmittance(wvl_750, Length::new::<millimeter>(10.0))?;
        assert_relative_eq!(t_interp, 0.85, epsilon = 1e-12);

        // Absorption coefficient: alpha = -ln(0.80) / 0.01 m
        let alpha = model.absorption_coefficient(wvl_1000)?;
        let expected_alpha = -0.80_f64.ln() / 0.01;
        assert_relative_eq!(alpha.get::<per_meter>(), expected_alpha, epsilon = 1e-10);
        Ok(())
    }

    #[test]
    fn test_extinction_coefficient() -> OpmResult<()> {
        let k = 1.0e-4;
        let model = AbsorptionModel::ExtinctionCoefficient(k);

        let wvl = Length::new::<nanometer>(1000.0); // 1.0e-6 m
        let path = Length::new::<millimeter>(1.0); // 1.0e-3 m

        // alpha = 4 * PI * k / lambda = 4 * PI * 1e-4 / 1e-6 = 400 * PI ~ 1256.637 m^-1
        let alpha = model.absorption_coefficient(wvl)?;
        let expected_alpha = 400.0 * PI;
        assert_relative_eq!(alpha.get::<per_meter>(), expected_alpha, epsilon = 1e-10);

        // T = exp(-alpha * d)
        let transmittance = model.transmittance(wvl, path)?;
        assert_relative_eq!(
            transmittance,
            (-expected_alpha * 1.0e-3).exp(),
            epsilon = 1e-10
        );
        Ok(())
    }

    #[test]
    fn test_invalid_arguments_guardrails() -> OpmResult<()> {
        let model = AbsorptionModel::new_constant_attenuation(0.9)?;

        // Negative path length must return an error
        let negative_path = Length::new::<millimeter>(-1.0);
        let wvl = Length::new::<nanometer>(500.0);
        assert!(model.transmittance(wvl, negative_path).is_err());

        // Zero or negative wavelength must return an error
        let zero_wvl = Length::new::<nanometer>(0.0);
        let neg_wvl = Length::new::<nanometer>(-500.0);
        assert!(model.absorption_coefficient(zero_wvl).is_err());
        assert!(model.absorption_coefficient(neg_wvl).is_err());
        Ok(())
    }

    #[test]
    fn test_from_trait_conversions() -> OpmResult<()> {
        // Spectrum conversion
        let spec = Spectrum::default();
        let from_owned: AbsorptionModel = spec.clone().into();
        let from_borrowed: AbsorptionModel = (&spec).into();
        assert_eq!(from_owned, AbsorptionModel::LambertBeerSpectrum(spec.clone()));
        assert_eq!(from_borrowed, AbsorptionModel::LambertBeerSpectrum(spec));

        // Validated LinearNumberDensity conversion
        let val_density = validated!(LinearNumberDensity::new::<per_meter>(15.0), AllFinite && AllPositive)?;
        let from_density: AbsorptionModel = val_density.into();
        assert_eq!(from_density, AbsorptionModel::LambertBeerConstant(val_density));

        // Validated f64 conversion
        let val_factor = validated!(0.85_f64, AllFinite && AllPositive)?;
        let from_factor: AbsorptionModel = val_factor.into();
        assert_eq!(from_factor, AbsorptionModel::ConstantAttenuation(val_factor));
        Ok(())
    }
}