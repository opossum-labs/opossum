use std::ops::Range;

use super::{RefractiveIndex, RefractiveIndexType};
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllInRange},
    nanometer, validated, validated_type,
};
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Length, Pressure, ThermodynamicTemperature},
    length::micrometer,
    pressure::{hectopascal, pascal},
    thermodynamic_temperature::{degree_celsius, kelvin},
};

// Constants for calculating saturation vapor pressure
const K_1: f64 = 1.16705214528E+03;
const K_2: f64 = -7.24213167032E+05;
const K_3: f64 = -1.70738469401E+01;
const K_4: f64 = 1.20208247025E+04;
const K_5: f64 = -3.23255503223E+06;
const K_6: f64 = 1.49151086135E+01;
const K_7: f64 = -4.82326573616E+03;
const K_8: f64 = 4.05113405421E+05;
const K_9: f64 = -2.38555575678E-01;
const K_10: f64 = 6.50175348448E+02;

// Constants for the Edlén formula
const A: f64 = 8342.54;
const B: f64 = 2406147.0;
const C: f64 = 15998.0;
const D: f64 = 96095.43;
const E: f64 = 0.601;
const F: f64 = 0.00972;
const G: f64 = 0.003661;

fn saturation_vapor_pressure(temperature: ThermodynamicTemperature) -> Pressure {
    let t = temperature.get::<kelvin>();
    let omega = t + K_9 / (t - K_10);
    let a = omega.powi(2) + K_1 * omega + K_2;
    let b = K_3 * omega.powi(2) + K_4 * omega + K_5;
    let c = K_6 * omega.powi(2) + K_7 * omega + K_8;
    let x = -b + f64::sqrt(b.powi(2) - 4.0 * a * c);
    let p = 1E6 * (2.0_f64 * c / x).powi(4);
    Pressure::new::<pascal>(p)
}

fn partial_vapor_pressure(temperature: ThermodynamicTemperature, humidity: f64) -> Pressure {
    humidity * saturation_vapor_pressure(temperature) / 100.0
}

#[derive(Deserialize)]
struct NonValidatedRelativeHumidity {
    pub humidity: f64,
}
type ValidatedRelativeHumidity = validated_type!(f64, AllFinite && AllInRange::<f64>);

/// Refractive index of air using the Edlén formula modified by
/// Birch and Downs. See https://emtoolbox.nist.gov/Wavelength/Documentation.asp
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RefrIndexAir {
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    humidity: ValidatedRelativeHumidity,
    wvl_range: Range<Length>,
}
impl Default for RefrIndexAir {
    fn default() -> Self {
        Self {
            temperature: ThermodynamicTemperature::new::<degree_celsius>(20.0),
            pressure: Pressure::new::<hectopascal>(1013.25),
            humidity: validated!(
                50.0,
                AllFinite && (AllInRange::new(0.0, 100.0, true).unwrap())
            )
            .unwrap(),
            wvl_range: nanometer!(300.0)..nanometer!(1700.0),
        }
    }
}
impl RefrIndexAir {
    /// Creates a new [`RefrIndexAir`].
    pub fn new(
        temperature: ThermodynamicTemperature,
        pressure: Pressure,
        humidity: f64,
    ) -> OpmResult<Self> {
        let mut n_air = RefrIndexAir::default();
        n_air.set_humidity(humidity)?;
        n_air.set_pressure(pressure);
        n_air.set_temperature(temperature);
        Ok(n_air)
    }
    /// Sets the temperature of this [`RefrIndexAir`].
    pub fn set_temperature(&mut self, temperature: ThermodynamicTemperature) {
        self.temperature = temperature;
    }
    /// Returns the temperature of this [`RefrIndexAir`].
    pub fn temperature(&self) -> ThermodynamicTemperature {
        self.temperature
    }
    /// Sets the pressure of this [`RefrIndexAir`].
    pub fn set_pressure(&mut self, pressure: Pressure) {
        self.pressure = pressure;
    }
    /// Returns the pressure of this [`RefrIndexAir`].
    pub fn pressure(&self) -> Pressure {
        self.pressure
    }
    /// Sets the humidity of this [`RefrIndexAir`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn set_humidity(&mut self, humidity: f64) -> OpmResult<()> {
        self.humidity = validated!(
            humidity,
            AllFinite && (AllInRange::new(0.0, 100.0, true).unwrap())
        )?;
        Ok(())
    }
    /// Returns the humidity of this [`RefrIndexAir`].
    pub fn humidity(&self) -> f64 {
        *self.humidity.get()
    }
    /// Returns the wvl range of this [`RefrIndexAir`].
    pub fn wvl_range(&self) -> Range<Length> {
        self.wvl_range.clone()
    }
}
impl RefractiveIndex for RefrIndexAir {
    #[inline]
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        if !self.wvl_range.contains(&wavelength) {
            return Err(OpossumError::Other("wavelength outside valid range".into()));
        }
        let s = 1.0 / wavelength.get::<micrometer>();
        let t = self.temperature.get::<degree_celsius>();
        let p = self.pressure.get::<pascal>();
        let p_v = partial_vapor_pressure(self.temperature, self.humidity()).get::<pascal>();
        let n_s = 1.0 + 1.0E-8 * (A + B / (130.0 - s) + C / (38.9 - s));
        let x = (1.0 + 1.0E-8 * (E - F * t) * p) / (1.0 + G * t);
        let n_tp = 1.0 + p * (n_s - 1.0) * x / D;
        let n = n_tp - 1.0E-10 * ((292.75) / (t + 273.15)) * (3.7345 - 0.0401 * s) * p_v;
        Ok(n)
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Air(self.clone())
    }
}
impl From<RefrIndexAir> for RefractiveIndexType {
    fn from(refr: RefrIndexAir) -> Self {
        Self::Air(refr)
    }
}
mod test {
    use super::*;
    use uom::si::pressure::kilopascal;
    #[test]
    fn test_saturation_vapor_pressure() {
        let p = saturation_vapor_pressure(ThermodynamicTemperature::new::<degree_celsius>(-10.0));
        assert_eq!(p.get::<pascal>(), 260.0);
    }
    #[test]
    fn test() {
        let n_air = RefrIndexAir::new(
            ThermodynamicTemperature::new::<degree_celsius>(40.0),
            Pressure::new::<kilopascal>(120.0),
            75.0,
        )
        .unwrap();
        assert_eq!(
            n_air.get_refractive_index(nanometer!(633.0)).unwrap(),
            1.0002899936603902
        );
    }
}
