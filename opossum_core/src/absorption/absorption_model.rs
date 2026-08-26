//! Module for optical absorption models in `opossum_core`.

use crate::{
    absorption::{
        absorption_catalog_transmittance::AbsCatTrans, absorption_constant::AbsConst,
        absorption_lb_constant::AbsLBConst,
    },
    error::{OpmResult, OpossumError},
    light::Spectrum,
    utils::default_from_name::DefaultFromName,
};
use serde::{Deserialize, Serialize};
use std::{f64::consts::PI, fmt::Display};
use strum::EnumIter;
use uom::si::{
    f64::{Length, LinearNumberDensity},
    length::meter,
    linear_number_density::per_meter,
};

/// Defines the absorption model for an optical material.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
pub enum AbsorptionModel {
    /// No absorption (perfectly transparent material).
    #[default]
    None,

    /// Wavelength- and path-length-independent attenuation factor [0.0, 1.0].
    /// Applies a flat intensity reduction upon surface interaction or transit.
    ConstantAttenuation(AbsConst),

    /// Wavelength-independent absorption coefficient for the Lambert-Beer law.
    LambertBeerConstant(AbsLBConst),

    /// Wavelength-dependent absorption coefficient (alpha) for the Lambert-Beer law.
    /// Uses a spectrum mapping wavelengths to absorption coefficients (in 1/m).
    LambertBeerSpectrum(Spectrum),

    /// Tabulated internal transmittance from glass catalogs.
    /// Provides transmittances at specific wavelengths for a given reference thickness.
    CatalogTransmittance(AbsCatTrans),

    /// Constant extinction coefficient `k` (imaginary part of complex refractive index n + i*k).
    /// The absorption coefficient is calculated via alpha(lambda) = 4 * PI * k / lambda.
    ExtinctionCoefficient(f64),
}

impl AbsorptionModel {
    /// Creates a constant attenuation model with validation (factor must be positive and finite).
    ///
    /// # Errors
    /// Returns an error if `factor` is non-positive or non-finite.
    pub fn new_constant_attenuation(factor: f64) -> OpmResult<Self> {
        let abs_const = AbsConst::new(factor)?;
        Ok(Self::ConstantAttenuation(abs_const))
    }

    /// Creates a constant Lambert-Beer absorption model with validation.
    ///
    /// # Errors
    /// Returns an error if `alpha` is non-positive or non-finite.
    pub fn new_lambert_beer_constant(alpha: LinearNumberDensity) -> OpmResult<Self> {
        let lbc = AbsLBConst::new(alpha)?;
        Ok(Self::LambertBeerConstant(lbc))
    }

    /// Creates a catalog internal transmittance model with validation.
    ///
    /// # Errors
    /// Returns an error if `reference_thickness` or any transmittance value fails validation.
    pub fn new_catalog_transmittance(
        reference_thickness: Length,
        raw_data: Vec<(Length, f64)>,
    ) -> OpmResult<Self> {
        let act = AbsCatTrans::new(reference_thickness, raw_data)?;

        Ok(Self::CatalogTransmittance(act))
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
            Self::LambertBeerConstant(alpha) => Ok(alpha.alpha()),
            Self::LambertBeerSpectrum(spectrum) => {
                let alpha_val = spectrum.get_value(&wavelength).ok_or_else(|| {
                    OpossumError::Spectrum(format!(
                        "Wavelength {} nm is outside absorption spectrum range.",
                        wavelength.get::<uom::si::length::nanometer>()
                    ))
                })?;
                Ok(LinearNumberDensity::new::<per_meter>(alpha_val))
            }
            Self::CatalogTransmittance(abs_cat_trans) => {
                let tau_i = interpolate_catalog_data(abs_cat_trans.data(), wavelength)?;
                let d_ref_m = abs_cat_trans.reference_thickness().get::<meter>();

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
            Self::ConstantAttenuation(abs_const) => Ok(abs_const.absorption_constant()),
            Self::LambertBeerConstant(_)
            | Self::LambertBeerSpectrum(_)
            | Self::ExtinctionCoefficient(_) => {
                let alpha = self.absorption_coefficient(wavelength)?;
                let alpha_m = alpha.get::<per_meter>();
                let d_m = path_length.get::<meter>();
                Ok((-alpha_m * d_m).exp())
            }
            Self::CatalogTransmittance(abs_cat_trans) => {
                let tau_i = interpolate_catalog_data(abs_cat_trans.data(), wavelength)?;
                let d_ref_m = abs_cat_trans.reference_thickness().get::<meter>();
                let d_m = path_length.get::<meter>();
                let exponent = d_m / d_ref_m;
                Ok(tau_i.clamp(0.0, 1.0).powf(exponent))
            }
        }
    }
}

impl Display for AbsorptionModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::ConstantAttenuation(_) => {
                write!(f, "Constant Attenuation")
            }
            Self::LambertBeerConstant(_) => {
                write!(f, "Lambert-Beer (Constant)")
            }
            Self::LambertBeerSpectrum(_) => {
                write!(f, "Lambert-Beer (Spectrum)")
            }
            Self::CatalogTransmittance(_) => {
                write!(f, "Catalog Transmittance")
            }
            Self::ExtinctionCoefficient(_) => {
                write!(f, "Extinction Coefficient")
            }
        }
    }
}
/// Helper function to linearly interpolate tabulated catalog transmittance data.
fn interpolate_catalog_data(
    data: &Vec<(Length, f64)>,
    target_wavelength: Length,
) -> OpmResult<f64> {
    if data.is_empty() {
        return Err(OpossumError::Other(
            "Catalog transmittance data table is empty.".into(),
        ));
    }
    if data.len() == 1 {
        return Ok(data[0].1);
    }

    // let target_nm = target_wavelength;
    let first_wvl = data[0].0;
    let last_wvl = data.last().unwrap().0;

    if target_wavelength < first_wvl || target_wavelength > last_wvl {
        return Err(OpossumError::Spectrum(format!(
            "Wavelength {target_wavelength:?} nm is outside catalog table range [{first_wvl:?}, {last_wvl:?}] nm."
        )));
    }
    // Find the bounding interval for linear interpolation
    for window in data.windows(2) {
        let (lambda_0, val_0) = (window[0].0, window[0].1);
        let (lambda_1, val_1) = (window[1].0, window[1].1);

        if (lambda_0..=lambda_1).contains(&target_wavelength) {
            let ratio = (target_wavelength - lambda_0) / (lambda_1 - lambda_0);
            return Ok(ratio.value.mul_add(val_1 - val_0, val_0));
        }
    }
    Ok(data.last().unwrap().1)
}

impl DefaultFromName for AbsorptionModel {
    fn default_from_name(name: &str) -> Option<Self> {
        match name {
            "None" => Some(Self::None),
            "ConstantAttenuation" | "Constant Attenuation" => {
                Some(Self::ConstantAttenuation(AbsConst::default()))
            }
            "LambertBeerConstant" | "Lambert-Beer (Constant)" => {
                Some(Self::LambertBeerConstant(AbsLBConst::default()))
            }
            "LambertBeerSpectrum" | "Lambert-Beer (Spectrum)" => {
                Some(Self::LambertBeerSpectrum(Spectrum::default()))
            }
            "ExtinctionCoefficient" | "Extinction Coefficient (k)" => {
                Some(Self::ExtinctionCoefficient(0.0))
            }
            "CatalogTransmittance" | "Catalog Transmittance" => {
                Some(Self::CatalogTransmittance(AbsCatTrans::default()))
            }
            _ => None,
        }
    }
}

impl From<AbsConst> for AbsorptionModel {
    fn from(abs_const: AbsConst) -> Self {
        Self::ConstantAttenuation(abs_const)
    }
}

impl From<&AbsConst> for AbsorptionModel {
    fn from(abs_const: &AbsConst) -> Self {
        Self::ConstantAttenuation(*abs_const)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f64::consts::PI;
    use uom::si::linear_number_density::per_meter;

    use crate::{micrometer, millimeter, nanometer};

    #[test]
    fn test_default_is_none() {
        let default_model = AbsorptionModel::default();
        assert_eq!(default_model, AbsorptionModel::None);
    }

    #[test]
    fn test_none_model() -> OpmResult<()> {
        let model = AbsorptionModel::None;
        let wvl = nanometer!(1054.0);
        let path = millimeter!(100.0);

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
        let wvl = nanometer!(532.0);
        let path = millimeter!(50.0);

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

        let wvl = nanometer!(1064.0);
        let path = millimeter!(10.0);

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
        let mut spectrum = Spectrum::new(micrometer!(0.5)..micrometer!(1.5), micrometer!(0.5))?;
        // Set absorption coefficients (in 1/m): 500 nm -> 10 m^-1, 1000 nm -> 50 m^-1, 1500 nm -> 100 m^-1
        spectrum.set_data(vec![(0.5, 10.0), (1.0, 50.0), (1.5, 100.0)])?;

        let model = AbsorptionModel::from(spectrum);

        // Exact match (with epsilon tolerance for floating-point interpolation)
        let wvl_1000 = nanometer!(1000.0);
        let path = millimeter!(20.0); // 0.02 m
        let alpha = model.absorption_coefficient(wvl_1000)?;
        assert_relative_eq!(alpha.get::<per_meter>(), 50.0, epsilon = 1e-9);

        // T = exp(-50 * 0.02) = exp(-1.0)
        let transmittance = model.transmittance(wvl_1000, path)?;
        assert_relative_eq!(transmittance, (-1.0_f64).exp(), epsilon = 1e-9);

        // Interpolated match: 750 nm -> alpha = 30 m^-1
        let wvl_750 = nanometer!(750.0);
        let alpha_interp = model.absorption_coefficient(wvl_750)?;
        assert_relative_eq!(alpha_interp.get::<per_meter>(), 30.0, epsilon = 1e-9);

        // Out-of-bounds wavelength should error
        let wvl_out = nanometer!(2000.0);
        assert!(model.absorption_coefficient(wvl_out).is_err());
        assert!(model.transmittance(wvl_out, path).is_err());
        Ok(())
    }

    #[test]
    fn test_catalog_transmittance_interpolation_and_scaling() -> OpmResult<()> {
        let d_ref = millimeter!(10.0);
        let raw_data = vec![
            (nanometer!(500.0), 0.90),
            (nanometer!(1000.0), 0.80),
            (nanometer!(1500.0), 0.50),
        ];

        let model = AbsorptionModel::new_catalog_transmittance(d_ref, raw_data)?;

        // Test reference thickness transmittance: T(10 mm) at 1000 nm should be exactly 0.80
        let wvl_1000 = nanometer!(1000.0);
        let t_10mm = model.transmittance(wvl_1000, millimeter!(10.0))?;
        assert_relative_eq!(t_10mm, 0.80, epsilon = 1e-12);

        // Double thickness: T(20 mm) = 0.80^2 = 0.64
        let t_20mm = model.transmittance(wvl_1000, millimeter!(20.0))?;
        assert_relative_eq!(t_20mm, 0.64, epsilon = 1e-12);

        // Half thickness: T(5 mm) = 0.80^0.5
        let t_5mm = model.transmittance(wvl_1000, millimeter!(5.0))?;
        assert_relative_eq!(t_5mm, 0.80_f64.sqrt(), epsilon = 1e-12);

        // Interpolation test: 750 nm -> tau_i = 0.85
        let wvl_750 = nanometer!(750.0);
        let t_interp = model.transmittance(wvl_750, millimeter!(10.0))?;
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

        let wvl = nanometer!(1000.0);
        let path = millimeter!(1.0);

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
        let negative_path = millimeter!(-1.0);
        let wvl = nanometer!(500.0);
        assert!(model.transmittance(wvl, negative_path).is_err());

        // Zero or negative wavelength must return an error
        let zero_wvl = nanometer!(0.0);
        let neg_wvl = nanometer!(-500.0);
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
        assert_eq!(
            from_owned,
            AbsorptionModel::LambertBeerSpectrum(spec.clone())
        );
        assert_eq!(from_borrowed, AbsorptionModel::LambertBeerSpectrum(spec));
        Ok(())
    }
}
