#![warn(missing_docs)]
//! Module for handling ray bundles
use std::ops::Range;
use std::path::Path;

use crate::aperture::Aperture;
use crate::distribution::DistributionStrategy;
use crate::error::{OpmResult, OpossumError};
use crate::nodes::wavefront::{WaveFrontData, WaveFrontErrorMap};
use crate::nodes::FilterType;
use crate::plottable::{PlotArgs, PlotData, PlotParameters, PlotType, Plottable, PltBackEnd};
use crate::properties::Proptype;
use crate::ray::{Ray, SplittingConfig};
use crate::reporter::PdfReportable;
use crate::spectrum::Spectrum;
use crate::surface::Surface;
use image::{DynamicImage, ImageBuffer};
use kahan::KahanSummator;
use log::warn;
use nalgebra::{distance, point, MatrixXx2, MatrixXx3, Point2, Point3, Vector2, Vector3};
use num::ToPrimitive;
use serde_derive::{Deserialize, Serialize};
use uom::num_traits::Zero;
use uom::si::angle::degree;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::{micrometer, millimeter, nanometer};

/// Struct containing all relevant information of a ray bundle
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rays {
    ///vector containing rays
    rays: Vec<Ray>,
    // ***
    // *** only temporary before we have concept for coordinate system
    // ***
    dist_to_next_surface: Length,
    z_position: Length,
    // ***
    // ***
    // ***
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
        let points: Vec<Point3<Length>> = if size.is_zero() {
            vec![Point3::new(Length::zero(), Length::zero(), Length::zero())]
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
        Ok(Self {
            rays,
            dist_to_next_surface: Length::zero(),
            z_position: Length::zero(),
        })
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
        position: Point3<Length>,
        cone_angle: Angle,
        nr_of_rings: u8,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        if cone_angle.is_sign_negative() || cone_angle >= Angle::new::<degree>(180.0) {
            return Err(OpossumError::Other(
                "cone angle must be within (0.0..180.0) degrees range".into(),
            ));
        }
        let size_after_unit_length = (cone_angle / 2.0).tan().value;
        let points: Vec<Point3<Length>> = if cone_angle.is_zero() {
            vec![Point3::new(Length::zero(), Length::zero(), Length::zero())]
        } else {
            DistributionStrategy::Hexapolar { nr_of_rings }
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
        Ok(Self {
            rays,
            dist_to_next_surface: Length::zero(),
            z_position: Length::zero(),
        })
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
    /// Returns the wavefront of the bundle of [`Rays`] at the center wavelength or at each band of the spectrum with a defined resolution.
    /// This function calculates the wavefront of a ray bundle as multiple of its wavelength with reference to the ray that is closest to the optical axis.
    /// # Attributes
    /// - `center_wavelength_flag`: flag to define if the center wavelength should be used for calculation or if a wavefront for all spectral components should be analyzed
    /// - `spec_res`: spectral resolution to calculate the center wavelength or for individal spectral analysis
    ///
    /// # Errors
    /// This function errors for the moment if `center_wavelength_flag` is set to false
    ///
    /// # Panics
    /// This method panics if the usize `to_f64()`conversion fails. This is not expected
    pub fn get_wavefront_data_in_units_of_wvl(
        &self,
        center_wavelength_flag: bool,
        spec_res: Length,
    ) -> OpmResult<WaveFrontData> {
        let spec = self.to_spectrum(&spec_res)?;
        if center_wavelength_flag {
            let center_wavelength = spec.center_wavelength();
            let wf_err = self.wavefront_error_at_pos_in_units_of_wvl(center_wavelength);
            Ok(WaveFrontData {
                wavefront_error_maps: vec![WaveFrontErrorMap::new(&wf_err, center_wavelength)?],
            })
        } else {
            let spec_start = spec.range().start.get::<micrometer>();
            let spec_res_micro = spec_res.get::<micrometer>();
            let mut rays_sorted_by_spectrum = vec![Vec::<Ray>::new(); spec.data_vec().len()];
            //sort rays into spectral resolution fields
            for ray in &self.rays {
                let index_opt = ((ray.wavelength().get::<micrometer>() - spec_start)
                    / spec_res_micro)
                    .floor()
                    .to_usize();
                if let Some(idx) = index_opt {
                    rays_sorted_by_spectrum[idx].push(ray.clone());
                }
            }

            let mut wf_error_maps =
                Vec::<WaveFrontErrorMap>::with_capacity(rays_sorted_by_spectrum.len());
            for (idx, rays) in rays_sorted_by_spectrum.iter().enumerate() {
                if !rays.is_empty() {
                    let wvl = idx.to_f64().unwrap().mul_add(spec_res_micro, spec_start);
                    wf_error_maps.push(WaveFrontErrorMap::new(
                        &Self::from(rays.clone())
                            .wavefront_error_at_pos_in_units_of_wvl(Length::new::<micrometer>(wvl)),
                        Length::new::<micrometer>(wvl),
                    )?);
                }
            }

            Ok(WaveFrontData {
                wavefront_error_maps: wf_error_maps,
            })
        }
    }

    /// Calculates the wavefront error of a ray bundle with a specified wavelength at a certain position along the optical axis in the optical system
    /// # Attributes
    /// - `wavelength`: wave length that is used for this wavefront calculation
    ///
    /// # Returns
    /// This method returns a Matrix with 3 columns for the x(1) & y(2) axes and the negative optical path(3) and a dynamic number of rows. x & y referes to the transverse extend of the beam with reference to its the optical axis
    #[must_use]
    pub fn wavefront_error_at_pos_in_units_of_wvl(&self, wavelength: Length) -> MatrixXx3<f64> {
        let wvl = wavelength.get::<nanometer>();
        let mut wave_front_err = MatrixXx3::from_element(self.rays.len(), 0.);
        let mut min_radius = f64::INFINITY;
        let mut path_length_at_center = 0.;
        for (i, ray) in self.rays.iter().enumerate() {
            let position = Vector2::new(
                ray.position().x.get::<millimeter>(),
                ray.position().y.get::<millimeter>(),
            );
            wave_front_err[(i, 0)] = position.x;
            wave_front_err[(i, 1)] = position.y;
            //the wavefront error has the negative sign of the optical path difference
            wave_front_err[(i, 2)] = -ray.path_length().get::<nanometer>();

            let radius = position.x.mul_add(position.x, position.y * position.y);
            if radius < min_radius {
                min_radius = radius;
                path_length_at_center = wave_front_err[(i, 2)];
            }
        }

        for mut wf_err in wave_front_err.row_iter_mut() {
            wf_err[2] -= path_length_at_center;
            wf_err[2] /= wvl;
        }

        wave_front_err
    }

    /// Returns the x and y positions of the ray bundle in form of a `[MatrixXx2<f64>]`.
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
    /// The propagation length must be set with the function `set_dist_to_next_surface`.
    /// # Errors
    /// This function returns an error if
    ///  - the z component of a ray direction is zero.
    ///  - the given length is not finite.
    pub fn propagate_along_z(&mut self) -> OpmResult<()> {
        if !self.dist_to_next_surface.is_zero() {
            for ray in &mut self.rays {
                ray.propagate_along_z(self.dist_to_next_surface)?;
            }
        }
        self.z_position += self.dist_to_next_surface;
        self.set_dist_zero();
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
            ray.refract_paraxial(focal_length)?;
        }
        Ok(())
    }
    /// Refract a ray bundle on a [`Surface`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn refract_on_surface(&mut self, surface: &dyn Surface, n2: f64) -> OpmResult<()> {
        for ray in &mut self.rays {
            ray.refract_on_surface(surface, n2)?;
        }
        self.z_position += self.dist_to_next_surface;
        self.set_dist_zero();
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
    /// # Warnings
    ///
    /// This function emits a warning log entry if the given threshold is negative. In this case the ray bundle is not modified.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given energy threshold is not finite.
    pub fn delete_by_threshold_energy(&mut self, min_energy_per_ray: Energy) -> OpmResult<()> {
        if min_energy_per_ray.is_sign_negative() {
            warn!("negative threshold energy given. Ray bundle unmodified.");
            return Ok(());
        }
        if !min_energy_per_ray.is_finite() {
            return Err(OpossumError::Other(
                "threshold energy must be finite".into(),
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
    /// Set the refractive index of the medium all [`Rays`] are propagating in.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive index is < 1.0 or not finite.
    pub fn set_refractive_index(&mut self, refractive_index: f64) -> OpmResult<()> {
        if refractive_index < 1.0 || !refractive_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index must be >=1.0 and finite".into(),
            ));
        }
        for ray in &mut self.rays {
            ray.set_refractive_index(refractive_index)?;
        }
        Ok(())
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
    /// Get the position history of all rays in thie ray bundle
    ///
    /// # Returns
    /// This method returns a vector of N-row x 3 column matrices that contain the position history of all the rays
    #[must_use]
    pub fn get_rays_position_history_in_mm(&self) -> RayPositionHistory {
        let mut rays_pos_history = Vec::<MatrixXx3<f64>>::with_capacity(self.rays.len());
        for ray in &self.rays {
            rays_pos_history.push(ray.position_history_in_mm());
        }
        RayPositionHistory { rays_pos_history }
    }
    /// Returns the dist to next surface of this [`Rays`].
    ///
    /// **Note**: This function is a hack and will be removed in later versions.
    #[must_use]
    pub fn dist_to_next_surface(&self) -> Length {
        self.dist_to_next_surface
    }
    /// Sets the dist to next surface of this [`Rays`].
    ///
    /// **Note**: This function is a hack and will be removed in later versions.
    pub fn set_dist_to_next_surface(&mut self, dist_to_next_surface: Length) {
        self.dist_to_next_surface = dist_to_next_surface;
    }
    fn set_dist_zero(&mut self) {
        self.dist_to_next_surface = Length::zero();
    }
    /// Returns the absolute z of last surface of this [`Rays`].
    #[must_use]
    pub fn absolute_z_of_last_surface(&self) -> Length {
        self.z_position
    }
}

/// struct that holds the history of the ray positions that is needed for report generation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RayPositionHistory {
    /// vector of ray positions for each ray
    pub rays_pos_history: Vec<MatrixXx3<f64>>,
}
impl RayPositionHistory {
    /// Projects a set of 3d vectors onto a plane
    /// # Attributes
    /// `plane_normal_vec`: normal vector of the plane to project onto
    ///
    /// # Errors
    /// This function errors if the length of the plane normal vector is zero
    /// # Returns
    /// This function returns a set of 2d vectors in the defined plane projected to a view that is perpendicular to this plane.
    pub fn project_to_plane(
        &self,
        plane_normal_vec: Vector3<f64>,
    ) -> OpmResult<Vec<MatrixXx2<f64>>> {
        let vec_norm = plane_normal_vec.norm();

        if vec_norm < f64::EPSILON {
            return Err(OpossumError::Other(
                "The plane normal vector must have a non-zero length!".into(),
            ));
        }

        let normed_normal_vec = plane_normal_vec / vec_norm;

        //define an axis on the plane.
        //Do this by projection of one of the main coordinate axes onto that plane
        //Beforehand check, if these axes are not parallel to the normal vec
        let (co_ax_1, co_ax_2) =
            if plane_normal_vec.cross(&Vector3::new(1., 0., 0.)).norm() < f64::EPSILON {
                //parallel to the x-axis
                (Vector3::new(0., 0., 1.), Vector3::new(0., 1., 0.))
            } else if plane_normal_vec.cross(&Vector3::new(0., 1., 0.)).norm() < f64::EPSILON {
                (Vector3::new(0., 0., 1.), Vector3::new(1., 0., 0.))
            } else if plane_normal_vec.cross(&Vector3::new(0., 0., 1.)).norm() < f64::EPSILON {
                (Vector3::new(1., 0., 0.), Vector3::new(0., 1., 0.))
            } else {
                //arbitrarily project x-axis onto that plane
                let x_vec = Vector3::new(1., 0., 0.);
                let mut proj_x = x_vec - x_vec.dot(&normed_normal_vec) * plane_normal_vec;
                proj_x /= proj_x.norm();

                //second axis defined by cross product of x-axis projection and plane normal, which yields another vector that is perpendicular to both others
                (proj_x, proj_x.cross(&normed_normal_vec))
            };

        let mut rays_pos_projection =
            Vec::<MatrixXx2<f64>>::with_capacity(self.rays_pos_history.len());
        for ray_pos in &self.rays_pos_history {
            let mut projected_ray_pos = MatrixXx2::<f64>::zeros(ray_pos.column(0).len());
            for (row, pos) in ray_pos.row_iter().enumerate() {
                let pos_t = pos.transpose();
                let proj_pos = pos_t - pos_t.dot(&normed_normal_vec) * plane_normal_vec;

                projected_ray_pos[(row, 0)] = proj_pos.dot(&co_ax_1);
                projected_ray_pos[(row, 1)] = proj_pos.dot(&co_ax_2);
            }
            rays_pos_projection.push(projected_ray_pos);
        }
        Ok(rays_pos_projection)
    }
}
impl PdfReportable for RayPositionHistory {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let img = self.to_plot(Path::new(""), (1000, 1000), PltBackEnd::Buf)?;
        layout.push(
            genpdf::elements::Image::from_dynamic_image(DynamicImage::ImageRgb8(
                img.unwrap_or_else(ImageBuffer::default),
            ))
            .map_err(|e| format!("adding of image failed: {e}"))?,
        );
        Ok(layout)
    }
}

impl From<Vec<Ray>> for Rays {
    fn from(value: Vec<Ray>) -> Self {
        Self {
            rays: value,
            dist_to_next_surface: Length::zero(),
            z_position: Length::zero(),
        }
    }
}

impl From<Rays> for Proptype {
    fn from(value: Rays) -> Self {
        Self::RayPositionHistory(value.get_rays_position_history_in_mm())
    }
}

impl Plottable for RayPositionHistory {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm (z axis)".into()))?
            .set(&PlotArgs::YLabel("distance in mm (y axis)".into()))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::MultiLine2D(plt_params.clone())
    }

    fn get_plot_data(&self, _plt_type: &PlotType) -> OpmResult<Option<PlotData>> {
        if self.rays_pos_history.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PlotData::MultiDim2(
                self.project_to_plane(Vector3::new(1., 0., 0.))?,
            )))
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{aperture::CircleConfig, ray::SplittingConfig};
    use approx::assert_abs_diff_eq;
    use itertools::izip;
    use log::Level;
    use testing_logger;
    use uom::si::{energy::joule, length::nanometer};
    #[test]
    fn default() {
        let rays = Rays::default();
        assert_eq!(rays.nr_of_rays(), 0);
    }
    #[test]
    fn new_uniform_collimated() {
        let wvl = Length::new::<nanometer>(1054.0);
        let energy = Energy::new::<joule>(1.0);
        let strategy = &DistributionStrategy::Hexapolar { nr_of_rings: 2 };
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
        let strategy = &DistributionStrategy::Hexapolar { nr_of_rings: 2 };
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
        let position = Point3::new(Length::zero(), Length::zero(), Length::zero());
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
        rays.set_dist_to_next_surface(Length::new::<millimeter>(1.0));
        rays.propagate_along_z().unwrap();
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
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
    fn set_refractive_index() {
        let mut rays = Rays::default();
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert!(rays.set_refractive_index(0.9).is_err());
        assert!(rays.set_refractive_index(f64::NAN).is_err());
        assert!(rays.set_refractive_index(f64::INFINITY).is_err());
        assert!(rays.set_refractive_index(1.0).is_ok());
        rays.set_refractive_index(2.0).unwrap();
        assert_eq!(rays.rays[0].refractive_index(), 2.0);
        assert_eq!(rays.rays[1].refractive_index(), 2.0);
    }
    #[test]
    fn total_energy() {
        let mut rays = Rays::default();
        assert!(rays.total_energy().is_zero());
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
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
            &DistributionStrategy::Random {
                nr_of_points: 100000,
            },
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
                Point3::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(2.0),
                    Length::zero(),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(
                Point3::new(
                    Length::new::<millimeter>(2.0),
                    Length::new::<millimeter>(3.0),
                    Length::zero(),
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
                Point3::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(2.0),
                    Length::zero(),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(
                Point3::new(
                    Length::new::<millimeter>(2.0),
                    Length::new::<millimeter>(3.0),
                    Length::zero(),
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
                Point3::new(
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(1.0),
                    Length::zero(),
                ),
                Length::new::<nanometer>(1053.0),
                Energy::new::<joule>(1.0),
            )
            .unwrap(),
        );
        assert_eq!(rays.beam_radius_rms().unwrap(), Length::zero());
        rays.add_ray(
            Ray::new_collimated(
                Point3::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(0.0),
                    Length::zero(),
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::zero(),
            ),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        rays.set_dist_to_next_surface(Length::new::<millimeter>(1.0));
        rays.propagate_along_z().unwrap();
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::zero(),
            ),
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
            Point3::new(
                Length::zero(),
                Length::new::<millimeter>(1.0),
                Length::zero(),
            ),
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
        testing_logger::setup();
        let mut rays = Rays::default();
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(f64::NAN))
            .is_err());
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(f64::INFINITY))
            .is_err());
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(-0.1))
            .is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "negative threshold energy given. Ray bundle unmodified."
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
        assert!(rays
            .delete_by_threshold_energy(Energy::new::<joule>(0.0))
            .is_ok());
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray1 = Ray::new_collimated(
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
                Length::zero(),
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
                Length::zero(),
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
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
                Length::zero(),
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
            Point3::new(
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
                Length::zero(),
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
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1052.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1052.1),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        let spectrum = rays.to_spectrum(&Length::new::<nanometer>(0.5)).unwrap();
        assert_abs_diff_eq!(
            spectrum.total_energy(),
            4.0,
            epsilon = 100000.0 * f64::EPSILON
        );
    }
    #[test]
    fn split() {
        let ray1 = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let ray2 = Ray::new_collimated(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
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

    #[test]
    fn get_rays_position_history_in_mm() {
        let ray_vec = vec![Ray::new(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Vector3::new(0., 1., 2.),
            Length::new::<nanometer>(1053.),
            Energy::new::<joule>(1.),
        )
        .unwrap()];
        let mut rays = Rays::from(ray_vec);
        rays.set_dist_to_next_surface(Length::new::<millimeter>(1.));
        let _ = rays.propagate_along_z();
        rays.set_dist_to_next_surface(Length::new::<millimeter>(2.));
        let _ = rays.propagate_along_z();

        let pos_hist_comp = vec![MatrixXx3::from_vec(vec![
            0., 0., 0., 0., 0.5, 1.5, 0., 1., 3.,
        ])];

        let pos_hist = rays.get_rays_position_history_in_mm();
        for (ray_pos, ray_pos_calc) in izip!(pos_hist_comp.iter(), pos_hist.rays_pos_history.iter())
        {
            for (row, row_calc) in izip!(ray_pos.row_iter(), ray_pos_calc.row_iter()) {
                assert_eq!(row[0], row_calc[0]);
                assert_eq!(row[1], row_calc[1]);
                assert_eq!(row[2], row_calc[2]);
            }
        }
    }
    #[test]
    fn project_to_plane() {
        let pos_hist = RayPositionHistory {
            rays_pos_history: vec![
                MatrixXx3::from_vec(vec![1., 0., 0.]),
                MatrixXx3::from_vec(vec![0., 1., 0.]),
                MatrixXx3::from_vec(vec![0., 0., 1.]),
            ],
        };
        let projected_rays = pos_hist.project_to_plane(Vector3::new(1., 0., 0.)).unwrap();
        assert_eq!(projected_rays[0][(0, 0)], 0.);
        assert_eq!(projected_rays[0][(0, 1)], 0.);
        assert_eq!(projected_rays[1][(0, 0)], 0.);
        assert_eq!(projected_rays[1][(0, 1)], 1.);
        assert_eq!(projected_rays[2][(0, 0)], 1.);
        assert_eq!(projected_rays[2][(0, 1)], 0.);

        let projected_rays = pos_hist.project_to_plane(Vector3::new(0., 1., 0.)).unwrap();
        assert_eq!(projected_rays[0][(0, 0)], 0.);
        assert_eq!(projected_rays[0][(0, 1)], 1.);
        assert_eq!(projected_rays[1][(0, 0)], 0.);
        assert_eq!(projected_rays[1][(0, 1)], 0.);
        assert_eq!(projected_rays[2][(0, 0)], 1.);
        assert_eq!(projected_rays[2][(0, 1)], 0.);

        let projected_rays = pos_hist.project_to_plane(Vector3::new(0., 0., 1.)).unwrap();
        assert_eq!(projected_rays[0][(0, 0)], 1.);
        assert_eq!(projected_rays[0][(0, 1)], 0.);
        assert_eq!(projected_rays[1][(0, 0)], 0.);
        assert_eq!(projected_rays[1][(0, 1)], 1.);
        assert_eq!(projected_rays[2][(0, 0)], 0.);
        assert_eq!(projected_rays[2][(0, 1)], 0.);
    }
    #[test]
    fn get_wavefront_data_in_units_of_wvl() {
        //empty rays vector
        let rays = Rays::from(Vec::<Ray>::new());
        let wf_data = rays.get_wavefront_data_in_units_of_wvl(true, Length::new::<nanometer>(10.));
        assert!(wf_data.is_err());

        let mut rays = Rays::new_hexapolar_point_source(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Angle::new::<degree>(90.),
            5,
            Length::new::<nanometer>(1000.),
            Energy::new::<joule>(1.),
        )
        .unwrap();

        rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
        let _ = rays.propagate_along_z();
        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(true, Length::new::<nanometer>(10.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 1);

        rays.add_ray(
            Ray::new(
                Point3::new(Length::zero(), Length::zero(), Length::zero()),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1005.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
        );

        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, Length::new::<nanometer>(10.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 1);

        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, Length::new::<nanometer>(3.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 2);
        rays.add_ray(
            Ray::new(
                Point3::new(Length::zero(), Length::zero(), Length::zero()),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1007.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
        );

        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, Length::new::<nanometer>(3.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 3);
    }
    #[test]
    fn wavefront_error_at_pos_in_units_of_wvl() {
        let mut rays = Rays::new_hexapolar_point_source(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Angle::new::<degree>(90.),
            1,
            Length::new::<nanometer>(1000.),
            Energy::new::<joule>(1.),
        )
        .unwrap();
        rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
        let _ = rays.propagate_along_z();

        let wf_error = rays.wavefront_error_at_pos_in_units_of_wvl(Length::new::<nanometer>(1000.));

        for (i, val) in wf_error.column(2).iter().enumerate() {
            if i != 0 {
                assert!((val - (1. - f64::sqrt(2.)) * 10000.).abs() < f64::EPSILON * val.abs())
            } else {
                assert!(val.abs() < f64::EPSILON)
            }
        }
        let mut rays = Rays::new_hexapolar_point_source(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Angle::new::<degree>(90.),
            1,
            Length::new::<nanometer>(500.),
            Energy::new::<joule>(1.),
        )
        .unwrap();
        rays.set_dist_to_next_surface(Length::new::<millimeter>(10.));
        let _ = rays.propagate_along_z();

        let wf_error = rays.wavefront_error_at_pos_in_units_of_wvl(Length::new::<nanometer>(500.));

        for (i, val) in wf_error.column(2).iter().enumerate() {
            if i != 0 {
                assert!((val - (1. - f64::sqrt(2.)) * 20000.).abs() < f64::EPSILON * val.abs())
            } else {
                assert!(val.abs() < f64::EPSILON)
            }
        }
    }
    #[test]
    fn get_xy_rays_pos_test() {
        let rays = Rays::new_hexapolar_point_source(
            Point3::new(Length::zero(), Length::zero(), Length::zero()),
            Angle::new::<degree>(90.),
            1,
            Length::new::<nanometer>(1000.),
            Energy::new::<joule>(1.),
        )
        .unwrap();

        let xy_pos = rays.get_xy_rays_pos();
        for val in xy_pos.row_iter() {
            assert!(val[(0, 0)].abs() < f64::EPSILON);
            assert!(val[(0, 1)].abs() < f64::EPSILON);
        }

        let pos_xy = MatrixXx2::from_vec(vec![1., 2., -10., -2000., 1., 2., -10., -2000.]);

        let ray_vec = vec![
            Ray::new(
                Point3::new(
                    Length::new::<millimeter>(1.),
                    Length::new::<millimeter>(1.),
                    Length::zero(),
                ),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1000.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
            Ray::new(
                Point3::new(
                    Length::new::<millimeter>(2.),
                    Length::new::<millimeter>(2.),
                    Length::zero(),
                ),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1000.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
            Ray::new(
                Point3::new(
                    Length::new::<millimeter>(-10.),
                    Length::new::<millimeter>(-10.),
                    Length::zero(),
                ),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1000.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
            Ray::new(
                Point3::new(
                    Length::new::<millimeter>(-2000.),
                    Length::new::<millimeter>(-2000.),
                    Length::zero(),
                ),
                Vector3::new(0., 1., 0.),
                Length::new::<nanometer>(1000.),
                Energy::new::<joule>(1.),
            )
            .unwrap(),
        ];

        let rays = Rays::from(ray_vec);
        let xy_pos = rays.get_xy_rays_pos();

        for (val_is, val_got) in izip!(pos_xy.row_iter(), xy_pos.row_iter()) {
            assert!((val_is[(0, 0)] - val_got[(0, 0)]).abs() < f64::EPSILON * val_is[(0, 0)].abs());
            assert!((val_is[(0, 1)] - val_got[(0, 1)]).abs() < f64::EPSILON * val_is[(0, 1)].abs());
        }
    }
}
