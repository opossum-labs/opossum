#![warn(missing_docs)]
//! Module for handling rays
use std::ops::Range;

use crate::aperture::Aperture;
use crate::error::{OpmResult, OpossumError};
use crate::nodes::FilterType;
use crate::plottable::Plottable;
use crate::properties::Proptype;
use crate::reporter::PdfReportable;
use crate::spectrum::Spectrum;
use image::DynamicImage;
use kahan::KahanSummator;
use nalgebra::{distance, point, Point2, Point3, Vector3};
use plotters::prelude::{ChartBuilder, Circle, EmptyElement};
use plotters::series::PointSeries;
use plotters::style::{IntoFont, TextStyle, RED};
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use sobol::{params::JoeKuoD6, Sobol};
use uom::num_traits::Zero;
use uom::si::angle::degree;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::millimeter;

///Struct that contains all information about an optical ray
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Ray {
    ///Stores all positions of the ray (in mm)
    pos: Point3<f64>, // this should be a vector of points?
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
        position: Point2<Length>,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        Self::new(position, Vector3::z(), wave_length, energy)
    }
    /// Creates a new [`Ray`].
    ///
    /// The dircetion vector is normalized. The direction is thus stored aa (`direction cosine`)[`https://en.wikipedia.org/wiki/Direction_cosine`]
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given wavelength is <= 0.0, `NaN` or +inf
    ///  - the given energy is < 0.0, `NaN` or +inf
    ///  - the direction vector has a zero length
    pub fn new(
        position: Point2<Length>,
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
            pos: Point3::new(
                position.x.get::<millimeter>(),
                position.y.get::<millimeter>(),
                0.0,
            ),
            dir: direction.normalize(),
            //pol: Vector2::new(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)), // horizontal polarization
            e: energy,
            wvl: wave_length,
            //id: 0,
            //bounce: 0,
            path_length: Length::zero(),
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
    /// freely propagate a ray along its direction. The length is given as the projection on the z-axis (=optical axis).
    ///
    /// # Errors
    /// This functions retruns an error if the initial ray direction has a zero z component (= ray not propagating in z direction).
    pub fn propagate_along_z(&self, length_along_z: Length) -> OpmResult<Self> {
        if self.dir[2].abs() < f64::EPSILON {
            return Err(OpossumError::Other(
                "z-Axis of direction vector must be != 0.0".into(),
            ));
        }
        let mut new_ray = self.clone();
        let length_in_ray_dir = length_along_z.get::<millimeter>() / self.dir[2];
        new_ray.pos = self.pos + length_in_ray_dir * self.dir;

        let normalized_dir = self.dir.normalize();
        let length_in_ray_dir = length_along_z.get::<millimeter>() / normalized_dir[2];
        new_ray.path_length += Length::new::<millimeter>(length_in_ray_dir);
        Ok(new_ray)
    }
    /// Refract a ray on a paraxial surface of a given focal length.
    ///
    /// Modify the ray direction
    /// # Errors
    ///
    /// This function will return an error if the given focal length is zero or not finite
    pub fn refract_paraxial(&self, focal_length: Length) -> OpmResult<Self> {
        if focal_length.is_zero() || !focal_length.is_finite() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 & finite".into(),
            ));
        }
        let optical_power = 1.0 / focal_length.get::<millimeter>();
        let mut new_ray = self.clone();
        new_ray.dir.x = optical_power.mul_add(-self.pos.x, self.dir.x);
        new_ray.dir.y = optical_power.mul_add(-self.pos.y, self.dir.y);
        new_ray.dir.z = 1.0;
        // *** no longer normalized ***
        // new_ray.dir.normalize_mut();
        // *** removed since it introduced severe rounding errors ***
        Ok(new_ray)
    }

    /// Attenuate a ray's energy by a given filter.
    ///
    /// This function attenuates the ray's energy by the given [`FilterType`]. For [`FilterType::Constant`] the energy is simply multiplied with the
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
    /// This function modifies the energy of the existing ray and generates a new split ray. The splitting ratio must be within the range
    /// `(0.0..=1.0)`. A ratio of 1.0 means that all energy remains in the initial beam and the split beam has an energy of zero. A ratio of 0.0
    /// corresponds to a fully reflected beam.
    ///
    /// # Errors
    ///
    /// This function will return an error if `splitting_ratio` is outside the interval [0.0..1.0].
    pub fn split(&mut self, splitting_ratio: f64) -> OpmResult<Self> {
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
}
/// Struct containing all relevant information of a ray bundle
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rays {
    ///vector containing rays
    rays: Vec<Ray>,
    //Maximum number of bounces
    //max_bounces:    usize, do we need this here?
}
impl Rays {
    /// Generate a set of collimated rays (collinear with optical axis).
    ///
    /// This functions generates a bundle of (collimated) rays of the given wavelength and the given *total* energy. The energy is
    /// evenly distributed over the indivual rays. The ray positions are distributed according to the given [`DistributionStrategy`].
    ///
    /// If the given size id zero, a bundle consisting of a single ray along the optical - position (0.0,0.0,0.0) - axis is generated.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given wavelength is <= 0.0, NaN or +inf
    ///  - the given energy is <= 0.0, NaN or +inf
    ///  - the given size is < 0.0, NaN or +inf
    pub fn new_uniform_collimated(
        size: Length,
        wave_length: Length,
        energy: Energy,
        strategy: &DistributionStrategy,
    ) -> OpmResult<Self> {
        if size.is_sign_negative() || !size.is_finite() {
            return Err(OpossumError::Other(
                "radius must be >= 0.0 and finite".into(),
            ));
        }
        let points: Vec<Point2<Length>> = if size.is_zero() {
            vec![Point2::new(Length::zero(), Length::zero())]
        } else {
            strategy.generate(size)
        };
        let nr_of_rays = points.len();
        let mut rays: Vec<Ray> = Vec::new();
        #[allow(clippy::cast_precision_loss)]
        let energy_per_ray = energy / nr_of_rays as f64;
        for point in points {
            let ray = Ray::new_collimated(point, wave_length, energy_per_ray)?;
            rays.push(ray);
        }
        Ok(Self { rays })
    }
    /// Generate a ray cone (= point source)
    ///
    /// Generate a bundle of rays emerging from a given (x,y) point and a cone direction (as hexapolar pattern) of a given (full) cone angle.
    /// The parameter `number_of_rings` determines the "density" of the hexapolar pattern (see corresponding function). If the cone angle is zero, a ray bundle
    /// with a single ray along the optical axis at the given position is created.
    ///
    /// # Errors
    /// This functions returns an error if
    ///  - the given wavelength is <= 0.0, nan, or +inf
    ///  - the given energy is < 0.0, nan, or +inf
    ///  - the given cone angle is < 0.0 degrees or >= 180.0 degrees
    pub fn new_hexapolar_point_source(
        position: Point2<Length>,
        cone_angle: Angle,
        number_of_rings: u8,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        if cone_angle.is_sign_negative() || cone_angle >= Angle::new::<degree>(180.0) {
            return Err(OpossumError::Other(
                "cone angle must be within (0.0..180.0) degrees range".into(),
            ));
        }
        let size_after_unit_length = (cone_angle / 2.0).tan().value;
        let points: Vec<Point2<Length>> = if cone_angle.is_zero() {
            vec![Point2::new(Length::zero(), Length::zero())]
        } else {
            DistributionStrategy::Hexapolar(number_of_rings)
                .generate(Length::new::<millimeter>(size_after_unit_length))
        };
        let nr_of_rays = points.len();
        #[allow(clippy::cast_precision_loss)]
        let energy_per_ray = energy / nr_of_rays as f64;
        let mut rays: Vec<Ray> = Vec::new();
        for point in points {
            let direction = Vector3::new(
                point.x.get::<millimeter>(),
                point.y.get::<millimeter>(),
                1.0,
            );
            let ray = Ray::new(position, direction, wave_length, energy_per_ray)?;
            rays.push(ray);
        }
        Ok(Self { rays })
    }
    /// Returns the total energy of this [`Rays`].
    ///
    /// This simply sums up all energies of the individual rays.
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        let energies: Vec<f64> = self.rays.iter().map(|r| r.e.get::<joule>()).collect();
        let kahan_sum: kahan::KahanSum<f64> = energies.iter().kahan_sum();
        Energy::new::<joule>(kahan_sum.sum())
    }
    /// Returns the number of rays of this [`Rays`].
    #[must_use]
    pub fn nr_of_rays(&self) -> usize {
        self.rays.len()
    }
    /// Apodize (cut out or attenuate) the ray bundle by a given [`Aperture`].
    pub fn apodize(&mut self, aperture: &Aperture) {
        let mut new_rays: Vec<Ray> = Vec::new();
        for ray in &self.rays {
            let pos = point![ray.pos.x, ray.pos.y];
            let ap_factor = aperture.apodization_factor(&pos);
            if ap_factor > 0.0 {
                let mut new_ray = ray.clone();
                new_ray.e *= ap_factor;
                new_rays.push(new_ray);
            }
        }
        self.rays = new_rays;
    }
    /// Returns the centroid of this [`Rays`].
    ///
    /// This functions returns the centroid of the positions of this ray bundle. The function returns `None` if [`Rays`] is empty.
    #[must_use]
    pub fn centroid(&self) -> Option<Point3<Length>> {
        #[allow(clippy::cast_precision_loss)]
        let len = self.rays.len() as f64;
        if len == 0.0 {
            return None;
        }
        let c = self.rays.iter().fold((0.0, 0.0, 0.0), |c, r| {
            (c.0 + r.pos.x, c.1 + r.pos.y, c.2 + r.pos.z)
        });
        Some(Point3::new(
            Length::new::<millimeter>(c.0 / len),
            Length::new::<millimeter>(c.1 / len),
            Length::new::<millimeter>(c.2 / len),
        ))
    }
    /// Returns the geometric beam radius [`Rays`].
    ///
    /// This function calculates the maximum distance of a ray bundle from it centroid.
    #[must_use]
    pub fn beam_radius_geo(&self) -> Option<Length> {
        self.centroid().map(|c| {
            let c_in_millimeter = Point2::new(c.x.get::<millimeter>(), c.y.get::<millimeter>());
            let mut max_dist = 0.0;
            for ray in &self.rays {
                let ray_2d = Point2::new(ray.pos.x, ray.pos.y);
                let dist = distance(&ray_2d, &c_in_millimeter);
                if dist > max_dist {
                    max_dist = dist;
                }
            }
            Length::new::<millimeter>(max_dist)
        })
    }
    /// Returns the rms beam radius [`Rays`].
    ///
    /// This function calculates the rms (root mean square) size of a ray bundle from it centroid. So far, the rays / spots are not weighted by their
    /// particular energy.
    #[must_use]
    pub fn beam_radius_rms(&self) -> Option<Length> {
        self.centroid().map(|c| {
            let c_in_millimeter = Point2::new(c.x.get::<millimeter>(), c.y.get::<millimeter>());
            let mut sum_dist_sq = 0.0;
            for ray in &self.rays {
                let ray_2d = Point2::new(ray.pos.x, ray.pos.y);
                sum_dist_sq += distance(&ray_2d, &c_in_millimeter).powi(2);
            }
            #[allow(clippy::cast_precision_loss)]
            let nr_of_rays = self.rays.len() as f64;
            sum_dist_sq /= nr_of_rays;
            Length::new::<millimeter>(sum_dist_sq.sqrt())
        })
    }
    /// Add a single ray to the ray bundle.
    ///
    /// # Panics
    /// Panics if the resulting ray bundle exceeds `isize::MAX` bytes.
    pub fn add_ray(&mut self, ray: Ray) {
        self.rays.push(ray);
    }
    /// Add (merge) another ray bundle
    ///
    /// # Panics
    /// Panics if the resulting ray bundle exceeds `isize::MAX` bytes.
    pub fn add_rays(&mut self, rays: &mut Rays) {
        self.rays.append(&mut rays.rays)
    }
    /// Propagate a ray bundle along the z axis.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the z component of a ray direction is zero.
    ///  - the given length is not finite.
    pub fn propagate_along_z(&mut self, length_along_z: Length) -> OpmResult<()> {
        for ray in &mut self.rays {
            *ray = ray.propagate_along_z(length_along_z)?;
        }
        Ok(())
    }
    /// Refract a ray bundle on a paraxial surface of given focal length.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the z component of a ray direction is zero.
    ///  - the focal length is zero or not finite.
    pub fn refract_paraxial(&mut self, focal_length: Length) -> OpmResult<()> {
        if focal_length.is_zero() || !focal_length.is_finite() {
            return Err(OpossumError::Other(
                "focal length must be !=0.0 and finite".into(),
            ));
        }
        for ray in &mut self.rays {
            *ray = ray.refract_paraxial(focal_length)?;
        }
        Ok(())
    }
    /// Filter a ray bundle by a given filter.
    ///
    /// Filter the energy of of the rays by a given [`FilterType`].
    /// # Errors
    ///
    /// This function will return an error if the transmission factor for the [`FilterType::Constant`] is not within the range `(0.0..=1.0)`.
    pub fn filter_energy(&mut self, filter: &FilterType) -> OpmResult<()> {
        if let FilterType::Constant(t) = filter {
            if !(0.0..=1.0).contains(t) {
                return Err(OpossumError::Other(
                    "transmission value must be in the range [0.0;1.0]".into(),
                ));
            }
        }
        for ray in &mut self.rays {
            *ray = ray.filter_energy(filter)?;
        }
        Ok(())
    }
    /// Returns the wavelength range of this [`Rays`].
    ///
    /// This functions returns the minimum and maximum wavelength of the containing rays as `Range`. If [`Rays`] is empty, `None` is returned.
    #[must_use]
    pub fn wavelength_range(&self) -> Option<Range<Length>> {
        if self.rays.is_empty() {
            return None;
        };
        let mut min = Length::new::<millimeter>(f64::INFINITY);
        let mut max = Length::zero();
        for ray in &self.rays {
            let w = ray.wavelength();
            if w > max {
                max = w;
            }
            if w < min {
                min = w;
            }
        }
        Some(min..max)
    }
    /// Create a [`Spectrum`] (with a given resolution) from a ray bundle.
    ///
    /// This functions creates a spectrum by adding all individual rays from ray bundle with respect to their particular wavelength and energy.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - [`Rays`] is empty
    ///   - the `resolution` is invalid (negative, infinite)
    pub fn to_spectrum(&self, resolution: &Length) -> OpmResult<Spectrum> {
        let mut range = self
            .wavelength_range()
            .ok_or_else(|| OpossumError::Other("from_rays: rays seems to be empty".into()))?;
        range.end += *resolution * 2.0; // add 2* resolution to be sure to have all rays included in the wavelength range...
        let mut spectrum = Spectrum::new(range, *resolution)?;
        for ray in &self.rays {
            spectrum.add_single_peak(ray.wavelength(), ray.energy().get::<joule>())?;
        }
        Ok(spectrum)
    }
    /// Split a ray bundle by a given splitting ratio.
    ///
    /// This function splits a ray bundle by a given energy splitting ratio. It modifies the energies of all containing ray and generates a new
    /// bundle with the split rays. The splitting ratio must be in the interval [0.0; 1.0].
    /// A splitting ratio of 1.0 means a fully transmitted beam. All split rays have an energy of zero. In contrast, a splitting ratio of 1.0 means a
    /// fully reflected beam. All energy goes into the split rays.
    /// # Errors
    ///
    /// This function will return an error if the splitting ratio is outside the interval [0.0; 1.0].
    pub fn split(&mut self, splitting_ratio: f64) -> OpmResult<Self> {
        if !(0.0..=1.0).contains(&splitting_ratio) {
            return Err(OpossumError::Other(
                "splitting_ratio must be within [0.0;1.0]".into(),
            ));
        }
        let mut split_rays = Self::default();
        for ray in &mut self.rays {
            let split_ray = ray.split(splitting_ratio)?;
            split_rays.add_ray(split_ray);
        }
        Ok(split_rays)
    }
    /// Merge two ray bundles.
    ///
    /// This function simply adds the given rays to the existing ray bundle.
    pub fn merge(&mut self, rays: &Self) {
        for ray in &rays.rays {
            self.add_ray(ray.clone());
        }
    }
}
/// Strategy for the creation of a 2D point set
pub enum DistributionStrategy {
    /// Circular, hexapolar distribution with a given number of rings within a given radius
    Hexapolar(u8),
    /// Square, random distribution with a given number of points within a given side length
    Random(usize),
    /// Square, low-discrepancy quasirandom distribution with a given number of points within a given side length
    Sobol(usize),
}
impl DistributionStrategy {
    /// Generate a vector of 2D points within a given size (which depends on the concrete strategy)
    #[must_use]
    pub fn generate(&self, size: Length) -> Vec<Point2<Length>> {
        match self {
            Self::Hexapolar(rings) => hexapolar(*rings, size),
            Self::Random(nr_of_rays) => random(*nr_of_rays, size),
            Self::Sobol(nr_of_rays) => sobol(*nr_of_rays, size),
        }
    }
}
fn hexapolar(rings: u8, radius: Length) -> Vec<Point2<Length>> {
    let mut points: Vec<Point2<Length>> = Vec::new();
    let radius_step = radius / f64::from(rings);
    points.push(point![Length::zero(), Length::zero()]);
    for ring in 0u8..rings {
        let radius = f64::from(ring + 1) * radius_step;
        let points_per_ring = 6 * (ring + 1);
        let angle_step = 2.0 * std::f64::consts::PI / f64::from(points_per_ring);
        for point_nr in 0u8..points_per_ring {
            let point = (f64::from(point_nr) * angle_step).sin_cos();
            points.push(point![radius * point.0, radius * point.1]);
        }
    }
    points
}
fn random(nr_of_rays: usize, side_length: Length) -> Vec<Point2<Length>> {
    let mut points: Vec<Point2<Length>> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..nr_of_rays {
        points.push(point![
            Length::new::<millimeter>(
                rng.gen_range(-side_length.get::<millimeter>()..side_length.get::<millimeter>())
            ),
            Length::new::<millimeter>(
                rng.gen_range(-side_length.get::<millimeter>()..side_length.get::<millimeter>())
            )
        ]);
    }
    points
}
fn sobol(nr_of_rays: usize, side_length: Length) -> Vec<Point2<Length>> {
    let side_length = side_length.get::<millimeter>();
    let mut points: Vec<Point2<Length>> = Vec::new();
    let params = JoeKuoD6::minimal();
    let seq = Sobol::<f64>::new(2, &params);
    let offset = side_length / 2.0;
    for point in seq.take(nr_of_rays) {
        points.push(point!(
            Length::new::<millimeter>(point[0] - offset),
            Length::new::<millimeter>(point[1] - offset)
        ));
    }
    points
}

impl From<Rays> for Proptype {
    fn from(value: Rays) -> Self {
        Self::Rays(value)
    }
}
impl PdfReportable for Rays {
    fn pdf_report(&self) -> crate::error::OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let img = self.to_img_buf_plot().unwrap();
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(img))
                .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
    }
}
impl Plottable for Rays {
    fn chart<B: plotters::prelude::DrawingBackend>(
        &self,
        root: &plotters::prelude::DrawingArea<B, plotters::coord::Shift>,
    ) -> crate::error::OpmResult<()> {
        let mut x_min = self
            .rays
            .iter()
            .map(|r| r.pos.x)
            .fold(f64::INFINITY, f64::min)
            * 1.1;
        if !x_min.is_finite() {
            x_min = -1.0;
        }
        let mut x_max = self
            .rays
            .iter()
            .map(|r| r.pos.x)
            .fold(f64::NEG_INFINITY, f64::max)
            * 1.1;
        if !x_max.is_finite() {
            x_max = 1.0;
        }
        if (x_max - x_min).abs() < 10.0 * f64::EPSILON {
            x_max = 1.0;
            x_min = -1.0;
        }
        let mut y_min = self
            .rays
            .iter()
            .map(|r| r.pos.y)
            .fold(f64::INFINITY, f64::min)
            * 1.1;
        if !y_min.is_finite() {
            y_min = -1.0;
        }
        let mut y_max = self
            .rays
            .iter()
            .map(|r| r.pos.y)
            .fold(f64::NEG_INFINITY, f64::max)
            * 1.1;
        if !y_max.is_finite() {
            y_max = 1.0;
        }
        if (y_max - y_min).abs() < 10.0 * f64::EPSILON {
            y_max = 1.0;
            y_min = -1.0;
        }
        let mut chart = ChartBuilder::on(root)
            .margin(15)
            .x_label_area_size(100)
            .y_label_area_size(100)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;

        chart
            .configure_mesh()
            .x_desc("x (mm)")
            .y_desc("y (mm)")
            .label_style(TextStyle::from(("sans-serif", 30).into_font()))
            .draw()
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
        let points: Vec<(f64, f64)> = self.rays.iter().map(|ray| (ray.pos.x, ray.pos.y)).collect();
        let series = PointSeries::of_element(points, 5, &RED, &|c, s, st| {
            EmptyElement::at(c)    // We want to construct a composed element on-the-fly
                + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
        });

        chart
            .draw_series(series)
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
        root.present()
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{aperture::CircleConfig, spectrum::Spectrum};
    use approx::{abs_diff_eq, assert_abs_diff_eq};
    use assert_matches::assert_matches;
    use std::path::PathBuf;
    use uom::si::{energy::joule, length::nanometer};
    #[test]
    fn ray_new_collimated() {
        let position = Point2::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
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
    fn ray_new() {
        let position = Point2::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
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
    fn ray_propagate_along_z() {
        let wvl = Length::new::<nanometer>(1053.0);
        let energy = Energy::new::<joule>(1.0);
        let ray = Ray::new(
            Point2::new(Length::zero(), Length::zero()),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray
            .propagate_along_z(Length::new::<millimeter>(1.0))
            .is_ok());
        let newray = ray
            .propagate_along_z(Length::new::<millimeter>(1.0))
            .unwrap();
        assert_eq!(newray.wavelength(), wvl);
        assert_eq!(newray.energy(), energy);
        assert_eq!(newray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(
            ray.propagate_along_z(Length::new::<millimeter>(1.0))
                .unwrap()
                .position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(1.0)
            )
        );
        assert_eq!(
            ray.propagate_along_z(Length::new::<millimeter>(2.0))
                .unwrap()
                .position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(2.0)
            )
        );
        assert_eq!(
            ray.propagate_along_z(Length::new::<millimeter>(-1.0))
                .unwrap()
                .position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(-1.0)
            )
        );
        let ray = Ray::new(
            Point2::new(Length::zero(), Length::zero()),
            Vector3::new(0.0, 1.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert_eq!(
            ray.propagate_along_z(Length::new::<millimeter>(1.0))
                .unwrap()
                .position(),
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0)
            )
        );
        assert_eq!(
            ray.propagate_along_z(Length::new::<millimeter>(2.0))
                .unwrap()
                .position(),
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(2.0),
                Length::new::<millimeter>(2.0)
            )
        );
        let ray = Ray::new(
            Point2::new(Length::zero(), Length::zero()),
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
    fn ray_refract_paraxial() {
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(100.0))
                .unwrap()
                .pos,
            ray.pos
        );
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(100.0))
                .unwrap()
                .dir,
            ray.dir
        );
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
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(-100.0))
                .unwrap()
                .pos,
            ray.pos
        );
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(-100.0))
                .unwrap()
                .dir,
            ray.dir
        );
        let ray = Ray::new_collimated(
            Point2::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(2.0),
            ),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(100.0))
                .unwrap()
                .pos,
            ray.pos
        );
        let ray_dir = ray
            .refract_paraxial(Length::new::<millimeter>(100.0))
            .unwrap()
            .dir;
        let test_ray_dir = Vector3::new(-1.0, -2.0, 100.0) / 100.0; //.normalize();
        assert_abs_diff_eq!(ray_dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray_dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray_dir.z, test_ray_dir.z);
        assert_eq!(
            ray.refract_paraxial(Length::new::<millimeter>(-100.0))
                .unwrap()
                .pos,
            ray.pos
        );
        let ray_dir = ray
            .refract_paraxial(Length::new::<millimeter>(-100.0))
            .unwrap()
            .dir;
        let test_ray_dir = Vector3::new(1.0, 2.0, 100.0) / 100.0;
        assert_abs_diff_eq!(ray_dir.x, test_ray_dir.x);
        assert_abs_diff_eq!(ray_dir.y, test_ray_dir.y);
        assert_abs_diff_eq!(ray_dir.z, test_ray_dir.z);
    }
    #[test]
    fn ray_filter_energy() {
        let position = Point2::new(Length::zero(), Length::new::<millimeter>(1.0));
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
    fn ray_filter_spectrum() {
        let position = Point2::new(Length::zero(), Length::new::<millimeter>(1.0));
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
    fn ray_split() {
        let mut ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert!(ray.split(1.1).is_err());
        assert!(ray.split(-0.1).is_err());
        let split_ray = ray.split(0.1).unwrap();
        assert_eq!(ray.energy(), Energy::new::<joule>(0.1));
        assert_eq!(split_ray.energy(), Energy::new::<joule>(0.9));
        assert_eq!(ray.position(), split_ray.position());
        assert_eq!(ray.dir, split_ray.dir);
        assert_eq!(ray.wavelength(), split_ray.wavelength());
    }
    #[test]
    fn strategy_hexapolar() {
        let strategy = DistributionStrategy::Hexapolar(0);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 1);
        let strategy = DistributionStrategy::Hexapolar(1);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 7);
        let strategy = DistributionStrategy::Hexapolar(5);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 91);
    }
    #[test]
    fn strategy_random() {
        let strategy = DistributionStrategy::Random(10);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn strategy_sobol() {
        let strategy = DistributionStrategy::Sobol(10);
        assert_eq!(strategy.generate(Length::new::<millimeter>(1.0)).len(), 10);
    }
    #[test]
    fn rays_default() {
        let rays = Rays::default();
        assert_eq!(rays.nr_of_rays(), 0);
    }
    #[test]
    fn rays_new_uniform_collimated() {
        let wvl = Length::new::<nanometer>(1054.0);
        let energy = Energy::new::<joule>(1.0);
        let strategy = &DistributionStrategy::Hexapolar(2);
        let rays =
            Rays::new_uniform_collimated(Length::new::<millimeter>(1.0), wvl, energy, strategy);
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.rays.len(), 19);
        assert!(
            Energy::abs(rays.total_energy() - Energy::new::<joule>(1.0))
                < Energy::new::<joule>(10.0 * f64::EPSILON)
        );
        assert!(Rays::new_uniform_collimated(
            Length::new::<millimeter>(-0.1),
            wvl,
            energy,
            strategy
        )
        .is_err(),);
        assert!(Rays::new_uniform_collimated(
            Length::new::<millimeter>(f64::NAN),
            wvl,
            energy,
            strategy
        )
        .is_err(),);
        assert!(Rays::new_uniform_collimated(
            Length::new::<millimeter>(f64::INFINITY),
            wvl,
            energy,
            strategy
        )
        .is_err(),);
    }
    #[test]
    fn rays_new_uniform_collimated_zero() {
        let wvl = Length::new::<nanometer>(1054.0);
        let energy = Energy::new::<joule>(1.0);
        let strategy = &DistributionStrategy::Hexapolar(2);
        let rays = Rays::new_uniform_collimated(Length::zero(), wvl, energy, strategy);
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.rays.len(), 1);
        assert_eq!(rays.rays[0].pos, Point3::new(0.0, 0.0, 0.0));
        assert_eq!(rays.rays[0].dir, Vector3::z());
    }
    #[test]
    fn rays_new_hexapolar_point_source() {
        let position = Point2::new(Length::zero(), Length::zero());
        let wave_length = Length::new::<nanometer>(1053.0);
        let rays = Rays::new_hexapolar_point_source(
            position,
            Angle::new::<degree>(90.0),
            1,
            wave_length,
            Energy::new::<joule>(1.0),
        );

        let mut rays = rays.unwrap();
        for ray in &rays.rays {
            assert_eq!(
                ray.position(),
                Point3::new(Length::zero(), Length::zero(), Length::zero())
            )
        }
        rays.propagate_along_z(Length::new::<millimeter>(1.0))
            .unwrap();
        assert_eq!(
            rays.rays[0].position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(1.0)
            )
        );
        assert_eq!(rays.rays[1].position()[0], Length::zero());
        assert_abs_diff_eq!(
            rays.rays[1].position()[1].value,
            Length::new::<millimeter>(1.0).value
        );
        assert_eq!(rays.rays[1].position()[2], Length::new::<millimeter>(1.0));
        assert!(Rays::new_hexapolar_point_source(
            position,
            Angle::new::<degree>(-1.0),
            1,
            wave_length,
            Energy::new::<joule>(1.0),
        )
        .is_err());
        assert!(Rays::new_hexapolar_point_source(
            position,
            Angle::new::<degree>(180.0),
            1,
            wave_length,
            Energy::new::<joule>(1.0),
        )
        .is_err());
        assert!(Rays::new_hexapolar_point_source(
            position,
            Angle::new::<degree>(1.0),
            1,
            wave_length,
            Energy::new::<joule>(-0.1),
        )
        .is_err());
        let rays = Rays::new_hexapolar_point_source(
            position,
            Angle::zero(),
            1,
            wave_length,
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(rays.nr_of_rays(), 1);
        assert_eq!(
            rays.rays[0].position(),
            Point3::new(position.x, position.y, Length::zero())
        );
        assert_eq!(rays.rays[0].dir, Vector3::z());
    }
    #[test]
    fn rays_add_ray() {
        let mut rays = Rays::default();
        assert_eq!(rays.rays.len(), 0);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(rays.rays.len(), 1);
    }
    #[test]
    fn rays_add_rays() {
        let mut rays = Rays::default();
        assert_eq!(rays.rays.len(), 0);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(rays.rays.len(), 1);
        let mut rays2 = rays.clone();
        rays.add_rays(&mut rays2);
        assert_eq!(rays.rays.len(), 2);
    }
    #[test]
    fn rays_total_energy() {
        let mut rays = Rays::default();
        assert!(rays.total_energy().is_zero());
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), Energy::new::<joule>(1.0));
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), Energy::new::<joule>(2.0));

        let rays = Rays::new_uniform_collimated(
            Length::new::<millimeter>(1.0),
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
            &DistributionStrategy::Random(100000),
        )
        .unwrap();
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 1.0);
    }
    #[test]
    fn rays_centroid() {
        let mut rays = Rays::default();
        assert_eq!(rays.centroid(), None);
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(2.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(2.0),
                    Length::new::<millimeter>(3.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        assert_eq!(
            rays.centroid().unwrap(),
            Point3::new(
                Length::new::<millimeter>(1.5),
                Length::new::<millimeter>(2.5),
                Length::zero()
            )
        );
    }
    #[test]
    fn rays_beam_radius_geo() {
        let mut rays = Rays::default();
        assert!(rays.beam_radius_geo().is_none());
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(2.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(2.0),
                    Length::new::<millimeter>(3.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        assert_eq!(
            rays.beam_radius_geo().unwrap(),
            Length::new::<millimeter>(0.5_f64.sqrt())
        );
    }
    #[test]
    fn rays_beam_radius_rms() {
        let mut rays = Rays::default();
        assert!(rays.beam_radius_rms().is_none());
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(1.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        assert_eq!(rays.beam_radius_rms().unwrap(), Length::zero());
        rays.add_ray(
            Ray::new_collimated(
                Point2::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(0.0),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        assert_eq!(
            rays.beam_radius_rms().unwrap(),
            Length::new::<millimeter>(f64::sqrt(2.0) / 2.0)
        );
    }
    #[test]
    fn rays_propagate_along_z_axis() {
        let mut rays = Rays::default();
        let ray0 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::new::<millimeter>(1.0)),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        rays.propagate_along_z(Length::new::<millimeter>(1.0))
            .unwrap();
        assert_eq!(
            rays.rays[0].position(),
            Point3::new(
                Length::zero(),
                Length::zero(),
                Length::new::<millimeter>(1.0)
            )
        );
        assert_eq!(
            rays.rays[1].position(),
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0)
            )
        );
    }
    #[test]
    fn rays_refract_paraxial() {
        let mut rays = Rays::default();
        assert!(rays
            .refract_paraxial(Length::new::<millimeter>(0.0))
            .is_err());
        assert!(rays
            .refract_paraxial(Length::new::<millimeter>(f64::NAN))
            .is_err());
        assert!(rays
            .refract_paraxial(Length::new::<millimeter>(f64::INFINITY))
            .is_err());
        assert!(rays
            .refract_paraxial(Length::new::<millimeter>(f64::NEG_INFINITY))
            .is_err());
        assert!(rays
            .refract_paraxial(Length::new::<millimeter>(100.0))
            .is_ok());
        let ray0 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::new::<millimeter>(1.0)),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray0.clone());
        rays.add_ray(ray1.clone());
        rays.refract_paraxial(Length::new::<millimeter>(100.0))
            .unwrap();
        assert_eq!(rays.rays[0].pos, ray0.pos);
        assert_eq!(rays.rays[0].dir, ray0.dir);
        assert_eq!(rays.rays[1].pos, ray1.pos);
        let new_dir = Vector3::new(0.0, -1.0, 100.0) / 100.0;
        assert_abs_diff_eq!(rays.rays[1].dir.x, new_dir.x);
        assert_abs_diff_eq!(rays.rays[1].dir.y, new_dir.y);
        assert_abs_diff_eq!(rays.rays[1].dir.z, new_dir.z);
    }
    #[test]
    fn rays_filter_energy() {
        let mut rays = Rays::default();
        assert!(rays.filter_energy(&FilterType::Constant(0.5)).is_ok());
        assert!(rays.filter_energy(&FilterType::Constant(-0.1)).is_err());
        assert!(rays.filter_energy(&FilterType::Constant(1.1)).is_err());
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::new::<millimeter>(1.0)),
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray.clone());
        let new_ray = ray.filter_energy(&FilterType::Constant(0.3)).unwrap();
        rays.filter_energy(&FilterType::Constant(0.3)).unwrap();
        assert_eq!(rays.rays[0].pos, new_ray.pos);
        assert_eq!(rays.rays[0].dir, new_ray.dir);
        assert_eq!(rays.rays[0].wvl, new_ray.wvl);
        assert_eq!(rays.rays[0].e, new_ray.e);
        assert_eq!(rays.rays.len(), 1);
    }
    #[test]
    fn rays_apodize() {
        let mut rays = Rays::default();
        let ray0 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point2::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        assert_eq!(rays.total_energy(), Energy::new::<joule>(2.0));
        let circle_config = CircleConfig::new(0.5, Point2::new(0.0, 0.0)).unwrap();
        let aperture = Aperture::BinaryCircle(circle_config);
        rays.apodize(&aperture);
        assert_eq!(rays.total_energy(), Energy::new::<joule>(1.0));
    }
    #[test]
    fn rays_wavelength_range() {
        let mut rays = Rays::default();
        assert_eq!(rays.wavelength_range(), None);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point2::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(Length::new::<nanometer>(1053.0)..Length::new::<nanometer>(1053.0))
        );
        let ray = Ray::new_collimated(
            Point2::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            Length::new::<nanometer>(1050.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(Length::new::<nanometer>(1050.0)..Length::new::<nanometer>(1053.0))
        );
        let ray = Ray::new_collimated(
            Point2::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            Length::new::<nanometer>(1051.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(Length::new::<nanometer>(1050.0)..Length::new::<nanometer>(1053.0))
        );
    }
    #[test]
    fn rays_into_proptype() {
        assert_matches!(Rays::default().into(), Proptype::Rays(_));
    }
    #[test]
    fn rays_to_spectrum() {
        let mut rays = Rays::default();
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1052.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1052.1),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let spectrum = rays.to_spectrum(&Length::new::<nanometer>(0.5)).unwrap();
        println!("{}", spectrum);
        assert_abs_diff_eq!(
            spectrum.total_energy(),
            4.0,
            epsilon = 100000.0 * f64::EPSILON
        );
    }
    #[test]
    fn rays_split() {
        assert!(Rays::default().split(1.1).is_err());
        assert!(Rays::default().split(-0.1).is_err());
        let ray1 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray2 = Ray::new_collimated(
            Point2::new(Length::zero(), Length::zero()),
            Length::new::<nanometer>(1050.0),
            Energy::new::<joule>(2.0),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(ray1.clone());
        rays.add_ray(ray2.clone());
        let split_rays = rays.split(0.2).unwrap();
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(
            split_rays.total_energy().get::<joule>(),
            2.4,
            epsilon = 10.0 * f64::EPSILON
        );
    }
}
