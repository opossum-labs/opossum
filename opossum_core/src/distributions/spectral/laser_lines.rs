use crate::{
    error::OpmResult,
    generic_validators::{AllNotEmpty, AllPositive, XNormal, YFinite, YNotAllZero},
    nanometer, validated_vec, validated_vec_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uom::si::f64::Length;

use super::SpectralDistribution;

/// The minimum difference between two wavelengths to be considered distinct (in nanometers).
pub const MIN_WAVELENGTH_DIFF_NM: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, EnsureValidated)]
/// A struct representing a collection of laser lines with their respective wavelengths and relative intensities.
///
/// Note: Laser lines must have unique wavelengths. Wavelengths are considered equal if their difference is less than [`MIN_WAVELENGTH_DIFF_NM`] nm.
pub struct LaserLines {
    lines: validated_vec_type!(
        Vec<(Length, f64)>,
        XNormal && YFinite && AllPositive,
        AllNotEmpty && YNotAllZero
    ),
}
impl LaserLines {
    /// Creates a new `LaserLines` instance with the given laser lines.
    ///
    /// The given intensities are normalized to sum to 1.0.
    ///
    /// # Arguments
    ///
    /// * `lines` - A vector of tuples containing the wavelength and intensity of each laser line.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// * the vector is empty,
    /// * any wavelength is negative or infinite,
    /// * any intensity is negative or infinite,
    /// * the sum of intensities is zero,
    /// * any two wavelengths differ by less than [`MIN_WAVELENGTH_DIFF_NM`] nm.
    pub fn new(lines: Vec<(Length, f64)>) -> OpmResult<Self> {
        let mut laser_lines = Self::default();
        laser_lines.set_lines(lines)?;
        Ok(laser_lines)
    }

    /// Adds a list of laser lines to the [`LaserLines`] distribution.
    ///
    /// Each laser line is a tuple containing a [`Length`] representing the wavelength,
    /// and a `f64` representing the intensity.
    ///
    /// # Parameters
    /// * `lines` – A vector of `(Length, f64)` tuples, each representing a spectral line.
    ///
    /// # Returns
    /// * `Ok(())` if all lines are valid and added successfully.
    /// * `Err(OpossumError)` if validation fails.
    ///
    /// # Errors
    /// This method returns an error if:
    /// - The input list is empty.
    /// - Any wavelength is negative or not normal.
    /// - Any intensity is negative or not finite.
    /// - Any new wavelength is already present (difference < [`MIN_WAVELENGTH_DIFF_NM`] nm).
    pub fn add_lines(&mut self, lines: Vec<(Length, f64)>) -> OpmResult<()> {
        let current_lines = self.lines.get();
        for (new_wvl, _) in &lines {
            // Check against existing lines
            if current_lines.iter().any(|(current_wvl, _)| {
                (*current_wvl - *new_wvl).abs() < nanometer!(MIN_WAVELENGTH_DIFF_NM)
            }) {
                return Err(crate::error::OpossumError::Spectrum(format!(
                    "Laser line with wavelength {:.6} nm already exists",
                    new_wvl.get::<uom::si::length::nanometer>()
                )));
            }
        }

        // Check for duplicates within the new lines
        for (i, (wvl1, _)) in lines.iter().enumerate() {
            for (wvl2, _) in lines.iter().skip(i + 1) {
                if (*wvl1 - *wvl2).abs() < nanometer!(MIN_WAVELENGTH_DIFF_NM) {
                    return Err(crate::error::OpossumError::Spectrum(format!(
                        "Duplicate laser line with wavelength {:.6} nm in input",
                        wvl1.get::<uom::si::length::nanometer>()
                    )));
                }
            }
        }

        for line in lines {
            self.lines.push(line)?;
        }
        Ok(())
    }

    /// Sets a list of laser lines to the [`LaserLines`] distribution.
    ///
    /// Each laser line is a tuple containing a [`Length`] representing the wavelength,
    /// and a `f64` representing the intensity.
    ///
    /// # Parameters
    /// * `lines` – A vector of `(Length, f64)` tuples, each representing a spectral line.
    ///
    /// # Returns
    /// * `Ok(())` if all lines are valid and added successfully.
    /// * `Err(OpossumError)` if validation fails.
    ///
    /// # Errors
    /// This method returns an error if:
    /// - The input list is empty.
    /// - Any wavelength is negative or not normal.
    /// - Any intensity is negative or not finite.
    /// - Any wavelengths in the input are duplicates (difference < [`MIN_WAVELENGTH_DIFF_NM`] nm).
    pub fn set_lines(&mut self, lines: Vec<(Length, f64)>) -> OpmResult<()> {
        // Check for duplicates within the new lines
        for (i, (wvl1, _)) in lines.iter().enumerate() {
            for (wvl2, _) in lines.iter().skip(i + 1) {
                if (*wvl1 - *wvl2).abs() < nanometer!(MIN_WAVELENGTH_DIFF_NM) {
                    return Err(crate::error::OpossumError::Spectrum(format!(
                        "Duplicate laser line with wavelength {:.6} nm in input",
                        wvl1.get::<uom::si::length::nanometer>()
                    )));
                }
            }
        }
        self.lines.set(lines)?;
        Ok(())
    }

    /// Returns an immutable reference to the list of laser lines stored in this [`LaserLines`] instance.
    ///
    /// Each line is represented as a tuple `(Length, f64)`, where the `Length` is the wavelength and
    /// `f64` is the corresponding intensity.
    ///
    /// # Returns
    /// A reference to the vector of spectral lines.
    #[must_use]
    pub fn lines(&self) -> &Vec<(Length, f64)> {
        self.lines.get()
    }

    /// Deletes a line form `LaserLines`
    ///
    /// # Errors
    /// Returns an error if setting the new lines fails. More of an esotheric error that should not happen as the other lines have all been validated before
    pub fn delete_line(&mut self, index: usize) -> OpmResult<()> {
        self.lines.remove(index)?;
        Ok(())
    }

    /// Set a line at a given index in this [`LaserLines`].
    /// # Errors
    /// This function will return an error if the index is out of bounds or if the new line is invalid.
    pub fn set_line(&mut self, index: usize, new_line: (Length, f64)) -> OpmResult<()> {
        self.lines.replace(index, new_line)?;
        Ok(())
    }

    /// Returns a reference to a line at a given index from this [`LaserLines`].
    /// # Errors
    /// This function will return an error if the index is out of bounds.
    pub fn get_line(&self, index: usize) -> OpmResult<&(Length, f64)> {
        self.lines.get_at_index(index)
    }
}

impl Default for LaserLines {
    fn default() -> Self {
        Self {
            lines: validated_vec!(
                vec![(nanometer!(1054.), 1.)],
                XNormal && YFinite && AllPositive,
                AllNotEmpty && YNotAllZero
            )
            .unwrap(),
        }
    }
}
impl Serialize for LaserLines {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize directly as a plain list of tuples, eliminating `lines` and `values`
        self.lines().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LaserLines {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum LaserLinesDeHelper {
            // New direct list representation: [(Length, f64), ...]
            Direct(Vec<(Length, f64)>),
            // Intermediate layer containing values: (values: [...])
            Values { values: Vec<(Length, f64)> },
            // Legacy outer layer: (lines: ...)
            Lines { lines: Box<Self> },
        }

        // Recursively unwrap nested legacy structures
        fn extract_lines(helper: LaserLinesDeHelper) -> Vec<(Length, f64)> {
            match helper {
                LaserLinesDeHelper::Direct(v) => v,
                LaserLinesDeHelper::Values { values } => values,
                LaserLinesDeHelper::Lines { lines } => extract_lines(*lines),
            }
        }

        let helper = LaserLinesDeHelper::deserialize(deserializer)?;
        let raw_lines = extract_lines(helper);

        // Re-run validation logic (ensures sorting, difference limits, and positivity)
        Self::new(raw_lines).map_err(serde::de::Error::custom)
    }
}
impl SpectralDistribution for LaserLines {
    /// Generates the laser lines.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the wavelength and intensity of each laser line.
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>> {
        // Normalize the intensities to sum to 1.0
        let sum_intensity: f64 = self
            .lines
            .get()
            .iter()
            .map(|(_, intensity)| *intensity)
            .sum();

        Ok(self
            .lines()
            .clone()
            .iter()
            .map(|(wvl, intensity)| (*wvl, *intensity / sum_intensity))
            .collect::<Vec<(Length, f64)>>())
    }
}
impl From<LaserLines> for super::SpecDistType {
    fn from(laser_lines: LaserLines) -> Self {
        Self::LaserLines(laser_lines)
    }
}

#[cfg(test)]
mod laser_lines_tests {
    use crate::error::OpossumError;

    use super::*;
    use uom::si::f64::Length;

    fn valid_line(wl: f64, intensity: f64) -> (Length, f64) {
        (nanometer!(wl), intensity)
    }

    #[test]
    fn test_default() {
        let laser = LaserLines::default();
        let lines = laser.lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].0.is_normal());
        assert!(lines[0].1 > 0.0);
    }

    #[test]
    fn test_new_valid() -> OpmResult<()> {
        let vec = vec![valid_line(532.0, 0.5), valid_line(1064.0, 0.5)];
        let laser = LaserLines::new(vec.clone())?;
        assert_eq!(laser.lines(), &vec);
        Ok(())
    }

    #[test]
    fn test_new_invalid_empty() {
        let vec: Vec<(Length, f64)> = vec![];
        let res = LaserLines::new(vec);
        assert!(res.is_err());
    }

    #[test]
    fn test_new_invalid_wavelength() {
        let vec = vec![valid_line(-532.0, 0.5)];
        let res = LaserLines::new(vec);
        assert!(res.is_err());
    }

    #[test]
    fn test_new_invalid_intensity_negative() {
        let vec = vec![valid_line(532.0, -1.0)];
        let res = LaserLines::new(vec);
        assert!(res.is_err());
    }

    #[test]
    fn test_new_invalid_intensity_zero_sum() {
        let vec = vec![valid_line(532.0, 0.0), valid_line(1064.0, 0.0)];
        let res = LaserLines::new(vec);
        assert!(res.is_err());
    }

    #[test]
    fn test_add_lines_valid_and_invalid() {
        let mut laser = LaserLines::default();
        assert!(laser.add_lines(vec![valid_line(532.0, 0.5)]).is_ok());

        // Invalid wavelength
        let res = laser.add_lines(vec![valid_line(-100.0, 0.5)]);
        assert!(res.is_err());

        // Invalid intensity
        let res = laser.add_lines(vec![valid_line(532.0, -0.5)]);
        assert!(res.is_err());
    }

    #[test]
    fn test_set_lines() {
        let mut laser = LaserLines::default();

        // Valid set
        let vec = vec![valid_line(532.0, 0.5), valid_line(1064.0, 0.5)];
        assert!(laser.set_lines(vec.clone()).is_ok());
        assert_eq!(laser.lines(), &vec);

        // Invalid: empty vector
        assert!(laser.set_lines(vec![]).is_err());

        // Invalid: negative wavelength
        let vec2 = vec![valid_line(-532.0, 0.5)];
        assert!(laser.set_lines(vec2).is_err());

        // Invalid: intensity zero sum
        let vec3 = vec![valid_line(532.0, 0.0), valid_line(1064.0, 0.0)];
        assert!(laser.set_lines(vec3).is_err());
    }

    #[test]
    fn test_delete_line() -> OpmResult<()> {
        let mut laser = LaserLines::new(vec![valid_line(532.0, 0.5), valid_line(1064.0, 0.5)])?;

        // Valid delete
        assert!(laser.delete_line(0).is_ok());
        assert_eq!(laser.lines().len(), 1);

        // Delete last element → invalid (AllNotEmpty)
        assert!(laser.delete_line(0).is_err());
        assert_eq!(laser.lines().len(), 1); // unchanged
        Ok(())
    }

    #[test]
    fn test_generate_normalized() -> OpmResult<()> {
        let laser = LaserLines::new(vec![valid_line(532.0, 1.0), valid_line(1064.0, 3.0)])?;
        let generated = laser.generate()?;

        // Intensities normalized to sum 1.0
        let sum: f64 = generated.iter().map(|(_, intensity)| *intensity).sum();
        assert!((sum - 1.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_duplicate_wavelengths() {
        let mut laser = LaserLines::default(); // default has 1054.0

        // Try adding same wavelength
        let res = laser.add_lines(vec![valid_line(1054.0, 0.5)]);
        assert!(res.is_err());
        if let Err(crate::error::OpossumError::Spectrum(msg)) = res {
            assert!(msg.contains("already exists"));
        } else {
            panic!("Wrong error type");
        }

        // Try adding duplicates within input
        let res = laser.add_lines(vec![valid_line(600.0, 0.5), valid_line(600.0, 0.5)]);
        assert!(res.is_err());
        if let Err(crate::error::OpossumError::Spectrum(msg)) = res {
            assert!(msg.contains("Duplicate laser line"));
        } else {
            panic!("Wrong error type");
        }
    }

    #[test]
    fn test_set_duplicate_wavelengths() {
        let mut laser = LaserLines::default();

        let res = laser.set_lines(vec![valid_line(532.0, 0.5), valid_line(532.0, 0.5)]);
        assert!(res.is_err());
    }
    #[test]
    fn test_laser_lines_flat_serde_and_legacy_roundtrip() -> OpmResult<()> {
        let lines_vec = vec![valid_line(532.0, 0.5), valid_line(1064.0, 0.5)];
        let laser = LaserLines::new(lines_vec.clone())?;

        // 1. Verify serialization produces a clean direct list
        let ron_str = ron::to_string(&laser).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!ron_str.contains("lines:"));
        assert!(!ron_str.contains("values:"));

        let restored: LaserLines =
            ron::from_str(&ron_str).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(laser, restored);

        // 2. Verify backward compatibility with full legacy nesting
        let legacy_ron = r#"(
            lines: (
                values: [
                    (0.000000532, 0.5),
                    (0.000001064, 0.5),
                ],
            ),
        )"#;
        let restored_legacy: LaserLines =
            ron::from_str(legacy_ron).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(laser, restored_legacy);

        // 3. Verify backward compatibility with intermediate (values: [...])
        let intermediate_ron = r#"(
            values: [
                (0.000000532, 0.5),
                (0.000001064, 0.5),
            ],
        )"#;
        let restored_intermediate: LaserLines =
            ron::from_str(intermediate_ron).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(laser, restored_intermediate);

        Ok(())
    }
}
