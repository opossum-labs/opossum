#![warn(missing_docs)]
//! Module for handling optical rays
use nalgebra::{MatrixXx3, Point3, Vector3};
use num::Zero;
use serde_derive::{Deserialize, Serialize};
use uom::si::{
    f64::{Energy, Length},
    length::millimeter,
};

use crate::{
    error::{OpmResult, OpossumError},
    nodes::FilterType,
    properties::Proptype,
    spectrum::Spectrum, surface::Surface,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// The configuration for splitting a [`Ray`].
pub enum SplittingConfig {
    /// Ideal beam splitter with a fixed splitting ratio
    Ratio(f64),
    /// A beam splitter with a given transmission spectrum
    Spectrum(Spectrum),
}
impl From<SplittingConfig> for Proptype {
    fn from(value: SplittingConfig) -> Self {
        Self::SplitterType(value)
    }
}
impl SplittingConfig {
    /// Check validity of [`SplittingConfig`].
    ///
    /// This function returns ture if all values in a spectrum or the ratio is in the range (0.0..=1.0).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Ratio(r) => (0.0..=1.0).contains(r),
            Self::Spectrum(s) => s.is_transmission_spectrum(),
        }
    }
}
///Struct that contains all information about an optical ray
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Ray {
    ///Stores the current position of the ray (in mm)
    pos: Point3<f64>,
    ///Stores the position history of the ray (in mm)
    pos_hist: Vec<Point3<f64>>,
    /// stores the current propagation direction of the ray (stored as direction cosine)
    dir: Vector3<f64>,
    // ///stores the polarization vector (Jones vector) of the ray
    // pol: Vector2<Complex<f64>>,
    /// Energy of the ray
    e: Energy,
    ///Wavelength of the ray
    wvl: Length,
    // ///Bounce count of the ray. Necessary to check if the maximum number of bounces is reached
    // bounce: usize,
    // //True if ray is allowd to further propagate, false else
    // //valid:  bool,
    path_length: Length,
    // refractive index of the medium this ray is propagatin in.
    refractive_index: f64,
}
impl Ray {
    /// Create a new collimated ray.
    ///
    /// Generate a ray a horizontally polarized ray collinear with the z axis (optical axis).
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given wavelength is <= 0.0, `NaN` or +inf
    ///  - the given energy is < 0.0, `NaN` or +inf
    pub fn new_collimated(
        position: Point3<Length>,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        Self::new(position, Vector3::z(), wave_length, energy)
    }
    /// Creates a new [`Ray`].
    ///
    /// The dircetion vector is normalized. The direction is thus stored as (`direction cosine`)[`https://en.wikipedia.org/wiki/Direction_cosine`]
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given wavelength is <= 0.0, `NaN` or +inf
    ///  - the given energy is < 0.0, `NaN` or +inf
    ///  - the direction vector has a zero length
    pub fn new(
        position: Point3<Length>,
        direction: Vector3<f64>,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        if wave_length.is_zero() || wave_length.is_sign_negative() || !wave_length.is_finite() {
            return Err(OpossumError::Other("wavelength must be >0".into()));
        }
        if energy.is_sign_negative() || !energy.is_finite() {
            return Err(OpossumError::Other("energy must be >0".into()));
        }
        if direction.norm().is_zero() {
            return Err(OpossumError::Other("length of direction must be >0".into()));
        }
        let init_pos = Point3::new(
            position.x.get::<millimeter>(),
            position.y.get::<millimeter>(),
            0.0,
        );
        let mut pos_hist = Vec::<Point3<f64>>::with_capacity(50);
        pos_hist.push(init_pos);
        Ok(Self {
            pos: init_pos,
            pos_hist,
            dir: direction.normalize(),
            //pol: Vector2::new(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)), // horizontal polarization
            e: energy,
            wvl: wave_length,
            //id: 0,
            //bounce: 0,
            path_length: Length::zero(),
            refractive_index: 1.0,
        })
    }
    /// Returns the position of thi [`Ray`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        Point3::new(
            Length::new::<millimeter>(self.pos.x),
            Length::new::<millimeter>(self.pos.y),
            Length::new::<millimeter>(self.pos.z),
        )
    }
    /// Returns the energy of this [`Ray`].
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.e
    }
    /// Returns the wavelength of this [`Ray`].
    #[must_use]
    pub fn wavelength(&self) -> Length {
        self.wvl
    }

    /// Returns the position history of this [`Ray`].
    #[must_use]
    pub fn position_history_in_mm(&self) -> MatrixXx3<f64> {
        let mut pos_mm = MatrixXx3::zeros(self.pos_hist.len());

        for (idx, pos) in self.pos_hist.iter().enumerate() {
            pos_mm[(idx, 0)] = pos.x;
            pos_mm[(idx, 1)] = pos.y;
            pos_mm[(idx, 2)] = pos.z;
        }
        pos_mm
    }
    /// freely propagate a ray along its direction. The length is given as the projection on the z-axis (=optical axis).
    ///
    /// This function also respects the refractive index sotred in the ray while calculating the optical path length.
    ///
    /// # Errors
    /// This functions retruns an error if the initial ray direction has a zero z component (= ray not propagating in z direction).
    pub fn propagate_along_z(&mut self, length_along_z: Length) -> OpmResult<()> {
        if self.dir[2].abs() < f64::EPSILON {
            return Err(OpossumError::Other(
                "z-Axis of direction vector must be != 0.0".into(),
            ));
        }
        let length_in_ray_dir = length_along_z.get::<millimeter>() / self.dir[2];
        self.pos += length_in_ray_dir * self.dir;
        self.pos_hist.push(self.pos);

        let normalized_dir = self.dir.normalize();
        let length_in_ray_dir = length_along_z.get::<millimeter>() / normalized_dir[2];
        self.path_length += Length::new::<millimeter>(length_in_ray_dir) * self.refractive_index;
        Ok(())
    }
    /// Refract a ray on a paraxial surface of a given focal length.
    ///
    /// Modify the ray direction in order to simulate a perfect lens. **Note**: This function also
    /// modifies the path length of a ray in order to return correct values for the wavefront.
    /// # Errors
    ///
    /// This function will return an error if the given focal length is zero or not finite
    pub fn refract_paraxial(&mut self, focal_length: Length) -> OpmResult<()> {
        if focal_length.is_zero() || !focal_length.is_finite() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 & finite".into(),
            ));
        }
        let f = focal_length.get::<millimeter>();
        let optical_power = 1.0 / f;
        self.dir.x = optical_power.mul_add(-self.pos.x, self.dir.x);
        self.dir.y = optical_power.mul_add(-self.pos.y, self.dir.y);
        self.dir.z = 1.0;
        // correct path length
        let r_square = self.pos.x.mul_add(self.pos.x, self.pos.y * self.pos.y);
        let f_square = f * f;
        self.path_length -= Length::new::<millimeter>((r_square + f_square).sqrt()) - focal_length;
        Ok(())
    }
    /// Refract a ray on a given surface using Snellius' law.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn refract_on_surface(&mut self, s: &dyn Surface, n2: f64) -> OpmResult<()> {
        if n2<1.0 || !n2.is_finite() {
            return Err(OpossumError::Other("the refractive index must be >=1.0 and finite".into()));
        }
        if let Some((intersection_point, surface_normal)) = s.calc_intersect_and_normal(self) {
            self.pos=intersection_point.map(|c| c.get::<millimeter>());

            Ok(())
        }
        else {
            Ok(())
        }
    }
    /// Attenuate a ray's energy by a given filter.
    ///
    /// This function attenuates the ray's energy by the given [`FilterType`]. For [`FilterType::Constant`] the energy is simply multiplied by the
    /// given transmission factor.
    /// # Errors
    ///
    /// This function will return an error if the transmission factor for the [`FilterType::Constant`] is not within the interval `(0.0..=1.0)`
    pub fn filter_energy(&self, filter: &FilterType) -> OpmResult<Self> {
        let transmission = match filter {
            FilterType::Constant(t) => {
                if !(0.0..=1.0).contains(t) {
                    return Err(OpossumError::Other(
                        "transmission factor must be within (0.0..=1.0)".into(),
                    ));
                }
                *t
            }
            FilterType::Spectrum(s) => {
                let transmission = s.get_value(&self.wavelength());
                if let Some(t) = transmission {
                    t
                } else {
                    return Err(OpossumError::Other(
                        "wavelength of ray outside filter spectrum".into(),
                    ));
                }
            }
        };
        let mut new_ray = self.clone();
        new_ray.e *= transmission;
        Ok(new_ray)
    }
    /// Split a ray with the given energy splitting ratio.
    ///
    /// This function modifies the energy of the existing ray and generates a new split ray. The splitting strategy is determined by the
    /// given [`SplittingConfig`]:
    ///
    /// ## [`SplittingConfig::Ratio`]
    ///
    /// The splitting ratio must be within the range `(0.0..=1.0)`. A ratio of 1.0 means that all energy remains in the initial beam
    /// and the split beam has an energy of zero. A ratio of 0.0 corresponds to a fully reflected beam.
    ///
    /// ## [`SplittingConfig::Spectrum`]
    ///
    /// The splitting ratio is determined by the wavelength
    /// of the ray and the given transmission / reflection spectrum. This [`Spectrum`] must contain values in the range (0.0..=1.0). A spectrum value
    /// of 1.0 means that all energy remains in the initial beam and the split beam has an energy of zero. A spectrum value of 0.0 corresponds to
    /// a fully reflected beam.
    ///
    /// # Errors
    ///
    /// This function will return an error if `splitting_ratio` is outside the interval [0.0..1.0] or the wavelength of the ray is outside the given
    /// spectrum.
    pub fn split(&mut self, config: &SplittingConfig) -> OpmResult<Self> {
        let splitting_ratio = match config {
            SplittingConfig::Ratio(ratio) => *ratio,
            SplittingConfig::Spectrum(spectrum) => {
                (*spectrum).get_value(&self.wvl).ok_or_else(|| {
                    OpossumError::Spectrum(
                        "ray splitting failed. wavelength outside given spectrum".into(),
                    )
                })?
            }
        };
        if !(0.0..=1.0).contains(&splitting_ratio) {
            return Err(OpossumError::Other(
                "splitting_ratio must be within [0.0;1.0]".into(),
            ));
        }
        let mut split_ray = self.clone();
        self.e *= splitting_ratio;
        split_ray.e *= 1.0 - splitting_ratio;
        Ok(split_ray)
    }
    /// Returns the direction of this [`Ray`].
    ///
    /// Return the ray direction vector as directional cosine. **Note**: This vector is not necessarily normalized.
    #[must_use]
    pub const fn direction(&self) -> Vector3<f64> {
        self.dir
    }
    /// Returns the path length of this [`Ray`].
    ///
    /// Return the geometric path length of the ray.
    #[must_use]
    pub fn path_length(&self) -> Length {
        self.path_length
    }
    /// Returns the refractive index of this [`Ray`].
    #[must_use]
    pub const fn refractive_index(&self) -> f64 {
        self.refractive_index
    }
    /// Sets the refractive index of this [`Ray`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive inde is <1.0 or not finite.
    pub fn set_refractive_index(&mut self, refractive_index: f64) -> OpmResult<()> {
        if refractive_index < 1.0 || !refractive_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index must be >=1.0 and finite".into(),
            ));
        }
        self.refractive_index = refractive_index;
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        spectrum::Spectrum,
        spectrum_helper::{self, generate_filter_spectrum},
    };
    use approx::{abs_diff_eq, assert_abs_diff_eq};
    use itertools::izip;
    use std::path::PathBuf;
    use uom::si::{energy::joule, length::nanometer};
    #[test]
    fn new_collimated() {
        let position = Point3::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
            Length::new::<millimeter>(0.0),
        );
        let ray = Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        );
        assert!(ray.is_ok());
        let ray = ray.unwrap();
        assert_eq!(ray.pos, Point3::new(1.0, 2.0, 0.0));
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.wvl, Length::new::<nanometer>(1053.0));
        assert_eq!(ray.e, Energy::new::<joule>(1.0));
        assert_eq!(ray.path_length, Length::zero());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(0.0),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(-10.0),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(f64::NAN),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(f64::INFINITY),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(f64::NEG_INFINITY),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(0.0)
        )
        .is_ok());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(-0.1)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::NAN)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::INFINITY)
        )
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::NEG_INFINITY)
        )
        .is_err());
    }
    #[test]
    fn new() {
        let position = Point3::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
            Length::new::<millimeter>(0.0),
        );
        let direction = Vector3::new(0.0, 0.0, 2.0);
        let ray = Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        );
        assert!(ray.is_ok());
        let ray = ray.unwrap();
        assert_eq!(ray.pos, Point3::new(1.0, 2.0, 0.0));
        assert_eq!(
            ray.position(),
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(2.0),
                Length::zero()
            )
        );
        assert_eq!(ray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(ray.wvl, Length::new::<nanometer>(1053.0));
        assert_eq!(ray.wavelength(), Length::new::<nanometer>(1053.0));
        assert_eq!(ray.e, Energy::new::<joule>(1.0));
        assert_eq!(ray.energy(), Energy::new::<joule>(1.0));
        assert_eq!(ray.path_length, Length::zero());
        assert_eq!(ray.refractive_index, 1.0);
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(0.0),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(-10.0),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(f64::NAN),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(f64::INFINITY),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(f64::NEG_INFINITY),
            Energy::new::<joule>(1.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(-0.1)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::NAN)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::INFINITY)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(f64::NEG_INFINITY)
        )
        .is_err());
        assert!(Ray::new(
            position,
            Vector3::new(0.0, 0.0, 0.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0)
        )
        .is_err());
    }
    #[test]
    fn refractive_index() {
        let wvl = Length::new::<nanometer>(1053.0);
        let energy = Energy::new::<joule>(1.0);
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        ray.refractive_index = 2.0;
        assert_eq!(ray.refractive_index(), 2.0);
    }
    #[test]
    fn set_refractive_index() {
        let wvl = Length::new::<nanometer>(1053.0);
        let energy = Energy::new::<joule>(1.0);
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray.set_refractive_index(f64::NAN).is_err());
        assert!(ray.set_refractive_index(f64::INFINITY).is_err());
        assert!(ray.set_refractive_index(0.99).is_err());
        assert!(ray.set_refractive_index(1.0).is_ok());
        assert!(ray.set_refractive_index(2.0).is_ok());
        assert_eq!(ray.refractive_index, 2.0);
    }
    #[test]
    fn propagate_along_z() {
        let wvl = Length::new::<nanometer>(1053.0);
        let energy = Energy::new::<joule>(1.0);
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray
            .propagate_along_z(Length::new::<millimeter>(1.0))
            .is_ok());
        ray.propagate_along_z(Length::new::<millimeter>(1.0))
            .unwrap();
        assert_eq!(ray.wavelength(), wvl);
        assert_eq!(ray.energy(), energy);
        assert_eq!(ray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(2.0)
            )
        );
        assert_eq!(ray.path_length(), Length::new::<millimeter>(2.0));
        let _ = ray.propagate_along_z(Length::new::<millimeter>(2.0));

        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(4.0)
            )
        );
        let _ = ray.propagate_along_z(Length::new::<millimeter>(-5.0));

        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(-1.0)
            )
        );
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 1.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        let _ = ray.propagate_along_z(Length::new::<millimeter>(1.0));
        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0)
            )
        );
        let _ = ray.propagate_along_z(Length::new::<millimeter>(2.0));

        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(3.0),
                Length::new::<millimeter>(3.0)
            )
        );
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 1.0, 0.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray
            .propagate_along_z(Length::new::<millimeter>(1.0))
            .is_err());
    }
    #[test]
    fn propagate_along_z_refractive_index() {
        let wvl = Length::new::<nanometer>(1053.0);
        let energy = Energy::new::<joule>(1.0);
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        ray.set_refractive_index(2.0).unwrap();
        ray.propagate_along_z(Length::new::<millimeter>(1.0))
            .unwrap();
        assert_eq!(ray.wavelength(), wvl);
        assert_eq!(ray.energy(), energy);
        assert_eq!(ray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(
            ray.position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(1.0)
            )
        );
        assert_eq!(ray.path_length(), Length::new::<millimeter>(2.0));
    }
    #[test]
    fn refract_paraxial() {
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray_dir = ray.dir;
        let ray_pos = ray.pos;

        let mut refracted_ray = ray.clone();
        refracted_ray
            .refract_paraxial(Length::new::<millimeter>(100.0))
            .unwrap();
        assert_eq!(refracted_ray.pos, ray.pos);
        assert_eq!(refracted_ray.dir, ray.dir);
        assert_eq!(refracted_ray.e, ray.e);
        assert_eq!(refracted_ray.path_length, ray.path_length);

        assert!(ray
            .refract_paraxial(Length::new::<millimeter>(0.0))
            .is_err());
        assert!(ray
            .refract_paraxial(Length::new::<millimeter>(f64::NAN))
            .is_err());
        assert!(ray
            .refract_paraxial(Length::new::<millimeter>(f64::INFINITY))
            .is_err());
        assert!(ray
            .refract_paraxial(Length::new::<millimeter>(f64::NEG_INFINITY))
            .is_err());
        let _ = ray.refract_paraxial(Length::new::<millimeter>(-100.0));
        assert_eq!(ray.pos, ray_pos);
        assert_eq!(ray.dir, ray_dir);
        let mut ray = Ray::new_collimated(
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(2.0),
                Length::new::<millimeter>(0.0),
            ),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray_pos = ray.pos;
        let _ = ray.refract_paraxial(Length::new::<millimeter>(100.0));
        assert_eq!(ray.pos, ray_pos);

        let test_ray_dir = Vector3::new(-1.0, -2.0, 100.0) / 100.0;
        assert_abs_diff_eq!(ray.dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray.dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray.dir.z, test_ray_dir.z);
        let _ = ray.refract_paraxial(Length::new::<millimeter>(-100.0));
        assert_eq!(ray.pos, ray_pos);
        let _ = ray.refract_paraxial(Length::new::<millimeter>(-100.0));

        let test_ray_dir = Vector3::new(1.0, 2.0, 100.0) / 100.0;
        assert_abs_diff_eq!(ray.dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray.dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray.dir.z, test_ray_dir.z);

        let mut ray = Ray::new(
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(10.0),
                Length::zero(),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let _ = ray.refract_paraxial(Length::new::<millimeter>(10.0));
        assert_abs_diff_eq!(
            ray.path_length.get::<millimeter>(),
            -1.0 * (f64::sqrt(200.0) - 10.0),
            epsilon = 10.0 * f64::EPSILON
        );
    }
    #[test]
    fn filter_energy() {
        let position = Point3::new(
            Length::zero(),
            Length::new::<millimeter>(1.0),
            Length::zero(),
        );
        let wvl = Length::new::<nanometer>(1054.0);
        let ray = Ray::new_collimated(position, wvl, Energy::new::<joule>(1.0)).unwrap();
        let new_ray = ray.filter_energy(&FilterType::Constant(0.3)).unwrap();
        assert_eq!(new_ray.pos, Point3::new(0.0, 1.0, 0.0));
        assert_eq!(new_ray.dir, Vector3::z());
        assert_eq!(new_ray.wvl, wvl);
        assert_eq!(new_ray.e, Energy::new::<joule>(0.3));
        assert!(ray.filter_energy(&FilterType::Constant(-0.1)).is_err());
        assert!(ray.filter_energy(&FilterType::Constant(1.1)).is_err());
    }
    #[test]
    fn filter_spectrum() {
        let position = Point3::new(
            Length::zero(),
            Length::new::<millimeter>(1.0),
            Length::zero(),
        );
        let e_1j = Energy::new::<joule>(1.0);
        let ray = Ray::new_collimated(position, Length::new::<nanometer>(502.0), e_1j).unwrap();
        let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        spec_path.push("files_for_testing/spectrum/test_filter.csv");
        let s = Spectrum::from_csv(spec_path.to_str().unwrap()).unwrap();
        let filter = FilterType::Spectrum(s);
        let filtered_ray = ray.filter_energy(&filter).unwrap();
        assert_eq!(filtered_ray.energy(), Energy::new::<joule>(1.0));
        let ray = Ray::new_collimated(position, Length::new::<nanometer>(500.0), e_1j).unwrap();
        let filtered_ray = ray.filter_energy(&filter).unwrap();
        assert_eq!(filtered_ray.energy(), Energy::new::<joule>(0.0));
        let ray = Ray::new_collimated(position, Length::new::<nanometer>(501.5), e_1j).unwrap();
        let filtered_ray = ray.filter_energy(&filter).unwrap();
        assert!(abs_diff_eq!(
            filtered_ray.energy().get::<joule>(),
            0.5,
            epsilon = 300.0 * f64::EPSILON
        ));
        let ray = Ray::new_collimated(position, Length::new::<nanometer>(506.0), e_1j).unwrap();
        assert!(ray.filter_energy(&filter).is_err());
    }
    #[test]
    fn split_by_ratio() {
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert!(ray.split(&SplittingConfig::Ratio(1.1)).is_err());
        assert!(ray.split(&SplittingConfig::Ratio(-0.1)).is_err());
        let split_ray = ray.split(&SplittingConfig::Ratio(0.1)).unwrap();
        assert_eq!(ray.energy(), Energy::new::<joule>(0.1));
        assert_eq!(split_ray.energy(), Energy::new::<joule>(0.9));
        assert_eq!(ray.position(), split_ray.position());
        assert_eq!(ray.dir, split_ray.dir);
        assert_eq!(ray.wavelength(), split_ray.wavelength());
    }
    #[test]
    fn split_by_spectrum() {
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1000.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let spectrum = generate_filter_spectrum(
            Length::new::<nanometer>(500.0)..Length::new::<nanometer>(1500.0),
            Length::new::<nanometer>(1.0),
            &spectrum_helper::FilterType::ShortPassStep {
                cut_off: Length::new::<nanometer>(1000.0),
            },
        )
        .unwrap();
        let splitting_config = SplittingConfig::Spectrum(spectrum);
        let split_ray = ray.split(&splitting_config).unwrap();
        assert_eq!(ray.energy(), Energy::new::<joule>(0.0));
        assert_eq!(split_ray.energy(), Energy::new::<joule>(1.0));
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1001.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let split_ray = ray.split(&splitting_config).unwrap();
        assert_eq!(ray.energy(), Energy::zero());
        assert_eq!(split_ray.energy(), Energy::new::<joule>(1.0));
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(999.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let split_ray = ray.split(&&splitting_config).unwrap();
        assert_eq!(ray.energy(), Energy::new::<joule>(1.0));
        assert_eq!(split_ray.energy(), Energy::zero());
    }
    #[test]
    fn split_by_spectrum_fail() {
        let mut ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1501.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let spectrum = generate_filter_spectrum(
            Length::new::<nanometer>(500.0)..Length::new::<nanometer>(1500.0),
            Length::new::<nanometer>(1.0),
            &spectrum_helper::FilterType::ShortPassStep {
                cut_off: Length::new::<nanometer>(1000.0),
            },
        )
        .unwrap();
        assert!(ray.split(&SplittingConfig::Spectrum(spectrum)).is_err());
    }
    #[test]
    fn position_history_in_mm_test() {
        let mut ray = Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0., 1., 2.),
            Length::new::<nanometer>(1053.),
            Energy::new::<joule>(1.),
        )
        .unwrap();

        let _ = ray.propagate_along_z(Length::new::<millimeter>(1.));
        let _ = ray.propagate_along_z(Length::new::<millimeter>(2.));

        let pos_hist_comp = MatrixXx3::from_vec(vec![0., 0., 0., 0., 0.5, 1.5, 0., 1., 3.]);

        let pos_hist = ray.position_history_in_mm();
        for (row, row_calc) in izip!(pos_hist_comp.row_iter(), pos_hist.row_iter()) {
            assert_eq!(row[0], row_calc[0]);
            assert_eq!(row[1], row_calc[1]);
            assert_eq!(row[2], row_calc[2]);
        }
    }
}
