#![warn(missing_docs)]
//! Module for handling rays
use std::ops::Range;

use crate::aperture::Aperture;
use crate::error::{OpmResult, OpossumError};
use crate::nodes::FilterType;
use crate::ray::{Ray, SplittingConfig};
use crate::spectrum::Spectrum;
use kahan::KahanSummator;
use nalgebra::{distance, point, MatrixXx2, MatrixXx3, Point2, Point3, Vector2, Vector3};
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use sobol::{params::JoeKuoD6, Sobol};
use uom::num_traits::Zero;
use uom::si::angle::degree;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::{millimeter, nanometer};

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
        let energies: Vec<f64> = self
            .rays
            .iter()
            .map(|r| r.energy().get::<joule>())
            .collect();
        let kahan_sum: kahan::KahanSum<f64> = energies.iter().kahan_sum();
        Energy::new::<joule>(kahan_sum.sum())
    }
    /// Returns the number of rays of this [`Rays`].
    #[must_use]
    pub fn nr_of_rays(&self) -> usize {
        self.rays.len()
    }
    /// Apodize (cut out or attenuate) the ray bundle by a given [`Aperture`].
    ///
    /// # Errors
    ///
    /// This function returns an error if a single ray cannot be propery apodized (e.g. filter factor outside (0.0..=1.0)).
    pub fn apodize(&mut self, aperture: &Aperture) -> OpmResult<()> {
        let mut new_rays: Vec<Ray> = Vec::new();
        for ray in &self.rays {
            let pos = point![
                ray.position().x.get::<millimeter>(),
                ray.position().y.get::<millimeter>()
            ];
            let ap_factor = aperture.apodization_factor(&pos);
            if ap_factor > 0.0 {
                let new_ray = ray.filter_energy(&FilterType::Constant(ap_factor))?;
                new_rays.push(new_ray);
            }
        }
        self.rays = new_rays;
        Ok(())
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
        let c = self
            .rays
            .iter()
            .fold((Length::zero(), Length::zero(), Length::zero()), |c, r| {
                let pos = r.position();
                (c.0 + pos.x, c.1 + pos.y, c.2 + pos.z)
            });
        Some(Point3::new(c.0 / len, c.1 / len, c.2 / len))
    }
    /// Returns the geometric beam radius [`Rays`].
    ///
    /// This function calculates the maximum distance of a ray bundle from its centroid.
    #[must_use]
    pub fn beam_radius_geo(&self) -> Option<Length> {
        self.centroid().map(|c| {
            let c_in_millimeter = Point2::new(c.x.get::<millimeter>(), c.y.get::<millimeter>());
            let mut max_dist = 0.0;
            for ray in &self.rays {
                let ray_2d = Point2::new(
                    ray.position().x.get::<millimeter>(),
                    ray.position().y.get::<millimeter>(),
                );
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
                let ray_2d = Point2::new(
                    ray.position().x.get::<millimeter>(),
                    ray.position().y.get::<millimeter>(),
                );
                sum_dist_sq += distance(&ray_2d, &c_in_millimeter).powi(2);
            }
            #[allow(clippy::cast_precision_loss)]
            let nr_of_rays = self.rays.len() as f64;
            sum_dist_sq /= nr_of_rays;
            Length::new::<millimeter>(sum_dist_sq.sqrt())
        })
    }
    /// Returns the wavefront of the bundle of [`Rays`] at a specific wavelength wvl.
    ///
    /// This function calculates the wavefront of a ray bundle as multiple of its wavelength with reference to the ray that is closest to the optical axis.
    #[must_use]
    pub fn wavefront_error_in_lambda_at_wvl(&self, wvl: f64) -> MatrixXx3<f64> {
        let mut optical_path_length_at_pos = MatrixXx3::from_element(self.rays.len(), 0.);
        let mut min_radius = f64::INFINITY;
        let mut path_length_at_center = 0.;
        for (i, ray) in self.rays.iter().enumerate() {
            let position = Vector2::new(
                ray.position().x.get::<millimeter>(),
                ray.position().y.get::<millimeter>(),
            );
            optical_path_length_at_pos[(i, 0)] = position.x;
            optical_path_length_at_pos[(i, 1)] = position.y;
            optical_path_length_at_pos[(i, 2)] = ray.path_length().get::<nanometer>();

            let radius = position.x.mul_add(position.x, position.y * position.y);
            if radius < min_radius {
                min_radius = radius;
                path_length_at_center = optical_path_length_at_pos[(i, 2)];
            }
        }

        for mut ray_path in optical_path_length_at_pos.row_iter_mut() {
            ray_path[2] -= path_length_at_center;
            ray_path[2] /= wvl;
        }

        //the wavefront error has the negative sign of the optical path difference
        -optical_path_length_at_pos
    }

    /// Returns the x and y positions of the ray bundle in form of a `[MatrixXx3<f64>]`.
    #[must_use]
    pub fn get_xy_rays_pos(&self) -> MatrixXx2<f64> {
        let mut rays_at_pos = MatrixXx2::from_element(self.rays.len(), 0.);
        for (row, ray) in self.rays.iter().enumerate() {
            rays_at_pos[(row, 0)] = ray.position().x.get::<millimeter>();
            rays_at_pos[(row, 1)] = ray.position().y.get::<millimeter>();
        }
        rays_at_pos
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
    pub fn add_rays(&mut self, rays: &mut Self) {
        self.rays.append(&mut rays.rays);
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
    /// Remove rays below a given energy threshold.
    ///
    /// Removes all rays with an energy (per ray) below the given threshold.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given energy threshold is negative or not finite.
    pub fn delete_by_threshold_energy(&mut self, min_energy_per_ray: Energy) -> OpmResult<()> {
        if !min_energy_per_ray.is_finite() || min_energy_per_ray.is_sign_negative() {
            return Err(OpossumError::Other(
                "threshold energy must be >=0.0 and finite".into(),
            ));
        };
        self.rays.retain(|ray| ray.energy() >= min_energy_per_ray);
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
    /// Split a ray bundle
    ///
    /// This function splits a ray bundle determined by the given [`SplittingConfig`]. See [`split`](Ray::split) function for details.
    /// # Errors
    ///
    /// This function will return an error if the underlying split function for a single ray returns an error.
    pub fn split(&mut self, config: &SplittingConfig) -> OpmResult<Self> {
        let mut split_rays = Self::default();
        for ray in &mut self.rays {
            let split_ray = ray.split(config)?;
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

// impl From<Rays> for Proptype {
//     fn from(value: Rays) -> Self {
//         Self::Rays(value)
//     }
// }
// impl PdfReportable for Rays {
//     fn pdf_report(&self) -> crate::error::OpmResult<genpdf::elements::LinearLayout> {
//         let mut layout = genpdf::elements::LinearLayout::vertical();
//         let img = self.to_img_buf_plot().unwrap();
//         layout.push(
//             genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(img))
//                 .map_err(|e| format!("adding of image failed: {e}"))?,
//         );
//         Ok(layout)
//     }
// }
// impl Plottable for Rays {
//     fn chart<B: plotters::prelude::DrawingBackend>(
//         &self,
//         root: &plotters::prelude::DrawingArea<B, plotters::coord::Shift>,
//     ) -> crate::error::OpmResult<()> {
//         let mut x_min = self
//             .rays
//             .iter()
//             .map(|r| r.pos.x)
//             .fold(f64::INFINITY, f64::min)
//             * 1.1;
//         if !x_min.is_finite() {
//             x_min = -1.0;
//         }
//         let mut x_max = self
//             .rays
//             .iter()
//             .map(|r| r.pos.x)
//             .fold(f64::NEG_INFINITY, f64::max)
//             * 1.1;
//         if !x_max.is_finite() {
//             x_max = 1.0;
//         }
//         if (x_max - x_min).abs() < f64::EPSILON {
//             x_max = 1.0;
//             x_min = -1.0;
//         }
//         let mut y_min = self
//             .rays
//             .iter()
//             .map(|r| r.pos.y)
//             .fold(f64::INFINITY, f64::min)
//             * 1.1;
//         if !y_min.is_finite() {
//             y_min = -1.0;
//         }
//         let mut y_max = self
//             .rays
//             .iter()
//             .map(|r| r.pos.y)
//             .fold(f64::NEG_INFINITY, f64::max)
//             * 1.1;
//         if !y_max.is_finite() {
//             y_max = 1.0;
//         }
//         if (y_max - y_min).abs() < f64::EPSILON {
//             y_max = 1.0;
//             y_min = -1.0;
//         }
//         let mut chart = ChartBuilder::on(root)
//             .margin(15)
//             .x_label_area_size(100)
//             .y_label_area_size(100)
//             .build_cartesian_2d(x_min..x_max, y_min..y_max)
//             .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;

//         chart
//             .configure_mesh()
//             .x_desc("x (mm)")
//             .y_desc("y (mm)")
//             .label_style(TextStyle::from(("sans-serif", 30).into_font()))
//             .draw()
//             .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
//         let points: Vec<(f64, f64)> = self.rays.iter().map(|ray| (ray.pos.x, ray.pos.y)).collect();
//         let series = PointSeries::of_element(points, 5, &RED, &|c, s, st| {
//             EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//                 + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         });

//         chart
//             .draw_series(series)
//             .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
//         root.present()
//             .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;
//         Ok(())
//     }
// }
#[cfg(test)]
mod test {
    use super::*;
    use crate::{aperture::CircleConfig, ray::SplittingConfig};
    use approx::assert_abs_diff_eq;
    use uom::si::{energy::joule, length::nanometer};
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
    fn default() {
        let rays = Rays::default();
        assert_eq!(rays.nr_of_rays(), 0);
    }
    #[test]
    fn new_uniform_collimated() {
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
    fn new_uniform_collimated_zero() {
        let wvl = Length::new::<nanometer>(1054.0);
        let energy = Energy::new::<joule>(1.0);
        let strategy = &DistributionStrategy::Hexapolar(2);
        let rays = Rays::new_uniform_collimated(Length::zero(), wvl, energy, strategy);
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.rays.len(), 1);
        assert_eq!(
            rays.rays[0].position(),
            Point3::new(Length::zero(), Length::zero(), Length::zero())
        );
        assert_eq!(rays.rays[0].direction(), Vector3::z());
    }
    #[test]
    fn new_hexapolar_point_source() {
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
        assert_eq!(rays.rays[0].direction(), Vector3::z());
    }
    #[test]
    fn add_ray() {
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
    fn add_rays() {
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
    fn total_energy() {
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
    fn centroid() {
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
    fn beam_radius_geo() {
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
    fn beam_radius_rms() {
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
    fn propagate_along_z_axis() {
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
    fn refract_paraxial() {
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
        assert_eq!(rays.rays[0].position(), ray0.position());
        assert_eq!(rays.rays[0].direction(), ray0.direction());
        assert_eq!(rays.rays[1].position(), ray1.position());
        let new_dir = Vector3::new(0.0, -1.0, 100.0) / 100.0;
        assert_abs_diff_eq!(rays.rays[1].direction().x, new_dir.x);
        assert_abs_diff_eq!(rays.rays[1].direction().y, new_dir.y);
        assert_abs_diff_eq!(rays.rays[1].direction().z, new_dir.z);
    }
    #[test]
    fn filter_energy() {
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
        assert_eq!(rays.rays[0].position(), new_ray.position());
        assert_eq!(rays.rays[0].direction(), new_ray.direction());
        assert_eq!(rays.rays[0].wavelength(), new_ray.wavelength());
        assert_eq!(rays.rays[0].energy(), new_ray.energy());
        assert_eq!(rays.rays.len(), 1);
    }
    #[test]
    fn delete_by_threshold() {
        let mut rays = Rays::default();
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(f64::NAN))
            .is_err());
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(f64::INFINITY))
            .is_err());
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(-0.1))
            .is_err());
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(0.0))
            .is_ok());
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
            Energy::new::<joule>(0.1),
        )
        .unwrap();
        rays.add_ray(ray);
        rays.delete_by_threshold_energy(Energy::new::<joule>(0.1))
            .unwrap();
        assert_eq!(rays.nr_of_rays(), 2);
        rays.delete_by_threshold_energy(Energy::new::<joule>(0.5))
            .unwrap();
        assert_eq!(rays.nr_of_rays(), 1);
        rays.delete_by_threshold_energy(Energy::new::<joule>(1.1))
            .unwrap();
        assert_eq!(rays.nr_of_rays(), 0);
    }
    #[test]
    fn apodize() {
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
        rays.apodize(&aperture).unwrap();
        assert_eq!(rays.total_energy(), Energy::new::<joule>(1.0));
    }
    #[test]
    fn wavelength_range() {
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
    fn to_spectrum() {
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
    fn split() {
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
        assert!(rays.split(&SplittingConfig::Ratio(1.1)).is_err());
        assert!(rays.split(&SplittingConfig::Ratio(-0.1)).is_err());
        let split_rays = rays.split(&SplittingConfig::Ratio(0.2)).unwrap();
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(
            split_rays.total_energy().get::<joule>(),
            2.4,
            epsilon = 10.0 * f64::EPSILON
        );
    }
}
