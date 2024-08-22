#![warn(missing_docs)]
//! Module for handling optical rays
use std::fmt::Display;

use nalgebra::{vector, MatrixXx3, Point3, Vector3};
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{meter, nanometer},
};

use crate::{
    error::{OpmResult, OpossumError},
    meter,
    nodes::FilterType,
    spectrum::Spectrum,
    surface::OpticalSurface,
    utils::geom_transformation::Isometry,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// The configuration for splitting a [`Ray`].
pub enum SplittingConfig {
    /// Ideal beam splitter with a fixed splitting ratio
    Ratio(f64),
    /// A beam splitter with a given transmission spectrum
    Spectrum(Spectrum),
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
    /// Stores the current position of the ray
    pos: Point3<Length>,
    /// Stores the position history of the ray
    pos_hist: Vec<Point3<Length>>,
    /// Stores the current propagation direction of the ray (stored as direction cosine)
    dir: Vector3<f64>,
    // ///stores the polarization vector (Jones vector) of the ray
    // pol: Vector2<Complex<f64>>,
    /// Energy of the ray
    e: Energy,
    /// Wavelength of the ray
    wvl: Length,
    /// Bounce count of the ray. Used as stop criterion.
    number_of_bounces: usize,
    /// Refraction count of the ray. Used as stop criterion.
    number_of_refractions: usize,
    /// True if ray is allowd to further propagate, false else
    valid: bool,
    /// optical path length of the ray
    path_length: Length,
    // refractive index of the medium this ray is propagating in.
    refractive_index: f64,
}
impl Ray {
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
        Ok(Self {
            pos: position,
            pos_hist: Vec::<Point3<Length>>::with_capacity(50),
            dir: direction.normalize(),
            //pol: Vector2::new(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)), // horizontal polarization
            e: energy,
            wvl: wave_length,
            path_length: Length::zero(),
            refractive_index: 1.0,
            number_of_bounces: 0,
            number_of_refractions: 0,
            valid: true,
        })
    }
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
    /// Create a ray with a position at the global coordinate origin pointing along the positive z-axis.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the wavelength is <= 0.0 nm or not finite
    ///  - the energy is < 0.0 or not finite
    pub fn origin_along_z(wave_length: Length, energy: Energy) -> OpmResult<Self> {
        Self::new_collimated(Point3::origin(), wave_length, energy)
    }
    /// Returns the position of this [`Ray`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.pos
    }
    /// Returns the direction of this [`Ray`].
    ///
    /// Return the ray direction vector as directional cosine. **Note**: This vector is not necessarily normalized.
    #[must_use]
    pub const fn direction(&self) -> Vector3<f64> {
        self.dir
    }
    /// Sets the direction of this [`Ray`].
    ///
    /// # Errors
    ///
    /// This function will return an error if an invalid direction vector is provided.
    pub fn set_direction(&mut self, dir: Vector3<f64>) -> OpmResult<()> {
        if dir.norm().is_zero() {
            return Err(OpossumError::Other("length of direction must be >0".into()));
        }
        self.dir = dir;
        Ok(())
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
    /// Adds a position to the position history of the ray.
    /// Adds a position to the position history of the ray.
    /// This is, for example, necessary for adding the position when the ray may be set invalid at an aperture.
    pub fn add_to_pos_hist(&mut self, pos: Point3<Length>) {
        self.pos_hist.push(pos);
    }
    /// Returns the position history of this [`Ray`].
    ///
    /// This function returns a matrix with all positions (end of propagation and intersection points) of a ray path.
    /// **Note**: This function adds to current ray position to the list.
    #[must_use]
    pub fn position_history(&self) -> MatrixXx3<Length> {
        let nr_of_pos = self.pos_hist.len();
        let mut positions = MatrixXx3::<Length>::zeros(nr_of_pos + 1);

        for (idx, pos) in self.pos_hist.iter().enumerate() {
            positions[(idx, 0)] = pos.x;
            positions[(idx, 1)] = pos.y;
            positions[(idx, 2)] = pos.z;
        }
        positions[(nr_of_pos, 0)] = self.pos.x;
        positions[(nr_of_pos, 1)] = self.pos.y;
        positions[(nr_of_pos, 2)] = self.pos.z;
        positions
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
    /// This function will return an error if the given refractive index is <1.0 or not finite.
    pub fn set_refractive_index(&mut self, refractive_index: f64) -> OpmResult<()> {
        if refractive_index < 1.0 || !refractive_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index must be >=1.0 and finite".into(),
            ));
        }
        self.refractive_index = refractive_index;
        Ok(())
    }
    /// Propagate a ray freely along its direction by the given length.
    ///
    /// This function also respects the refractive index stored in the ray while calculating the optical path length.
    ///
    /// # Errors
    /// This functions returns an error if
    ///   - the initial ray direction vector is zero. (This should not happen at all.)
    ///   - the propagation length is not finite.
    pub fn propagate(&mut self, length: Length) -> OpmResult<()> {
        if self.dir.is_zero() {
            return Err(OpossumError::Other(
                "cannot propagate since length of direction vector must be >0".into(),
            ));
        }
        if !length.is_finite() {
            return Err(OpossumError::Other(
                "propagation length must be finite".into(),
            ));
        }
        self.pos_hist.push(self.pos);
        self.pos += vector![
            length * self.dir.x,
            length * self.dir.y,
            length * self.dir.z
        ];
        self.path_length += length * self.refractive_index * self.dir.norm();
        Ok(())
    }
    /// Create an [`Isometry`] from this [`Ray`].
    ///
    /// This function creates an [`Isometry`] with its position based on the ray position and the orientation (rotation) baed on the ray direction.
    #[must_use]
    pub fn to_isometry(&self) -> Isometry {
        Isometry::new_from_view(self.position(), self.direction(), Vector3::y())
    }
    /// Propagate a ray freely along its direction. The length is given as the projection on the z-axis (=optical axis).
    ///
    /// This function also respects the refractive index stored in the ray while calculating the optical path length.
    ///
    /// # Errors
    /// This functions returns an error if the initial ray direction has a zero z component (= ray not propagating in z direction).
    // pub fn propagate_along_z(&mut self, length_along_z: Length) -> OpmResult<()> {
    //     if self.dir[2].abs() < f64::EPSILON {
    //         return Err(OpossumError::Other(
    //             "z-Axis of direction vector must be != 0.0".into(),
    //         ));
    //     }
    //     self.pos_hist.push(self.pos);
    //     let length_in_ray_dir = length_along_z / self.dir[2];
    //     self.pos += vector![
    //         length_in_ray_dir * self.dir.x,
    //         length_in_ray_dir * self.dir.y,
    //         length_in_ray_dir * self.dir.z
    //     ];
    //     self.path_length += length_in_ray_dir * self.refractive_index * self.dir.norm();
    //     Ok(())
    // }
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
        let optical_power = 1.0 / focal_length;
        self.dir /= self.dir.z;
        self.dir.x -= (optical_power * self.pos.x).value;
        self.dir.y -= (optical_power * self.pos.y).value;

        // correct path length
        let r_square = self
            .pos
            .x
            .value
            .mul_add(self.pos.x.value, self.pos.y.value * self.pos.y.value);
        let f_square = (focal_length * focal_length).value;
        self.path_length -= meter!((r_square + f_square).sqrt()) - focal_length.abs();
        self.number_of_refractions += 1;
        Ok(())
    }
    /// Refract the [`Ray`] on a given [`Surface`] using Snellius' law.
    ///
    /// This function refracts an incoming [`Ray`] on a given [`OpticalSurface`] thereby changing its position (= intersection point) and
    /// its direction. The intial refractive index is (already) stored in the ray itself. The refractive index behind the surface is given
    /// by the parameter `n2`. In addition, it returns a possible reflected [`Ray`], which corresponds to the refracted ray (same position,
    /// wavelength) but with the reflection direction.
    ///
    /// This function also considers a possible surface coating which modifies the energy of the refracted and the reflected beam. If the
    /// [`Ray`] does not intersect with the surface, the [`Ray`] is unmodified and `None` is returned (since there is no reflection).
    ///
    /// This function also considers total reflection: If the n1 > n2 and the incoming angle is larger than Brewster's angle, the beam
    /// is totally reflected. In this case, this function also returns `None` (since there is no additional reflected ray).
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive index `n2` if <1.0 or not finite.
    pub fn refract_on_surface(&mut self, os: &OpticalSurface, n2: f64) -> OpmResult<Option<Self>> {
        if n2 < 1.0 || !n2.is_finite() {
            return Err(OpossumError::Other(
                "the refractive index must be >=1.0 and finite".into(),
            ));
        }
        if let Some((intersection_point, surface_normal)) =
            os.geo_surface().calc_intersect_and_normal(self)
        {
            // Snell's law in vector form (src: https://www.starkeffects.com/snells-law-vector.shtml)
            // mu=n_1 / n_2
            // s1: incoming direction (normalized??)
            // n: surface normal (normalized??)
            // s2: refracted dir
            //
            // s2 = mu * [ n x ( -n x s1) ] - n* sqrt(1 - mu^2 * (n x s1) dot (n x s1))
            let mu = self.refractive_index / n2;
            let s1 = self.dir.normalize();
            let n = surface_normal.normalize();
            let dis = (mu * mu).mul_add(-n.cross(&s1).dot(&n.cross(&s1)), 1.0);
            let reflected_dir = s1 - 2.0 * (s1.dot(&n)) * n;
            let pos_in_m = self.pos.map(|c| c.value);
            let intersection_in_m = intersection_point.map(|c| c.value);
            self.path_length +=
                self.refractive_index * meter!((pos_in_m - intersection_in_m).norm());
            self.pos_hist.push(self.pos);
            self.pos = intersection_point;
            // check, if total reflection
            if dis.is_sign_positive() {
                // handle energy (due to coating)
                let reflectivity = os.coating().calc_reflectivity(self, surface_normal, n2)?;
                let input_enery = self.energy();
                let refract_dir = mu * (n.cross(&(-1.0 * n.cross(&s1))))
                    - n * f64::sqrt((mu * mu).mul_add(-n.cross(&s1).dot(&n.cross(&s1)), 1.0));
                self.dir = refract_dir;
                self.e = input_enery * (1. - reflectivity);
                let mut reflected_ray = self.clone();
                reflected_ray.dir = reflected_dir;
                reflected_ray.e = input_enery * reflectivity;
                reflected_ray.number_of_bounces += 1;
                self.refractive_index = n2;
                self.number_of_refractions += 1;
                Ok(Some(reflected_ray))
            } else {
                self.number_of_bounces += 1;
                self.dir = reflected_dir;
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    /// Attenuate a ray's energy by a given filter.
    ///
    /// This function attenuates the ray's energy by the given [`FilterType`]. For [`FilterType::Constant`] the energy is simply multiplied by the
    /// given transmission factor.
    /// # Errors
    ///
    /// This function will return an error if the transmission factor for the [`FilterType::Constant`] is not within the interval `(0.0..=1.0)`
    pub fn filter_energy(&mut self, filter: &FilterType) -> OpmResult<()> {
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
        self.e *= transmission;
        // let mut new_ray = self.clone();
        // new_ray.e *= transmission;
        Ok(())
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
    /// **Note**: This function only copies the initial ray and modifies the energies. The split ray has the same position and direction as the
    /// original ray.
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
    /// Returns the validity of this [`Ray`].
    ///
    /// The `valid` status denotes, if a [`Ray`] should be further propagated thorugh a system. A [`Ray`] is set to invalid if e.g.
    /// its energy is below a given energy threshold or missed an optical surface.
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.valid
    }
    /// Invalidates this [`Ray`].
    pub fn set_invalid(&mut self) {
        self.valid = false;
    }
    /// Get [`Ray`] translated and rotated by given [`Isometry`]
    #[must_use]
    pub fn transformed_ray(&self, isometry: &Isometry) -> Self {
        let transformed_position = isometry.transform_point(&self.pos);
        let transformed_dir = isometry.transform_vector_f64(&self.dir);
        let mut new_ray = self.clone();
        new_ray.pos = transformed_position;
        new_ray.dir = transformed_dir;
        new_ray
    }
    /// Get [`Ray`] inverse translated and rotated by given [`Isometry`]
    #[must_use]
    pub fn inverse_transformed_ray(&self, isometry: &Isometry) -> Self {
        let transformed_position = isometry.inverse_transform_point(&self.pos);
        let transformed_dir = isometry.inverse_transform_vector_f64(&self.dir);
        let mut new_ray = self.clone();
        new_ray.pos = transformed_position;
        new_ray.dir = transformed_dir;
        new_ray
    }
    /// Returns the number of bounces of this [`Ray`].
    #[must_use]
    pub const fn number_of_bounces(&self) -> usize {
        self.number_of_bounces
    }
    /// Returns the number of refractions of this [`Ray`].
    #[must_use]
    pub const fn number_of_refractions(&self) -> usize {
        self.number_of_refractions
    }
}
impl Display for Ray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = Length::format_args(meter, uom::fmt::DisplayStyle::Abbreviation);
        let nm = Length::format_args(nanometer, uom::fmt::DisplayStyle::Abbreviation);
        let e = Energy::format_args(joule, uom::fmt::DisplayStyle::Abbreviation);
        write!(
            f,
            "pos: ({}, {}, {}), dir: ({}, {}, {}), energy: {:.6}, wavelength: {:.4}, valid: {}",
            m.with(self.pos[0]),
            m.with(self.pos[1]),
            m.with(self.pos[2]),
            self.dir[0],
            self.dir[1],
            self.dir[2],
            e.with(self.e),
            nm.with(self.wavelength()),
            self.valid
        )
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        coatings::CoatingType,
        degree, joule, millimeter, nanometer,
        spectrum_helper::{self, generate_filter_spectrum},
        surface::Plane,
    };
    use approx::{abs_diff_eq, assert_abs_diff_eq, assert_relative_eq};
    use itertools::izip;
    use std::path::PathBuf;
    use uom::si::{energy::joule, length::millimeter};
    #[test]
    fn new() {
        let pos = millimeter!(1.0, 2.0, 3.0);
        let dir = vector![0.0, 0.0, 2.0];
        let e = joule!(1.0);
        let wvl = nanometer!(1053.0);
        let ray = Ray::new(pos, dir, wvl, e);
        assert!(ray.is_ok());
        let ray = ray.unwrap();
        assert_eq!(ray.pos, pos);
        assert_eq!(ray.position(), pos);
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.wvl, wvl);
        assert_eq!(ray.wavelength(), wvl);
        assert_eq!(ray.e, e);
        assert_eq!(ray.energy(), e);
        assert_eq!(ray.path_length, Length::zero());
        assert_eq!(ray.refractive_index, 1.0);
        assert_eq!(ray.pos_hist.len(), 0);
        assert_eq!(ray.valid, true);
        assert_eq!(ray.number_of_bounces, 0);
        assert_eq!(ray.number_of_refractions, 0);
        assert!(Ray::new(pos, dir, nanometer!(0.0), e).is_err());
        assert!(Ray::new(pos, dir, nanometer!(-10.0), e).is_err());
        assert!(Ray::new(pos, dir, nanometer!(f64::NAN), e).is_err());
        assert!(Ray::new(pos, dir, nanometer!(f64::INFINITY), e).is_err());
        assert!(Ray::new(pos, dir, nanometer!(f64::NEG_INFINITY), e).is_err());
        assert!(Ray::new(pos, dir, wvl, joule!(-0.1)).is_err());
        assert!(Ray::new(pos, dir, wvl, joule!(f64::NAN)).is_err());
        assert!(Ray::new(pos, dir, wvl, joule!(f64::INFINITY)).is_err());
        assert!(Ray::new(pos, Vector3::zero(), wvl, e).is_err());
    }
    #[test]
    fn new_collimated() {
        let pos = millimeter!(1.0, 2.0, 0.0);
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let ray = Ray::new_collimated(pos, wvl, e);
        assert!(ray.is_ok());
        let ray = ray.unwrap();
        assert_eq!(ray.pos, pos);
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.wvl, wvl);
        assert_eq!(ray.e, e);
        assert_eq!(ray.path_length, Length::zero());
        assert_eq!(ray.pos_hist.len(), 0);
        assert_eq!(ray.valid, true);
        assert!(Ray::new_collimated(pos, nanometer!(0.0), e).is_err());
        assert!(Ray::new_collimated(pos, nanometer!(-10.0), e).is_err());
        assert!(Ray::new_collimated(pos, nanometer!(f64::NAN), e).is_err());
        assert!(Ray::new_collimated(pos, nanometer!(f64::INFINITY), e).is_err());
        assert!(Ray::new_collimated(pos, nanometer!(f64::NEG_INFINITY), e).is_err());
        assert!(Ray::new_collimated(pos, wvl, joule!(0.0)).is_ok());
        assert!(Ray::new_collimated(pos, wvl, joule!(-0.1)).is_err());
        assert!(Ray::new_collimated(pos, wvl, joule!(f64::NAN)).is_err());
        assert!(Ray::new_collimated(pos, wvl, joule!(f64::INFINITY)).is_err());
        assert!(Ray::new_collimated(pos, wvl, joule!(f64::NEG_INFINITY)).is_err());
    }
    #[test]
    fn valid() {
        let pos = millimeter!(1.0, 2.0, 0.0);
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        assert_eq!(ray.valid(), true);
        ray.valid = false;
        assert_eq!(ray.valid(), false);
    }
    #[test]
    fn set_valid() {
        let pos = millimeter!(1.0, 2.0, 0.0);
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        ray.set_invalid();
        assert_eq!(ray.valid(), false);
    }
    #[test]
    fn refractive_index() {
        let wvl = nanometer!(1053.0);
        let energy = joule!(1.0);
        let mut ray = Ray::origin_along_z(wvl, energy).unwrap();
        ray.refractive_index = 2.0;
        assert_eq!(ray.refractive_index(), 2.0);
    }
    #[test]
    fn set_refractive_index() {
        let wvl = nanometer!(1053.0);
        let energy = joule!(1.0);
        let mut ray = Ray::origin_along_z(wvl, energy).unwrap();
        assert!(ray.set_refractive_index(f64::NAN).is_err());
        assert!(ray.set_refractive_index(f64::INFINITY).is_err());
        assert!(ray.set_refractive_index(0.99).is_err());
        assert!(ray.set_refractive_index(1.0).is_ok());
        assert!(ray.set_refractive_index(2.0).is_ok());
        assert_eq!(ray.refractive_index, 2.0);
    }
    #[test]
    fn set_direction() {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        assert!(ray.set_direction(Vector3::zero()).is_err());
        let new_dir = vector![0.0, 1.0, 0.0];
        ray.set_direction(new_dir).unwrap();
        assert_eq!(ray.direction(), new_dir);
    }
    #[test]
    fn display() {
        let ray = Ray::origin_along_z(nanometer!(1001.0), joule!(1.0)).unwrap();
        assert_eq!(
            format!("{}", ray),
            "pos: (0 m, 0 m, 0 m), dir: (0, 0, 1), energy: 1.000000 J, wavelength: 1001.0000 nm, valid: true"
        );
    }
    #[test]
    fn propagate() {
        let wvl = nanometer!(1053.0);
        let energy = joule!(1.0);
        let mut ray = Ray::origin_along_z(wvl, energy).unwrap();
        assert!(ray.propagate(millimeter!(f64::INFINITY)).is_err());
        assert!(ray.propagate(millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(ray.propagate(millimeter!(f64::NAN)).is_err());
        assert!(ray.propagate(millimeter!(1.0)).is_ok());
        assert_eq!(ray.pos_hist, vec![millimeter!(0., 0., 0.)]);
        ray.propagate(millimeter!(1.0)).unwrap();
        assert_eq!(
            ray.pos_hist,
            vec![millimeter!(0., 0., 0.), millimeter!(0., 0., 1.0)]
        );
        assert_eq!(ray.wavelength(), wvl);
        assert_eq!(ray.energy(), energy);
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 0);
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.position(), millimeter!(0., 0., 2.0));
        assert_eq!(ray.path_length(), millimeter!(2.0));
        ray.propagate(millimeter!(2.0)).unwrap();

        assert_eq!(ray.position(), millimeter!(0., 0., 4.0));
        assert_eq!(
            ray.pos_hist,
            vec![
                millimeter!(0., 0., 0.),
                millimeter!(0., 0., 1.0),
                millimeter!(0., 0., 2.0)
            ]
        );
        ray.propagate(millimeter!(-5.0)).unwrap();

        assert_eq!(ray.position(), millimeter!(0., 0., -1.0));
        assert_eq!(
            ray.pos_hist,
            vec![
                millimeter!(0., 0., 0.),
                millimeter!(0., 0., 1.0),
                millimeter!(0., 0., 2.0),
                millimeter!(0., 0., 4.0)
            ]
        );
        let mut ray =
            Ray::new(millimeter!(0., 0., 0.), vector![0.0, 1.0, 1.0], wvl, energy).unwrap();
        ray.propagate(millimeter!(1.0)).unwrap();
        assert_eq!(
            ray.position(),
            millimeter!(0., 1. / f64::sqrt(2.0), 1. / f64::sqrt(2.0))
        );
        ray.propagate(millimeter!(2.0)).unwrap();

        assert_eq!(
            ray.position(),
            millimeter!(0., 3. / f64::sqrt(2.0), 3. / f64::sqrt(2.0))
        );
    }
    #[test]
    fn propagate_with_refractive_index() {
        let wvl = nanometer!(1053.0);
        let energy = joule!(1.0);
        let mut ray = Ray::new(millimeter!(0., 0., 0.), Vector3::z(), wvl, energy).unwrap();
        ray.set_refractive_index(2.0).unwrap();
        ray.propagate(millimeter!(1.0)).unwrap();
        assert_eq!(ray.wavelength(), wvl);
        assert_eq!(ray.energy(), energy);
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 0);
        assert_eq!(ray.position(), millimeter!(0., 0., 1.));
        assert_eq!(ray.path_length(), millimeter!(2.0));
    }
    #[test]
    fn refract_paraxial_wrong_params() {
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let mut ray = Ray::new_collimated(millimeter!(0., 0., 0.), wvl, e).unwrap();
        assert!(ray.refract_paraxial(millimeter!(0.0)).is_err());
        assert!(ray.refract_paraxial(millimeter!(f64::NAN)).is_err());
        assert!(ray.refract_paraxial(millimeter!(f64::INFINITY)).is_err());
        assert!(ray
            .refract_paraxial(millimeter!(f64::NEG_INFINITY))
            .is_err());
    }
    #[test]
    fn refract_paraxial_on_axis() {
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let pos: Point3<Length> = Point3::origin();
        let ray = Ray::new_collimated(pos, wvl, e).unwrap();
        let ray_dir = ray.dir;
        let mut refracted_ray = ray.clone();
        refracted_ray.refract_paraxial(millimeter!(100.0)).unwrap();
        assert_eq!(refracted_ray.pos, pos);
        assert_eq!(refracted_ray.dir, ray.dir);
        assert_eq!(refracted_ray.e, e);
        assert_eq!(refracted_ray.wvl, wvl);
        assert_eq!(refracted_ray.number_of_bounces(), ray.number_of_bounces());
        assert_eq!(
            refracted_ray.number_of_refractions(),
            ray.number_of_refractions() + 1
        );
        assert_eq!(refracted_ray.path_length, Length::zero());

        let mut refracted_ray = ray.clone();
        refracted_ray.refract_paraxial(millimeter!(-100.0)).unwrap();
        assert_eq!(refracted_ray.pos, pos);
        assert_eq!(refracted_ray.dir, ray_dir);
        assert_eq!(refracted_ray.e, e);
        assert_eq!(refracted_ray.wvl, wvl);
        assert_eq!(refracted_ray.number_of_bounces(), ray.number_of_bounces());
        assert_eq!(
            refracted_ray.number_of_refractions(),
            ray.number_of_refractions() + 1
        );
        assert_eq!(refracted_ray.path_length, Length::zero());
    }
    #[test]
    fn refract_paraxial_collimated() {
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let pos = millimeter!(1., 2., 0.);

        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        ray.refract_paraxial(millimeter!(100.0)).unwrap();
        assert_eq!(ray.pos, pos);
        let test_ray_dir = vector![-1.0, -2.0, 100.0] / 100.0;
        assert_abs_diff_eq!(ray.dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray.dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray.dir.z, test_ray_dir.z);

        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        ray.refract_paraxial(millimeter!(-100.0)).unwrap();
        assert_eq!(ray.pos, pos);
        let test_ray_dir = vector![1.0, 2.0, 100.0] / 100.0;
        assert_abs_diff_eq!(ray.dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray.dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray.dir.z, test_ray_dir.z);

        let pos = millimeter!(0., 10., 0.);
        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        ray.refract_paraxial(millimeter!(10.0)).unwrap();
        assert_abs_diff_eq!(
            ray.path_length.get::<millimeter>(),
            -1.0 * (f64::sqrt(200.0) - 10.0),
            epsilon = 10.0 * f64::EPSILON
        );
        let pos = millimeter!(0., 100., 0.);
        let mut ray = Ray::new_collimated(pos, wvl, e).unwrap();
        ray.refract_paraxial(millimeter!(100.0)).unwrap();
        assert_eq!(ray.pos, pos);
        let test_ray_dir = vector![0.0, -100.0, 100.0] / 100.0;
        assert_abs_diff_eq!(ray.dir, test_ray_dir);
    }
    #[test]
    fn refract_paraxial_recollimate() {
        let wvl = nanometer!(1053.0);
        let e = joule!(1.0);
        let pos = millimeter!(0., 100., 100.);
        let dir = vector![0.0, 1.0, 1.0];
        let mut ray = Ray::new(pos, dir, wvl, e).unwrap();

        ray.refract_paraxial(millimeter!(100.0)).unwrap();
        assert_eq!(ray.pos, pos);
        assert_eq!(ray.dir, Vector3::z());

        let dir = vector![0.0, -1.0, 1.0];
        let mut ray = Ray::new(pos, dir, wvl, e).unwrap();
        ray.refract_paraxial(millimeter!(-100.0)).unwrap();
        assert_eq!(ray.pos, pos);
        assert_eq!(ray.dir, Vector3::z());
    }
    #[test]
    fn refract_on_surface_collimated() {
        let position = Point3::origin();
        let wvl = nanometer!(1054.0);
        let e = joule!(1.0);
        let reflectivity = 0.2;
        let mut ray = Ray::new_collimated(position, wvl, e).unwrap();
        let plane_z_pos = millimeter!(10.0);
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), plane_z_pos),
            degree!(0.0, 0.0, 0.0),
        )
        .unwrap();
        let mut s = OpticalSurface::new(Box::new(Plane::new(&isometry)));
        s.set_coating(CoatingType::ConstantR { reflectivity });
        assert!(ray.refract_on_surface(&s, 0.9).is_err());
        assert!(ray.refract_on_surface(&s, f64::NAN).is_err());
        assert!(ray.refract_on_surface(&s, f64::INFINITY).is_err());
        let reflected_ray = ray.refract_on_surface(&s, 1.5).unwrap().unwrap();

        // refracted ray
        assert_eq!(ray.pos, millimeter!(0., 0., 10.));
        assert_eq!(ray.refractive_index, 1.5);
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.pos_hist, vec![Point3::origin()]);
        assert_eq!(ray.path_length(), plane_z_pos);
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 1);
        assert_eq!(ray.energy(), (1. - reflectivity) * e);

        // reflected ray
        assert_eq!(reflected_ray.pos, millimeter!(0., 0., 10.));
        assert_eq!(reflected_ray.refractive_index, 1.0);
        assert_eq!(reflected_ray.dir, -1.0 * Vector3::z());
        assert_eq!(reflected_ray.pos_hist, vec![Point3::origin()]);
        assert_eq!(reflected_ray.path_length(), plane_z_pos);
        assert_eq!(reflected_ray.number_of_bounces(), 1);
        assert_eq!(reflected_ray.number_of_refractions(), 0);
        assert_eq!(reflected_ray.energy(), reflectivity * e);

        let position = millimeter!(0., 1., 0.);
        let mut ray = Ray::new_collimated(position, wvl, e).unwrap();
        ray.refract_on_surface(&s, 1.5).unwrap();
        assert_eq!(ray.pos, millimeter!(0., 1., 10.));
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.path_length, plane_z_pos);
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 1);
    }
    #[test]
    fn refract_on_surface_non_intersecting() {
        let position = millimeter!(0., 0., 0.);
        let direction = vector![0.0, 0.0, -1.0];
        let wvl = nanometer!(1054.0);
        let e = joule!(1.0);
        let mut ray = Ray::new(position, direction, wvl, e).unwrap();
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), millimeter!(10.0)),
            degree!(0.0, 0.0, 0.0),
        )
        .unwrap();
        let s = OpticalSurface::new(Box::new(Plane::new(&isometry)));
        ray.refract_on_surface(&s, 1.5).unwrap();
        assert_eq!(ray.pos, millimeter!(0., 0., 0.));
        assert_eq!(ray.dir, direction);
        assert_eq!(ray.refractive_index, 1.0);
        assert_eq!(ray.path_length, Length::zero());
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 0);
    }
    #[test]
    fn refract_on_surface_non_collimated() {
        let position = Point3::origin();
        let direction = vector![0.0, 1.0, 1.0];
        let wvl = nanometer!(1054.0);
        let e = joule!(1.0);
        let mut ray = Ray::new(position, direction, wvl, e).unwrap();
        let plane_z_pos = millimeter!(10.0);
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), plane_z_pos),
            degree!(0.0, 0.0, 0.0),
        )
        .unwrap();
        let s = OpticalSurface::new(Box::new(Plane::new(&isometry)));
        assert!(ray.refract_on_surface(&s, 0.9).is_err());
        assert!(ray.refract_on_surface(&s, f64::NAN).is_err());
        assert!(ray.refract_on_surface(&s, f64::INFINITY).is_err());
        ray.refract_on_surface(&s, 1.0).unwrap();
        assert_eq!(ray.pos, millimeter!(0., 10., 10.));
        assert_eq!(ray.dir[0], 0.0);
        assert_abs_diff_eq!(ray.dir[1], direction.normalize()[1]);
        assert_abs_diff_eq!(ray.dir[2], direction.normalize()[2]);
        assert_abs_diff_eq!(ray.path_length.value, 2.0_f64.sqrt() * plane_z_pos.value);
        let mut ray = Ray::new(position, direction, wvl, e).unwrap();
        ray.refract_on_surface(&s, 1.5).unwrap();
        assert_eq!(ray.number_of_bounces(), 0);
        assert_eq!(ray.number_of_refractions(), 1);
        assert_eq!(ray.pos, millimeter!(0., 10., 10.));
        assert_eq!(ray.dir[0], 0.0);
        assert_abs_diff_eq!(ray.dir[1], 0.4714045207910317);
        assert_abs_diff_eq!(ray.dir[2], 0.8819171036881969);
        let direction = vector![1.0, 0.0, 1.0];
        let mut ray = Ray::new(position, direction, wvl, e).unwrap();
        ray.refract_on_surface(&s, 1.5).unwrap();
        assert_eq!(ray.pos, millimeter!(10., 0., 10.));
        assert_eq!(ray.dir[0], 0.4714045207910317);
        assert_abs_diff_eq!(ray.dir[1], 0.0);
        assert_abs_diff_eq!(ray.dir[2], 0.8819171036881969);
    }
    #[test]
    fn refract_on_surface_total_reflection() {
        let position = millimeter!(0., 0., 0.);
        let direction = vector![0.0, 2.0, 1.0];
        let wvl = nanometer!(1054.0);
        let e = joule!(1.0);
        let mut ray = Ray::new(position, direction, wvl, e).unwrap();
        ray.set_refractive_index(1.5).unwrap();
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), millimeter!(10.0)),
            degree!(0.0, 0.0, 0.0),
        )
        .unwrap();
        let s = OpticalSurface::new(Box::new(Plane::new(&isometry)));
        let reflected = ray.refract_on_surface(&s, 1.0).unwrap();
        assert!(reflected.is_none());
        assert_eq!(ray.pos, millimeter!(0., 20., 10.));
        let test_reflect = vector![0.0, 2.0, -1.0].normalize();
        assert_abs_diff_eq!(ray.dir[0], test_reflect[0]);
        assert_abs_diff_eq!(ray.dir[1], test_reflect[1]);
        assert_abs_diff_eq!(ray.dir[2], test_reflect[2]);
    }
    #[test]
    fn filter_energy() {
        let position = millimeter!(0., 1., 0.);
        let wvl = nanometer!(1054.0);
        let mut ray = Ray::new_collimated(position, wvl, joule!(1.0)).unwrap();
        let _ = ray.filter_energy(&FilterType::Constant(0.3)).unwrap();
        assert_eq!(ray.pos, millimeter!(0., 1., 0.));
        assert_eq!(ray.dir, Vector3::z());
        assert_eq!(ray.wvl, wvl);
        assert_eq!(ray.e, joule!(0.3));
        let mut ray = Ray::new_collimated(position, wvl, joule!(1.0)).unwrap();
        assert!(ray.filter_energy(&FilterType::Constant(-0.1)).is_err());
        let mut ray = Ray::new_collimated(position, wvl, joule!(1.0)).unwrap();
        assert!(ray.filter_energy(&FilterType::Constant(1.1)).is_err());
    }
    #[test]
    fn filter_spectrum() {
        let position = millimeter!(0., 1., 0.);
        let e_1j = joule!(1.0);
        let mut ray = Ray::new_collimated(position, nanometer!(502.0), e_1j).unwrap();
        let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        spec_path.push("files_for_testing/spectrum/test_filter.csv");
        let s = Spectrum::from_csv(spec_path.to_str().unwrap()).unwrap();
        let filter = FilterType::Spectrum(s);
        let _ = ray.filter_energy(&filter).unwrap();
        assert_eq!(ray.e, e_1j);
        assert_eq!(ray.pos, ray.pos);
        assert_eq!(ray.dir, ray.dir);
        assert_eq!(ray.wvl, ray.wvl);
        assert_eq!(ray.path_length, ray.path_length);
        assert_eq!(ray.pos_hist, ray.pos_hist);
        let mut ray = Ray::new_collimated(position, nanometer!(500.0), e_1j).unwrap();
        let _ = ray.filter_energy(&filter).unwrap();
        assert_eq!(ray.energy(), joule!(0.0));
        let mut ray = Ray::new_collimated(position, nanometer!(501.5), e_1j).unwrap();
        let _ = ray.filter_energy(&filter).unwrap();
        assert!(abs_diff_eq!(
            ray.energy().get::<joule>(),
            0.5,
            epsilon = 300.0 * f64::EPSILON
        ));
        let mut ray = Ray::new_collimated(position, nanometer!(506.0), e_1j).unwrap();
        assert!(ray.filter_energy(&filter).is_err());
    }
    #[test]
    fn split_by_ratio() {
        let mut ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1054.0), joule!(1.0)).unwrap();
        assert!(ray.split(&SplittingConfig::Ratio(1.1)).is_err());
        assert!(ray.split(&SplittingConfig::Ratio(-0.1)).is_err());
        let split_ray = ray.split(&SplittingConfig::Ratio(0.1)).unwrap();
        assert_eq!(ray.energy(), joule!(0.1));
        assert_eq!(split_ray.energy(), joule!(0.9));
        assert_eq!(ray.position(), split_ray.position());
        assert_eq!(ray.dir, split_ray.dir);
        assert_eq!(ray.wavelength(), split_ray.wavelength());
    }
    #[test]
    fn split_by_spectrum() {
        let mut ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1000.0), joule!(1.0)).unwrap();
        let spectrum = generate_filter_spectrum(
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
            &spectrum_helper::FilterType::ShortPassStep {
                cut_off: nanometer!(1000.0),
            },
        )
        .unwrap();
        let splitting_config = SplittingConfig::Spectrum(spectrum);
        let split_ray = ray.split(&splitting_config).unwrap();
        assert_eq!(ray.energy(), joule!(0.0));
        assert_eq!(split_ray.energy(), joule!(1.0));
        let mut ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1001.0), joule!(1.0)).unwrap();
        let split_ray = ray.split(&splitting_config).unwrap();
        assert_eq!(ray.energy(), Energy::zero());
        assert_eq!(split_ray.energy(), joule!(1.0));
        let mut ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(999.0), joule!(1.0)).unwrap();
        let split_ray = ray.split(&&splitting_config).unwrap();
        assert_eq!(ray.energy(), joule!(1.0));
        assert_eq!(split_ray.energy(), Energy::zero());
    }
    #[test]
    fn split_by_spectrum_fail() {
        let mut ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1501.0), joule!(1.0)).unwrap();
        let spectrum = generate_filter_spectrum(
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
            &spectrum_helper::FilterType::ShortPassStep {
                cut_off: nanometer!(1000.0),
            },
        )
        .unwrap();
        assert!(ray.split(&SplittingConfig::Spectrum(spectrum)).is_err());
    }
    #[test]
    fn position_history() {
        let dir = vector![0., 1., 2.];
        let mut ray =
            Ray::new(millimeter!(0., 0., 0.), dir, nanometer!(1053.), joule!(1.)).unwrap();
        ray.propagate(millimeter!(1.)).unwrap();
        ray.propagate(millimeter!(2.)).unwrap();
        let norm_dir = dir.normalize();
        let pos_hist_comp = MatrixXx3::from_vec(
            vec![
                0.,
                0.,
                0.,
                0.,
                1. * norm_dir[1],
                3. * norm_dir[1],
                0.0,
                1. * norm_dir[2],
                3. * norm_dir[2],
            ]
            .iter()
            .map(|x| millimeter!(*x))
            .collect::<Vec<Length>>(),
        );
        let pos_hist = ray.position_history();
        for (row, row_calc) in izip!(pos_hist_comp.row_iter(), pos_hist.row_iter()) {
            assert_abs_diff_eq!(row[0].value, row_calc[0].value);
            assert_abs_diff_eq!(row[1].value, row_calc[1].value);
            assert_abs_diff_eq!(row[2].value, row_calc[2].value);
        }
    }
    #[test]
    fn transformed_ray_trans() {
        let ray = Ray::origin_along_z(nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new_along_z(meter!(1.0)).unwrap();
        let new_ray = ray.transformed_ray(&iso);
        assert_eq!(new_ray.pos, meter!(0.0, 0.0, 1.0));
        assert_eq!(new_ray.dir, ray.dir);
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
    #[test]
    fn transformed_ray_rot() {
        let ray = Ray::origin_along_z(nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new(meter!(0.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0)).unwrap();
        let new_ray = ray.transformed_ray(&iso);
        assert_eq!(new_ray.pos, ray.pos);
        assert_relative_eq!(new_ray.dir, vector![1.0, 0.0, 0.0]);
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
    #[test]
    fn transformed_ray_rot_trans() {
        let ray = Ray::new_collimated(meter!(1., 1., 1.), nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new(meter!(0.0, 1.0, 0.0), degree!(0.0, 0.0, 90.0)).unwrap();
        let new_ray = ray.transformed_ray(&iso);
        assert_relative_eq!(new_ray.pos.x.value, -1.0, epsilon = 2.0 * f64::EPSILON);
        assert_relative_eq!(new_ray.pos.y.value, 2.0);
        assert_relative_eq!(new_ray.pos.z.value, 1.0);
        assert_relative_eq!(new_ray.dir, Vector3::z());
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
    #[test]
    fn inversetransformed_ray_trans() {
        let ray = Ray::origin_along_z(nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new_along_z(meter!(1.0)).unwrap();
        let new_ray = ray.inverse_transformed_ray(&iso);
        assert_eq!(new_ray.pos, meter!(0.0, 0.0, -1.0));
        assert_eq!(new_ray.dir, ray.dir);
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
    #[test]
    fn inverse_transformed_ray_rot() {
        let ray = Ray::origin_along_z(nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new(meter!(0.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0)).unwrap();
        let new_ray = ray.inverse_transformed_ray(&iso);
        assert_eq!(new_ray.pos, ray.pos);
        assert_relative_eq!(new_ray.dir, vector![-1.0, 0.0, 0.0]);
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
    #[test]
    fn inverse_transformed_ray_rot_trans() {
        let ray = Ray::new_collimated(meter!(-1., 2., 1.), nanometer!(1053.), joule!(1.)).unwrap();
        let iso = Isometry::new(meter!(0.0, 1.0, 0.0), degree!(0.0, 0.0, 90.0)).unwrap();
        let new_ray = ray.inverse_transformed_ray(&iso);
        assert_relative_eq!(new_ray.pos.x.value, 1.0, epsilon = 5.0 * f64::EPSILON);
        assert_relative_eq!(new_ray.pos.y.value, 1.0);
        assert_relative_eq!(new_ray.pos.z.value, 1.0);
        assert_relative_eq!(new_ray.dir, vector![0.0, 0.0, 1.0]);
        assert_eq!(new_ray.wvl, ray.wvl);
        assert_eq!(new_ray.e, ray.e);
    }
}
