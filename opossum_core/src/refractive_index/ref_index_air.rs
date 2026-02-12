//! Refractive index model for air.
//!
//! This model uses the Edlén formula modified by Birch and Downs (1993, 1994).
//!
//! Reference: <https://emtoolbox.nist.gov/Wavelength/Documentation.asp>

use super::{RefractiveIndex, RefractiveIndexType};
use crate::{
    degree_celsius,
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllInRange},
    hectopascal, validated, validated_type,
};
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Length, Pressure, ThermodynamicTemperature},
    pressure::pascal,
    thermodynamic_temperature::{degree_celsius, kelvin},
};

// --- Constants for Saturation Vapor Pressure (Birch and Downs) ---
const K_1: f64 = 1.167_052_145_28E+03;
const K_2: f64 = -7.242_131_670_32E+05;
const K_3: f64 = -1.707_384_694_01E+01;
const K_4: f64 = 1.202_082_470_25E+04;
const K_5: f64 = -3.232_555_032_23E+06;
const K_6: f64 = 1.491_510_861_35E+01;
const K_7: f64 = -4.823_265_736_16E+03;
const K_8: f64 = 4.051_134_054_21E+05;
const K_9: f64 = -2.385_555_756_78E-01;
const K_10: f64 = 6.501_753_484_48E+02;

// --- Constants for the Edlén Formula ---
const A: f64 = 8342.54;
const B: f64 = 2_406_147.0;
const C: f64 = 15998.0;
const D: f64 = 96095.43;
const E: f64 = 0.601;
const F: f64 = 0.00972;
const G: f64 = 0.003_661;

const COEFF_B_DENOM: f64 = 130.0;
const COEFF_C_DENOM: f64 = 38.9;

const WATER_VAPOR_CONST_A: f64 = 292.75; // K
const WATER_VAPOR_CONST_B: f64 = 3.7345;
const WATER_VAPOR_CONST_C: f64 = 0.0401; // um^2

// --- Validation Ranges & Limits ---
const TEMP_MIN: f64 = -40.0;
const TEMP_MAX: f64 = 100.0;
const PRESS_MIN: f64 = 100.0;
const PRESS_MAX: f64 = 1400.0;
const HUMIDITY_MIN: f64 = 0.0;
const HUMIDITY_MAX: f64 = 100.0;

// Valid wavelength range in nanometers
const WVL_MIN_NM: f64 = 300.0;
const WVL_MAX_NM: f64 = 1700.0;

/// Calculates the saturation vapor pressure of water in air.
#[allow(clippy::many_single_char_names)]
fn saturation_vapor_pressure(temperature: ThermodynamicTemperature) -> Pressure {
    let t = temperature.get::<kelvin>();

    let omega = t + K_9 / (t - K_10);
    let omega2 = omega * omega;

    let a = K_1.mul_add(omega, omega2) + K_2;
    let b = K_3.mul_add(omega2, K_4 * omega) + K_5;
    let c = K_6.mul_add(omega2, K_7 * omega) + K_8;

    let x = -b + f64::sqrt(b.mul_add(b, -(4.0 * a * c)));
    let p = 1E6 * (2.0 * c / x).powi(4);

    Pressure::new::<pascal>(p)
}

#[inline]
fn partial_vapor_pressure(temperature: ThermodynamicTemperature, humidity: f64) -> Pressure {
    (humidity / 100.0) * saturation_vapor_pressure(temperature)
}

#[derive(Deserialize)]
struct NonValidatedRefrIndexAir {
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    humidity: f64,
}

type ValidatedRelativeHumidity = validated_type!(f64, AllFinite && AllInRange::<f64>);
type ValidatedPressure = validated_type!(Pressure, AllFinite && AllInRange::<Pressure>);
type ValidatedTemperature = validated_type!(
    ThermodynamicTemperature,
    AllFinite && AllInRange::<ThermodynamicTemperature>
);

/// Refractive index model for air using the Edlén formula.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct RefrIndexAir {
    temperature: ValidatedTemperature,
    pressure: ValidatedPressure,
    humidity: ValidatedRelativeHumidity,

    // --- Cache Fields (Excluded from serialization) ---
    // These are calculated based on T, P, H to speed up get_refractive_index
    #[serde(skip)]
    dry_air_factor: f64,
    #[serde(skip)]
    water_vapor_factor: f64,
}

impl Default for RefrIndexAir {
    /// Creates a default instance representing "Standard Air".
    ///
    /// * Temperature: 20 °C
    /// * Pressure: 1013.25 hPa
    /// * Humidity: 50%
    fn default() -> Self {
        let mut instance = Self {
            temperature: validated!(
                degree_celsius!(20.0),
                AllFinite
                    && AllInRange::new(degree_celsius!(TEMP_MIN), degree_celsius!(TEMP_MAX), true)
                        .unwrap()
            )
            .unwrap(),
            pressure: validated!(
                hectopascal!(1013.25),
                AllFinite
                    && AllInRange::new(hectopascal!(PRESS_MIN), hectopascal!(PRESS_MAX), true)
                        .unwrap()
            )
            .unwrap(),
            humidity: validated!(
                50.0,
                AllFinite && AllInRange::new(HUMIDITY_MIN, HUMIDITY_MAX, true).unwrap()
            )
            .unwrap(),
            // Initialized with dummy values, updated immediately below
            dry_air_factor: 0.0,
            water_vapor_factor: 0.0,
        };
        instance.update_cache();
        instance
    }
}

impl RefrIndexAir {
    /// Creates a new [`RefrIndexAir`] instance with validated inputs.
    ///
    /// # Arguments
    /// * `temperature` - Temperature (valid: -40°C to 100°C).
    /// * `pressure` - Pressure (valid: 100 hPa to 1400 hPa).
    /// * `humidity` - Relative humidity (valid: 0% to 100%).
    ///
    /// # Errors
    /// Returns [`OpossumError`] if any input is outside its valid range.
    pub fn new(
        temperature: ThermodynamicTemperature,
        pressure: Pressure,
        humidity: f64,
    ) -> OpmResult<Self> {
        let mut n_air = Self::default();
        n_air.set_temperature(temperature)?;
        n_air.set_pressure(pressure)?;
        n_air.set_humidity(humidity)?;
        Ok(n_air)
    }

    /// Updates the cached factors derived from Temperature, Pressure, and Humidity.
    /// This removes heavy floating point operations from the wavelength-dependent hot path.
    fn update_cache(&mut self) {
        let t_celsius = self.temperature().get::<degree_celsius>();
        let p_pascal = self.pressure().get::<pascal>();

        // 1. Calculate TP correction factor (Eq. 4 in NIST docs)
        // x = (1 + 10^-8 * (0.601 - 0.00972 * t) * p) / (1 + 0.003661 * t)
        let x_numerator = (1.0E-8 * F.mul_add(-t_celsius, E)).mul_add(p_pascal, 1.0);
        let x_denominator = G.mul_add(t_celsius, 1.0);
        let x = x_numerator / x_denominator;

        // The formula for n_tp is: n_tp = 1 + p * (n_s - 1) * x / D
        // We factor out (n_s - 1). The term we need to cache is (p * x) / D.
        self.dry_air_factor = p_pascal * x / D;

        // 2. Calculate Water Vapor factor
        // The correction term is: - 10^-10 * (292.75 / T_kelvin) * p_v * (3.7345 - 0.0401 * s)
        // We cache everything except the sigma-dependent part.
        let p_v_pascal =
            partial_vapor_pressure(self.temperature(), self.humidity()).get::<pascal>();
        let t_kelvin = self.temperature().get::<kelvin>();

        self.water_vapor_factor = 1.0E-10 * (WATER_VAPOR_CONST_A / t_kelvin) * p_v_pascal;
    }

    /// Sets the temperature and validates the input range.
    ///
    /// # Errors
    /// Returns `OpossumError` if temperature is outside the range [-40°C, 100°C].
    #[allow(clippy::missing_panics_doc)]
    pub fn set_temperature(&mut self, temperature: ThermodynamicTemperature) -> OpmResult<()> {
        self.temperature = validated!(
            temperature,
            AllFinite
                && AllInRange::new(degree_celsius!(TEMP_MIN), degree_celsius!(TEMP_MAX), true)
                    .unwrap()
        )?;
        self.update_cache();
        Ok(())
    }

    /// Returns the current temperature.
    #[must_use]
    pub const fn temperature(&self) -> ThermodynamicTemperature {
        *self.temperature.get()
    }

    /// Sets the pressure and validates the input range.
    ///
    /// # Errors
    /// Returns `OpossumError` if pressure is outside the range [100 hPa, 1400 hPa].
    #[allow(clippy::missing_panics_doc)]
    pub fn set_pressure(&mut self, pressure: Pressure) -> OpmResult<()> {
        self.pressure = validated!(
            pressure,
            AllFinite
                && AllInRange::new(hectopascal!(PRESS_MIN), hectopascal!(PRESS_MAX), true).unwrap()
        )?;
        self.update_cache();
        Ok(())
    }

    /// Returns the current pressure.
    #[must_use]
    pub const fn pressure(&self) -> Pressure {
        *self.pressure.get()
    }

    /// Sets the relative humidity (0.0 to 100.0).
    ///
    /// # Errors
    /// Returns `OpossumError` if humidity is outside the range [0.0, 100.0].
    #[allow(clippy::missing_panics_doc)]
    pub fn set_humidity(&mut self, humidity: f64) -> OpmResult<()> {
        self.humidity = validated!(
            humidity,
            AllFinite && AllInRange::new(HUMIDITY_MIN, HUMIDITY_MAX, true).unwrap()
        )?;
        self.update_cache();
        Ok(())
    }

    /// Returns the current relative humidity.
    #[must_use]
    pub const fn humidity(&self) -> f64 {
        *self.humidity.get()
    }
}

impl<'de> serde::Deserialize<'de> for RefrIndexAir {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = NonValidatedRefrIndexAir::deserialize(deserializer)?;
        Self::new(helper.temperature, helper.pressure, helper.humidity)
            .map_err(serde::de::Error::custom)
    }
}

impl RefractiveIndex for RefrIndexAir {
    #[inline]
    #[allow(clippy::similar_names)]
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        let lambda_nm = wavelength.get::<uom::si::length::nanometer>();

        // Fast range check using constants
        if !(WVL_MIN_NM..=WVL_MAX_NM).contains(&lambda_nm) {
            return Err(OpossumError::Other(format!(
                "Wavelength {lambda_nm:.1}nm outside valid range {WVL_MIN_NM}-{WVL_MAX_NM}nm",
            )));
        }

        // Convert to micrometers for the formula
        let lambda_um = lambda_nm / 1000.0;
        let s = 1.0 / (lambda_um * lambda_um);

        // 1. Calculate Standard Air Refractive Index (n_s)
        // This part depends solely on wavelength
        let n_s = 1.0E-8f64.mul_add(A + B / (COEFF_B_DENOM - s) + C / (COEFF_C_DENOM - s), 1.0);
        // 2. Apply cached Dry Air Factor
        let n_tp = (n_s - 1.0).mul_add(self.dry_air_factor, 1.0);

        // 3. Apply cached Water Vapor Factor
        let correction =
            self.water_vapor_factor * WATER_VAPOR_CONST_C.mul_add(-s, WATER_VAPOR_CONST_B);

        Ok(n_tp - correction)
    }
}

impl From<RefrIndexAir> for RefractiveIndexType {
    fn from(refr: RefrIndexAir) -> Self {
        Self::Air(refr)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nanometer;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_default() {
        let n_air = RefrIndexAir::default();
        // Default: 20°C, 1013.25 hPa, 50%
        assert_eq!(n_air.temperature(), degree_celsius!(20.0));
        assert_eq!(n_air.pressure(), hectopascal!(1013.25));
        assert_eq!(n_air.humidity(), 50.0);

        // Ensure cache is initialized
        assert!(n_air.dry_air_factor > 0.0);
    }

    #[test]
    fn test_new_valid() {
        // Test within bounds
        let n_air = RefrIndexAir::new(degree_celsius!(25.0), hectopascal!(1000.0), 30.0);
        assert!(n_air.is_ok());
        let n_air = n_air.unwrap();
        assert_eq!(n_air.temperature(), degree_celsius!(25.0));
        assert_eq!(n_air.pressure(), hectopascal!(1000.0));
        assert_eq!(n_air.humidity(), 30.0);
    }

    #[test]
    fn test_new_out_of_range() {
        // Temperature Bounds [-40, 100]
        assert!(RefrIndexAir::new(degree_celsius!(-40.1), hectopascal!(1013.0), 50.0).is_err());
        assert!(RefrIndexAir::new(degree_celsius!(100.1), hectopascal!(1013.0), 50.0).is_err());

        // Pressure Bounds [100, 1400]
        assert!(RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(99.9), 50.0).is_err());
        assert!(RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(1400.1), 50.0).is_err());

        // Humidity Bounds [0, 100]
        assert!(RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(1013.0), -0.1).is_err());
        assert!(RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(1013.0), 100.1).is_err());
    }

    #[test]
    fn test_setters_out_of_range() {
        let mut n_air = RefrIndexAir::default();

        // Valid updates
        assert!(n_air.set_temperature(degree_celsius!(-40.0)).is_ok());
        assert!(n_air.set_pressure(hectopascal!(1400.0)).is_ok());
        assert!(n_air.set_humidity(100.0).is_ok());

        // Invalid updates (should not change state)
        assert!(n_air.set_temperature(degree_celsius!(-41.0)).is_err());
        assert_eq!(n_air.temperature(), degree_celsius!(-40.0)); // Old value remains

        assert!(n_air.set_pressure(hectopascal!(1500.0)).is_err());
        assert_eq!(n_air.pressure(), hectopascal!(1400.0));

        assert!(n_air.set_humidity(110.0).is_err());
        assert_eq!(n_air.humidity(), 100.0);
    }

    #[test]
    fn test_cache_consistency() {
        // Ensure that using the cache produces the same result as a fresh calculation would
        // (verified via the regression tests below against known values)
        let n_air = RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(1013.25), 50.0).unwrap();

        // Check internal cache state (sanity check)
        assert!(n_air.dry_air_factor > 0.0);
        assert!(n_air.water_vapor_factor > 0.0);
    }

    #[test]
    fn test_get_refractive_index_cached() {
        // Same test cases as before, ensuring the optimized math holds up
        let n_air = RefrIndexAir::new(degree_celsius!(15.0), hectopascal!(1013.25), 0.0).unwrap();
        assert_abs_diff_eq!(
            n_air.get_refractive_index(nanometer!(633.0)).unwrap(),
            1.000276529,
            epsilon = 1e-9
        );

        let n_air = RefrIndexAir::new(degree_celsius!(20.0), hectopascal!(1013.25), 50.0).unwrap();
        assert_abs_diff_eq!(
            n_air.get_refractive_index(nanometer!(633.0)).unwrap(),
            1.000271374,
            epsilon = 1e-9
        );
    }
}
