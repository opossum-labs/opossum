#![warn(missing_docs)]
//! Module for handling bundles of [`Ray`]s
use crate::{
    aperture::Aperture,
    degree,
    energy_distributions::EnergyDistribution,
    error::{OpmResult, OpossumError},
    joule, micrometer, millimeter, nanometer,
    nodes::{
        ray_propagation_visualizer::{RayPositionHistories, RayPositionHistorySpectrum},
        FilterType, FluenceData, WaveFrontData, WaveFrontErrorMap,
    },
    plottable::AxLims,
    position_distributions::{Hexapolar, PositionDistribution},
    properties::Proptype,
    ray::{Ray, SplittingConfig},
    refractive_index::RefractiveIndexType,
    spectrum::Spectrum,
    surface::Surface,
    utils::{
        filter_data::{get_min_max_filter_nonfinite, get_unique_finite_values},
        griddata::{
            calc_closed_poly_area, create_linspace_axes, create_voronoi_cells,
            interpolate_3d_triangulated_scatter_data, VoronoiedData,
        },
    },
};

use approx::relative_eq;
use itertools::izip;
use kahan::KahanSummator;
use log::warn;
use nalgebra::{distance, DVector, MatrixXx2, MatrixXx3, Point2, Point3, Vector2, Vector3};
use num::ToPrimitive;
use serde_derive::{Deserialize, Serialize};
use std::fmt::Display;
use std::ops::Add;
use std::ops::Range;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::{micrometer, millimeter, nanometer};
use uom::{num_traits::Zero, si::f64::Area};

/// Struct containing all relevant information of a ray bundle
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rays {
    /// vector containing the individual rays
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
    /// Generate a set of collimated rays (collinear with optical axis) with uniform energy distribution.
    ///
    /// This functions generates a bundle of (collimated) rays of the given wavelength and the given *total* energy. The energy is
    /// evenly distributed over the indivual rays. The ray positions are distributed according to the given [`PositionDistribution`].
    ///
    /// If the given size id zero, a bundle consisting of a single ray along the optical - position (0.0,0.0,0.0) - axis is generated.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    ///  - the given wavelength is <= 0.0, NaN or +inf
    ///  - the given energy is <= 0.0, NaN or +inf
    ///  - the given size is < 0.0, NaN or +inf
    pub fn new_uniform_collimated(
        wave_length: Length,
        energy: Energy,
        strategy: &dyn PositionDistribution,
    ) -> OpmResult<Self> {
        let points = strategy.generate();
        let nr_of_rays = points.len();
        let mut rays: Vec<Ray> = Vec::with_capacity(nr_of_rays);
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

    /// Generate a set of collimated rays (collinear with optical axis) with specified energy distribution and position distribution.
    ///
    /// This functions generates a bundle of (collimated) rays of the given wavelength and the given *total* energy. The energy is
    /// distributed according to the specified distribution function over the indivual rays: [`EnergyDistribution`]. The ray positions are distributed according to the given [`PositionDistribution`].
    ///  
    /// This function returns an error if
    /// # Errors
    ///  - the given wavelength is <= 0.0, NaN or +inf
    ///  - the given energy is <= 0.0, NaN or +inf
    ///  - the given size is < 0.0, NaN or +inf
    pub fn new_collimated(
        wave_length: Length,
        energy_strategy: &dyn EnergyDistribution,
        pos_strategy: &dyn PositionDistribution,
    ) -> OpmResult<Self> {
        let ray_pos = pos_strategy.generate();

        //currently the energy distribution only works in the x-y plane. therefore, all points are projected to this plane
        let ray_pos_plane = ray_pos
            .iter()
            .map(|p| Point2::<f64>::new(p.x.get::<millimeter>(), p.y.get::<millimeter>()))
            .collect::<Vec<Point2<f64>>>();
        //apply distribution strategy
        let ray_energies = energy_strategy.apply(&ray_pos_plane);

        //sum up energy of rays that are valid: energy is larger than machine epsilon times total energy
        let min_energy = f64::EPSILON * energy_strategy.get_total_energy();
        let total_energy_valid_rays = joule!(ray_energies
            .iter()
            .map(|e| {
                if *e > min_energy {
                    e.get::<joule>()
                } else {
                    0.
                }
            })
            .collect::<Vec<f64>>()
            .iter()
            .kahan_sum()
            .sum());
        //scaling factor if a significant amount of enery has been lost
        let energy_scale_factor = energy_strategy.get_total_energy() / total_energy_valid_rays;

        //create rays
        let nr_of_rays = ray_pos.len();
        let mut rays: Vec<Ray> = Vec::<Ray>::with_capacity(nr_of_rays);
        for (pos, energy) in izip!(ray_pos.iter(), ray_energies.iter()) {
            if *energy > f64::EPSILON * energy_strategy.get_total_energy() {
                let ray = Ray::new_collimated(*pos, wave_length, *energy * energy_scale_factor)?;
                rays.push(ray);
            }
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
    /// The parameter `number_of_rings` determines the "density" of the [`Hexapolar`] pattern (see docs there). If the cone angle is zero, a ray bundle
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
        if cone_angle.is_sign_negative() || cone_angle >= degree!(180.0) {
            return Err(OpossumError::Other(
                "cone angle must be within (0.0..180.0) degrees range".into(),
            ));
        }
        let size_after_unit_length = (cone_angle / 2.0).tan().value;
        let points: Vec<Point3<Length>> = if cone_angle.is_zero() {
            vec![millimeter!(0., 0., 0.)]
        } else {
            Hexapolar::new(millimeter!(size_after_unit_length), nr_of_rings)?.generate()
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
    /// This function returns the sum of all `valid` individual [`Ray`] energies.
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        let energies: Vec<f64> = self
            .rays
            .iter()
            .filter(|r| r.valid())
            .map(|r| r.energy().get::<joule>())
            .collect();
        let kahan_sum: kahan::KahanSum<f64> = energies.iter().kahan_sum();
        joule!(kahan_sum.sum())
    }
    /// Returns the number of rays of this [`Rays`].
    ///
    /// The given switch determines wehther all [`Ray`]s or only `valid` [`Ray`]s will be counted.
    #[must_use]
    pub fn nr_of_rays(&self, valid_only: bool) -> usize {
        self.rays
            .iter()
            .filter(|r| r.valid() || !valid_only)
            .count()
    }
    /// Returns the iterator of this [`Rays`].
    pub fn iter(&self) -> std::slice::Iter<'_, Ray> {
        self.rays.iter()
    }

    /// Returns the mutable iterator of this [`Rays`].
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Ray> {
        self.rays.iter_mut()
    }
    /// Apodize (cut out or attenuate) the ray bundle by a given [`Aperture`].
    ///
    /// This function only affects `valid` [`Ray`]s in the bundle.
    /// # Errors
    ///
    /// This function returns an error if a single ray cannot be propery apodized (e.g. filter factor outside (0.0..=1.0)).
    pub fn apodize(&mut self, aperture: &Aperture) -> OpmResult<()> {
        for ray in &mut self.rays {
            if ray.valid() {
                let ap_factor = aperture.apodization_factor(&ray.position().xy());
                if ap_factor > 0.0 {
                    ray.filter_energy(&FilterType::Constant(ap_factor))?;
                } else {
                    ray.add_to_pos_hist(ray.position());
                    ray.set_invalid();
                }
            }
        }
        Ok(())
    }
    /// Finds all unique wavelengths in this raybundle and returns them in a vector
    #[must_use]
    pub fn get_unique_wavelengths(&self, valid_only: bool) -> Vec<Length> {
        //get all wavelengths of the rays converted to nm
        let wvls = self
            .rays
            .iter()
            .filter(|&r| (r.valid() || !valid_only))
            .map(|r| r.wavelength().get::<nanometer>())
            .collect::<Vec<f64>>();

        //get unique wavelengths
        let unique_wvls = get_unique_finite_values(wvls.as_slice());

        //return as Vec<Length>
        unique_wvls
            .iter()
            .map(|w| nanometer!(*w))
            .collect::<Vec<Length>>()
    }
    /// Returns the centroid of this [`Rays`].
    ///
    /// This functions returns the centroid of the positions (`valid` [`Ray`]s only) of this ray bundle. The
    /// function returns `None` if [`Rays`] is empty.
    #[must_use]
    pub fn centroid(&self) -> Option<Point3<Length>> {
        #[allow(clippy::cast_precision_loss)]
        let len = self.nr_of_rays(true) as f64;
        if len == 0.0 {
            return None;
        }
        let c = self.rays.iter().filter(|r| r.valid()).fold(
            (Length::zero(), Length::zero(), Length::zero()),
            |c, r| {
                let pos = r.position();
                (c.0 + pos.x, c.1 + pos.y, c.2 + pos.z)
            },
        );
        Some(Point3::new(c.0 / len, c.1 / len, c.2 / len))
    }
    /// Returns the energy-weighted centroid of this [`Rays`].
    ///
    /// This functions returns the energy-weighted centroid of the positions (`valid` [`Ray`]s only) of this ray bundle. The
    /// function returns `None` if [`Rays`] is empty.
    #[must_use]
    pub fn energy_weighted_centroid(&self) -> Option<Point3<Length>> {
        #[allow(clippy::cast_precision_loss)]
        let len = self.nr_of_rays(true);
        if len == 0 {
            return None;
        }
        let c = self.rays.iter().filter(|r| r.valid()).fold(
            (Length::zero(), Length::zero(), Length::zero(), 0.),
            |c, r| {
                let pos = r.position();
                let energy = r.energy().get::<joule>();
                (
                    c.0 + pos.x * energy,
                    c.1 + pos.y * energy,
                    c.2 + pos.z * energy,
                    c.3 + energy,
                )
            },
        );
        Some(Point3::new(c.0 / c.3, c.1 / c.3, c.2 / c.3))
    }
    /// Returns the geometric beam radius [`Rays`].
    ///
    /// This function calculates the maximum distance of a ray bundle (`valid` [`Ray`]s only ) from its centroid.
    #[must_use]
    pub fn beam_radius_geo(&self) -> Option<Length> {
        self.centroid().map(|c| {
            let c_in_millimeter = Point2::new(c.x.get::<millimeter>(), c.y.get::<millimeter>());
            let mut max_dist = 0.0;
            for ray in &self.rays {
                if ray.valid() {
                    let ray_2d = Point2::new(
                        ray.position().x.get::<millimeter>(),
                        ray.position().y.get::<millimeter>(),
                    );
                    let dist = distance(&ray_2d, &c_in_millimeter);
                    if dist > max_dist {
                        max_dist = dist;
                    }
                }
            }
            millimeter!(max_dist)
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
                if ray.valid() {
                    let ray_2d = Point2::new(
                        ray.position().x.get::<millimeter>(),
                        ray.position().y.get::<millimeter>(),
                    );
                    sum_dist_sq += distance(&ray_2d, &c_in_millimeter).powi(2);
                }
            }
            #[allow(clippy::cast_precision_loss)]
            let nr_of_rays = self.nr_of_rays(true) as f64;
            sum_dist_sq /= nr_of_rays;
            millimeter!(sum_dist_sq.sqrt())
        })
    }

    /// Returns the rms beam radius [`Rays`].
    ///
    /// This function calculates the rms (root mean square) size of a ray bundle from it centroid. So far, the rays / spots are not weighted by their
    /// particular energy.
    #[must_use]
    pub fn energy_weighted_beam_radius_rms(&self) -> Option<Length> {
        self.energy_weighted_centroid().map(|c| {
            let mut sum_dist_sq = Area::zero();
            for ray in self.rays.iter().filter(|r| r.valid()) {
                let dist = (c.x - ray.position().x) * (c.x - ray.position().x)
                    + (c.y - ray.position().y) * (c.y - ray.position().y);
                sum_dist_sq += dist * ray.energy().get::<joule>();
            }
            sum_dist_sq /= self.total_energy().get::<joule>();
            sum_dist_sq.sqrt()
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
                            .wavefront_error_at_pos_in_units_of_wvl(micrometer!(wvl)),
                        micrometer!(wvl),
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
        let mut wave_front_err = MatrixXx3::from_element(self.nr_of_rays(true), 0.);
        let mut min_radius = f64::INFINITY;
        let mut path_length_at_center = 0.;
        for (i, ray) in self.rays.iter().filter(|r| r.valid()).enumerate() {
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
    ///
    /// The `valid_only` switch determines if all [`Ray`]s or only `valid` [`Ray`]s will be returned.
    #[must_use]
    pub fn get_xy_rays_pos(&self, valid_only: bool) -> MatrixXx2<f64> {
        let mut rays_at_pos = MatrixXx2::from_element(self.nr_of_rays(valid_only), 0.);
        for (row, ray) in self
            .rays
            .iter()
            .filter(|r| !valid_only || r.valid())
            .enumerate()
        {
            rays_at_pos[(row, 0)] = ray.position().x.get::<millimeter>();
            rays_at_pos[(row, 1)] = ray.position().y.get::<millimeter>();
        }
        rays_at_pos
    }

    fn calc_ray_fluence_in_voronoi_cells(
        &self,
        projected_ray_pos: &MatrixXx2<f64>,
        proj_ax1_lim: AxLims,
        proj_ax2_lim: AxLims,
    ) -> OpmResult<VoronoiedData> {
        let voronoi = create_voronoi_cells(projected_ray_pos, &proj_ax1_lim, &proj_ax2_lim)
            .map_err(|_| {
                OpossumError::Other(
                    "Voronoi diagram for fluence estimation could not be created!".into(),
                )
            })?;

        //get the voronoi cells
        let v_cells = voronoi.cells();

        let mut fluence_scatter = DVector::from_element(voronoi.sites.len(), 0.);

        for idx in 0..self.nr_of_rays(true) {
            let v_neighbours = v_cells[idx]
                .points()
                .iter()
                .map(|p| Point2::new(p.x, p.y))
                .collect::<Vec<Point2<f64>>>();
            let poly_area = calc_closed_poly_area(&v_neighbours)?;
            fluence_scatter[idx] = self.rays[idx].energy().get::<joule>() / poly_area;
        }

        VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence_scatter)
    }

    /// Calculates the spatial energy distribution (fluence) of a ray bundle, its coordinates in a plane transversal to its propagation diraction and the peak fluence in J/cm²
    /// # Errors
    /// This function errors if
    /// - creation of hte linearly spaced axes fails
    /// - voronating the ray position or the fluence calculation in the voronoi cells fails
    /// - interpolation fails
    pub fn calc_fluence_at_position(&self) -> OpmResult<FluenceData> {
        let num_axes_points = 100.;

        // get ray positions
        let rays_pos_vec = self.get_xy_rays_pos(true) / 10.; //for centimeter;

        //axes definition
        let (co_ax1, co_ax1_lim) = create_linspace_axes(rays_pos_vec.column(0), num_axes_points)?;
        let (co_ax2, co_ax2_lim) = create_linspace_axes(rays_pos_vec.column(1), num_axes_points)?;

        // calculate the fluence of each ray by linking the ray energy with the area of its voronoi cell
        let voronoi_fluence_scatter =
            self.calc_ray_fluence_in_voronoi_cells(&rays_pos_vec, co_ax1_lim, co_ax2_lim)?;

        //currently only interpolation. voronoid data for plotting must still be implemented
        let (interp_fluence, interp_mask) =
            interpolate_3d_triangulated_scatter_data(&voronoi_fluence_scatter, &co_ax1, &co_ax2)?;

        let (peak_fluence, mut average) = izip!(
            interp_fluence.into_iter(),
            interp_mask.into_iter()
        )
        .fold((f64::NEG_INFINITY, 0.), |arg0, v| {
            let max_val = if v.0.is_nan() {
                arg0.0
            } else {
                f64::max(arg0.0, *v.0)
            };
            let average_val = if v.1 > &f64::EPSILON {
                f64::add(arg0.1, *v.0)
            } else {
                arg0.1
            };
            (max_val, average_val)
        });

        average /= interp_mask.sum();

        Ok(FluenceData::new(
            peak_fluence,
            average,
            interp_fluence,
            co_ax1,
            co_ax2,
        ))
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
    /// This function propgatates all valid [`Ray`]s along the z axis. The propagation length must be
    /// set with the function `set_dist_to_next_surface`.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the z component of a ray direction is zero.
    ///  - the given length is not finite.
    pub fn propagate_along_z(&mut self) -> OpmResult<()> {
        if !self.dist_to_next_surface.is_zero() {
            for ray in &mut self.rays {
                if ray.valid() {
                    ray.propagate_along_z(self.dist_to_next_surface)?;
                }
            }
            self.z_position += self.dist_to_next_surface;
            self.set_dist_zero();
        }
        Ok(())
    }
    /// Refract a ray bundle on a paraxial surface of given focal length.
    ///
    /// This function refracts all valid [`Ray`]s.
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
            if ray.valid() {
                ray.refract_paraxial(focal_length)?;
            }
        }
        Ok(())
    }
    /// Refract a ray bundle on a [`Surface`].
    ///
    /// This function refracts all `valid` [`Ray`]s on a given surface.
    ///
    /// # Warnings
    ///
    /// This functions emits a warning of no valid [`Ray`]s are found in the bundle.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn refract_on_surface(
        &mut self,
        surface: &dyn Surface,
        refractive_index: &RefractiveIndexType,
    ) -> OpmResult<()> {
        let mut valid_rays_found = false;
        let mut rays_missed = false;
        for ray in &mut self.rays {
            if ray.valid() {
                let n2 = refractive_index.get_refractive_index(ray.wavelength());
                if ray.refract_on_surface(surface, n2)?.is_none() {
                    rays_missed = true;
                };
                valid_rays_found = true;
            }
        }
        self.z_position += self.dist_to_next_surface;
        self.set_dist_zero();
        if rays_missed {
            warn!("rays missed a surface");
        }
        if !valid_rays_found {
            warn!("ray bundle contains no valid rays - not propagating");
        }
        Ok(())
    }
    /// Filter a ray bundle by a given filter.
    ///
    /// Filter the energy of of all `valid` rays by a given [`FilterType`].
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
            if (*ray).valid() {
                ray.filter_energy(filter)?;
            }
        }
        Ok(())
    }
    /// Invalidate all [`Ray`]s below a given energy threshold.
    ///
    /// Sets all rays with an energy (per ray) below the given threshold to the `invalid` state.
    ///
    /// # Warnings
    ///
    /// This function emits a warning log entry if the given threshold is negative. In this case the ray bundle is not modified.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given energy threshold is not finite.
    pub fn invalidate_by_threshold_energy(&mut self, min_energy_per_ray: Energy) -> OpmResult<()> {
        if min_energy_per_ray.is_sign_negative() {
            warn!("negative threshold energy given. Ray bundle unmodified.");
            return Ok(());
        }
        if !min_energy_per_ray.is_finite() {
            return Err(OpossumError::Other(
                "threshold energy must be finite".into(),
            ));
        };
        let _ = self
            .rays
            .iter_mut()
            .filter(|r| r.energy() < min_energy_per_ray)
            .map(Ray::set_invalid)
            .count();
        Ok(())
    }
    /// Returns the wavelength range of this [`Rays`].
    ///
    /// This functions returns the minimum and maximum wavelength of the containing `valid` rays as `Range`. If [`Rays`] is empty, `None` is returned.
    #[must_use]
    pub fn wavelength_range(&self) -> Option<Range<Length>> {
        if self.rays.is_empty() {
            return None;
        };
        let mut min = millimeter!(f64::INFINITY);
        let mut max = Length::zero();
        for ray in &self.rays {
            if ray.valid() {
                let w = ray.wavelength();
                if w > max {
                    max = w;
                }
                if w < min {
                    min = w;
                }
            }
        }
        Some(min..max)
    }
    /// Create a [`Spectrum`] (with a given resolution) from a ray bundle.
    ///
    /// This functions creates a spectrum by adding all individual `valid` rays from ray bundle with
    /// respect to their particular wavelength and energy.
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
            if ray.valid() {
                spectrum.add_single_peak(ray.wavelength(), ray.energy().get::<joule>())?;
            }
        }
        Ok(spectrum)
    }
    /// Set the refractive index of the medium all [`Rays`] are propagating in.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive index is < 1.0 or not finite.
    pub fn set_refractive_index(
        &mut self,
        refractive_index: &RefractiveIndexType,
    ) -> OpmResult<()> {
        if self.nr_of_rays(true).is_zero() {
            warn!("ray bundle contains no valid rays for setting the refractive index");
        } else {
            for ray in &mut self.rays {
                if ray.valid() {
                    ray.set_refractive_index(
                        refractive_index.get_refractive_index(ray.wavelength()),
                    )?;
                }
            }
        }
        Ok(())
    }
    /// Split a ray bundle
    ///
    /// This function splits a ray bundle determined by the given [`SplittingConfig`]. See [`split`](Ray::split) function for details.
    /// **Note**: Only `valid`[`Ray`]s in the bunlde will be affected.
    /// # Errors
    ///
    /// This function will return an error if the underlying split function for a single ray returns an error.
    pub fn split(&mut self, config: &SplittingConfig) -> OpmResult<Self> {
        let mut split_rays = Self {
            z_position: self.absolute_z_of_last_surface(),
            ..Default::default()
        };
        for ray in &mut self.rays {
            if ray.valid() {
                let split_ray = ray.split(config)?;
                split_rays.add_ray(split_ray);
            } else {
                split_rays.add_ray(ray.clone());
            }
        }
        Ok(split_rays)
    }
    /// Merge two ray bundles.
    ///
    /// This function simply adds the given rays to the existing ray bundle.
    /// **Note**: The temporarily introduced "absolute z position" is taken from the ray bundle with the maximum position....We have
    /// to work on that...
    pub fn merge(&mut self, rays: &Self) {
        let max_z_position = self
            .absolute_z_of_last_surface()
            .max(rays.absolute_z_of_last_surface());
        for ray in &rays.rays {
            self.add_ray(ray.clone());
        }
        self.z_position = max_z_position;
    }

    /// Split an existing ray bundle into multiple ray bundles corresponding to their wavelength
    /// # Attributes
    /// - `wavelength_bin_size`: size of the wavelength binning
    ///
    /// If there is only one wavelength, the same ray bundle is returned
    /// # Errors
    /// This function errors if the minimum wavelength of the unique wavelengths can not be calculated. Normally, this cannot happen, since the wavlengths of a ray are finite from begin with.
    pub fn split_ray_bundle_by_wavelength(
        &self,
        wavelength_bin_size: Length,
        valid_only: bool,
    ) -> OpmResult<(Vec<Self>, Vec<Length>)> {
        let unique_wavelengths = self.get_unique_wavelengths(valid_only);
        let num_split_bundles = unique_wavelengths.len();
        if num_split_bundles == 1 {
            Ok((vec![self.clone()], unique_wavelengths))
        } else if num_split_bundles == 0 {
            Err(OpossumError::Other(
                "No rays in this bundle! Cannot split ray bundle by wavelengths!".into(),
            ))
        } else {
            //sort wavelengths
            //get "start" wavelength: smallest wavelength reduced by half a bin size
            let (start_wvl_f64, start_wvl) = if let Some((min, _)) = get_min_max_filter_nonfinite(
                unique_wavelengths
                    .iter()
                    .map(uom::si::f64::Length::get::<nanometer>)
                    .collect::<Vec<f64>>()
                    .as_slice(),
            ) {
                Ok((
                    min - wavelength_bin_size.get::<nanometer>() / 2.,
                    nanometer!(min),
                ))
            } else {
                Err(OpossumError::Other(
                    "Wavelength of ray is not finite! Cannot split ray bundle by wavelengths!"
                        .into(),
                ))
            }?;

            //for calculation, get bin size in units instehat of length quantity
            let bin_size: f64 = wavelength_bin_size.get::<nanometer>();

            //initialize vectors
            let mut ray_bundles = Vec::<Self>::with_capacity(num_split_bundles);
            let mut ray_center_wavelength = Vec::<Length>::with_capacity(num_split_bundles);

            //loop over all rays and sort them into a new ray bundle according to their wavelengths
            for ray in self.rays.iter().filter(|r| r.valid() || !valid_only) {
                let r_wvl = ray.wavelength().get::<nanometer>();
                let insertion_index = ((r_wvl - start_wvl_f64) / bin_size).floor();
                if ray_bundles.is_empty() {
                    ray_bundles.push(Self::from(vec![ray.clone()]));
                    ray_center_wavelength.push(start_wvl + insertion_index * wavelength_bin_size);
                } else {
                    let len_bundles = ray_bundles.len();
                    for (i, bundle) in ray_bundles.clone().iter().enumerate() {
                        let bundle_wvl = bundle.rays[0].wavelength().get::<nanometer>();
                        let insertion_index_bundle =
                            ((bundle_wvl - start_wvl_f64) / bin_size).floor();
                        if relative_eq!(insertion_index_bundle, insertion_index) {
                            ray_bundles[i].add_ray(ray.clone());
                            break;
                        } else if insertion_index < insertion_index_bundle {
                            ray_bundles.insert(i, Self::from(vec![ray.clone()]));
                            ray_center_wavelength
                                .insert(i, start_wvl + insertion_index * wavelength_bin_size);
                            break;
                        } else if i == len_bundles - 1 {
                            ray_bundles.push(Self::from(vec![ray.clone()]));
                            ray_center_wavelength
                                .push(start_wvl + insertion_index * wavelength_bin_size);
                        }
                    }
                }
            }

            Ok((ray_bundles, ray_center_wavelength))
        }
    }

    /// Get the position history of all rays in this ray bundle
    ///
    /// # Returns
    /// This method returns a vector of N-row x 3 column matrices that contain the position history of all the rays
    /// # Errors
    /// This function errors when the splitting of the rays by their wavelengths fails. For more info see `split_ray_bundle_by_wavelength`
    pub fn get_rays_position_history(&self) -> OpmResult<RayPositionHistories> {
        let (rays_by_wavelength, wavelengths) =
            self.split_ray_bundle_by_wavelength(nanometer!(1.), false)?;

        let mut ray_pos_hists = Vec::<RayPositionHistorySpectrum>::with_capacity(wavelengths.len());
        for (ray_bundle, wvl) in izip!(rays_by_wavelength, wavelengths) {
            let mut rays_pos_history =
                Vec::<MatrixXx3<Length>>::with_capacity(ray_bundle.rays.len());
            for ray in &ray_bundle {
                rays_pos_history.push(ray.position_history());
            }
            ray_pos_hists.push(RayPositionHistorySpectrum::new(
                rays_pos_history,
                wvl,
                nanometer!(1.),
            )?);
        }

        Ok(RayPositionHistories {
            rays_pos_history: ray_pos_hists,
        })
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

impl Display for Rays {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ray in self {
            let _ = writeln!(f, "{ray}");
        }
        write!(f, "# of rays: {:?}", self.nr_of_rays(false))
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

impl<'a> IntoIterator for &'a mut Rays {
    type IntoIter = std::slice::IterMut<'a, Ray>;
    type Item = &'a mut Ray;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl TryFrom<Rays> for Proptype {
    type Error = OpossumError;
    fn try_from(value: Rays) -> OpmResult<Self> {
        Ok(Self::RayPositionHistory(value.get_rays_position_history()?))
    }
}

impl<'a> IntoIterator for &'a Rays {
    type IntoIter = std::slice::Iter<'a, crate::ray::Ray>;
    type Item = &'a crate::ray::Ray;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        aperture::CircleConfig,
        centimeter,
        energy_distributions::General2DGaussian,
        joule, millimeter, nanometer,
        position_distributions::{FibonacciEllipse, FibonacciRectangle, Hexapolar, Random},
        radian,
        ray::SplittingConfig,
        refractive_index::RefrIndexConst,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use itertools::izip;
    use log::Level;
    use testing_logger;
    use uom::si::{energy::joule, length::nanometer};

    #[test]
    fn split_ray_bundle_by_wavelength_test() {
        let mut rays_1w = Rays::new_uniform_collimated(
            nanometer!(1053.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(2.), millimeter!(2.), 5).unwrap(),
        )
        .unwrap();

        let mut rays_2w = Rays::new_uniform_collimated(
            nanometer!(527.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(1.3), millimeter!(1.3), 10).unwrap(),
        )
        .unwrap();

        let mut rays_3w = Rays::new_uniform_collimated(
            nanometer!(1053. / 3.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(0.5), millimeter!(0.5), 15).unwrap(),
        )
        .unwrap();

        rays_1w.add_rays(&mut rays_2w);
        rays_1w.add_rays(&mut rays_3w);

        let mut ray_bundle = rays_1w;

        let (split_bundles, wavelengths) = ray_bundle
            .split_ray_bundle_by_wavelength(nanometer!(1.), true)
            .unwrap();

        assert_eq!(wavelengths.len(), 3);
        assert!(relative_eq!(
            wavelengths[0].get::<nanometer>(),
            1053. / 3.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[1].get::<nanometer>(),
            527.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[2].get::<nanometer>(),
            1053.,
            max_relative = 2. * f64::EPSILON
        ));

        assert_eq!(split_bundles[0].rays.len(), 15);
        assert_eq!(split_bundles[1].rays.len(), 10);
        assert_eq!(split_bundles[2].rays.len(), 5);

        ray_bundle.rays[0].set_invalid();
        ray_bundle.rays[5].set_invalid();
        ray_bundle.rays[20].set_invalid();

        let (split_bundles, wavelengths) = ray_bundle
            .split_ray_bundle_by_wavelength(nanometer!(1.), true)
            .unwrap();

        assert_eq!(wavelengths.len(), 3);
        assert!(relative_eq!(
            wavelengths[0].get::<nanometer>(),
            1053. / 3.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[1].get::<nanometer>(),
            527.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[2].get::<nanometer>(),
            1053.,
            max_relative = 2. * f64::EPSILON
        ));

        assert_eq!(split_bundles[0].rays.len(), 14);
        assert_eq!(split_bundles[1].rays.len(), 9);
        assert_eq!(split_bundles[2].rays.len(), 4);

        let (split_bundles, wavelengths) = ray_bundle
            .split_ray_bundle_by_wavelength(nanometer!(400.), true)
            .unwrap();

        assert_eq!(wavelengths.len(), 2);
        assert!(relative_eq!(
            wavelengths[0].get::<nanometer>(),
            1053. / 3.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[1].get::<nanometer>(),
            1151.,
            max_relative = 2. * f64::EPSILON
        ));

        assert_eq!(split_bundles[0].rays.len(), 23);
        assert_eq!(split_bundles[1].rays.len(), 4);

        let (split_bundles, wavelengths) = ray_bundle
            .split_ray_bundle_by_wavelength(nanometer!(400.), false)
            .unwrap();

        assert_eq!(wavelengths.len(), 2);
        assert!(relative_eq!(
            wavelengths[0].get::<nanometer>(),
            1053. / 3.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            wavelengths[1].get::<nanometer>(),
            1151.,
            max_relative = 2. * f64::EPSILON
        ));

        assert_eq!(split_bundles[0].rays.len(), 25);
        assert_eq!(split_bundles[1].rays.len(), 5);
    }
    #[test]
    fn get_unique_wavelengths() {
        let mut rays_1w = Rays::new_uniform_collimated(
            nanometer!(1053.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(2.), millimeter!(2.), 5).unwrap(),
        )
        .unwrap();

        let mut rays_2w = Rays::new_uniform_collimated(
            nanometer!(527.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(1.3), millimeter!(1.3), 10).unwrap(),
        )
        .unwrap();

        let mut rays_3w = Rays::new_uniform_collimated(
            nanometer!(1053. / 3.),
            joule!(1.),
            &FibonacciEllipse::new(millimeter!(0.5), millimeter!(0.5), 15).unwrap(),
        )
        .unwrap();

        rays_1w.add_rays(&mut rays_2w);
        rays_1w.add_rays(&mut rays_3w);

        let unique = rays_1w.get_unique_wavelengths(true);
        assert_eq!(unique.len(), 3);
        assert!(relative_eq!(
            unique[2].get::<nanometer>(),
            351.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            unique[1].get::<nanometer>(),
            527.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            unique[0].get::<nanometer>(),
            1053.,
            max_relative = 2. * f64::EPSILON
        ));

        rays_1w.rays[0].set_invalid();
        rays_1w.rays[1].set_invalid();
        rays_1w.rays[2].set_invalid();
        rays_1w.rays[3].set_invalid();
        rays_1w.rays[4].set_invalid();

        let unique = rays_1w.get_unique_wavelengths(true);
        assert_eq!(unique.len(), 2);
        assert!(relative_eq!(
            unique[0].get::<nanometer>(),
            527.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            unique[1].get::<nanometer>(),
            351.,
            max_relative = 2. * f64::EPSILON
        ));

        let unique = rays_1w.get_unique_wavelengths(false);
        assert_eq!(unique.len(), 3);
        assert!(relative_eq!(
            unique[2].get::<nanometer>(),
            351.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            unique[1].get::<nanometer>(),
            527.,
            max_relative = 2. * f64::EPSILON
        ));
        assert!(relative_eq!(
            unique[0].get::<nanometer>(),
            1053.,
            max_relative = 2. * f64::EPSILON
        ));
    }

    #[test]
    fn default() {
        let rays = Rays::default();
        assert_eq!(rays.nr_of_rays(true), 0);
        assert_eq!(rays.nr_of_rays(false), 0);
    }
    #[test]
    fn new_collimated_gaussian() {
        let wvl = nanometer!(1054.0);
        let pos_strategy = &Hexapolar::new(millimeter!(1.0), 2).unwrap();
        let energy_strategy = &General2DGaussian::new(
            joule!(1.),
            Point2::new(0., 0.),
            Point2::new(1., 1.),
            1.,
            radian!(0.),
            true,
        )
        .unwrap();
        let rays = Rays::new_collimated(wvl, energy_strategy, pos_strategy).unwrap();

        assert_relative_eq!(rays.total_energy().get::<joule>(), 1.)
    }
    #[test]
    fn new_uniform_collimated() {
        let wvl = nanometer!(1054.0);
        let energy = joule!(1.0);
        let strategy = &Hexapolar::new(millimeter!(1.0), 2).unwrap();
        let rays = Rays::new_uniform_collimated(wvl, energy, strategy);
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.nr_of_rays(true), 19);
        assert!(Energy::abs(rays.total_energy() - joule!(1.0)) < joule!(10.0 * f64::EPSILON));
    }
    #[test]
    fn new_uniform_collimated_zero() {
        let wvl = nanometer!(1054.0);
        let energy = joule!(1.0);
        let strategy = &Hexapolar::new(Length::zero(), 2).unwrap();
        let rays = Rays::new_uniform_collimated(wvl, energy, strategy);
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.nr_of_rays(true), 1);
        assert_eq!(rays.rays[0].position(), millimeter!(0., 0., 0.));
        assert_eq!(rays.rays[0].direction(), Vector3::z());
    }
    #[test]
    fn new_hexapolar_point_source() {
        let position = millimeter!(0., 0., 0.);
        let wave_length = nanometer!(1053.0);
        let rays =
            Rays::new_hexapolar_point_source(position, degree!(90.0), 1, wave_length, joule!(1.0));

        let mut rays = rays.unwrap();
        for ray in &rays.rays {
            assert_eq!(ray.position(), millimeter!(0., 0., 0.))
        }
        rays.set_dist_to_next_surface(millimeter!(1.0));
        rays.propagate_along_z().unwrap();
        assert_eq!(rays.rays[0].position(), millimeter!(0., 0., 1.));
        assert_eq!(rays.rays[1].position()[0], Length::zero());
        assert_abs_diff_eq!(rays.rays[1].position()[1].value, millimeter!(1.0).value);
        assert_eq!(rays.rays[1].position()[2], millimeter!(1.0));
        assert!(Rays::new_hexapolar_point_source(
            position,
            degree!(-1.0),
            1,
            wave_length,
            joule!(1.0),
        )
        .is_err());
        assert!(Rays::new_hexapolar_point_source(
            position,
            degree!(180.0),
            1,
            wave_length,
            joule!(1.0),
        )
        .is_err());
        assert!(Rays::new_hexapolar_point_source(
            position,
            degree!(1.0),
            1,
            wave_length,
            joule!(-0.1),
        )
        .is_err());
        let rays =
            Rays::new_hexapolar_point_source(position, Angle::zero(), 1, wave_length, joule!(1.0))
                .unwrap();
        assert_eq!(rays.nr_of_rays(false), 1);
        assert_eq!(
            rays.rays[0].position(),
            Point3::new(position.x, position.y, Length::zero())
        );
        assert_eq!(rays.rays[0].direction(), Vector3::z());
    }
    #[test]
    fn add_ray() {
        let mut rays = Rays::default();
        assert_eq!(rays.nr_of_rays(false), 0);
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        assert_eq!(rays.nr_of_rays(false), 1);
    }
    #[test]
    fn add_rays() {
        let mut rays = Rays::default();
        assert_eq!(rays.nr_of_rays(false), 0);
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        assert_eq!(rays.nr_of_rays(false), 1);
        let mut rays2 = rays.clone();
        rays.add_rays(&mut rays2);
        assert_eq!(rays.nr_of_rays(false), 2);
    }
    #[test]
    fn set_refractive_index() {
        testing_logger::setup();
        let mut rays = Rays::default();
        rays.set_refractive_index(&RefractiveIndexType::Const(
            RefrIndexConst::new(1.5).unwrap(),
        ))
        .unwrap();
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "ray bundle contains no valid rays for setting the refractive index"
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        rays.set_refractive_index(&RefractiveIndexType::Const(
            RefrIndexConst::new(2.0).unwrap(),
        ))
        .unwrap();
        assert_eq!(rays.rays[0].refractive_index(), 2.0);
        assert_eq!(rays.rays[1].refractive_index(), 2.0);
    }
    #[test]
    fn total_energy() {
        let mut rays = Rays::default();
        assert!(rays.total_energy().is_zero());
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), joule!(1.0));
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), joule!(2.0));
        let mut ray =
            Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        assert_eq!(rays.total_energy(), joule!(2.0));

        let rays = Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Random::new(millimeter!(1.0), millimeter!(1.0), 100000).unwrap(),
        )
        .unwrap();
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 1.0);
    }
    #[test]
    fn centroid() {
        let mut rays = Rays::default();
        assert_eq!(rays.centroid(), None);
        rays.add_ray(
            Ray::new_collimated(millimeter!(1.0, 2.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(millimeter!(2.0, 3.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap(),
        );
        assert_eq!(rays.centroid().unwrap(), millimeter!(1.5, 2.5, 0.));
        let mut ray =
            Ray::new_collimated(millimeter!(2.0, 3.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        assert_eq!(rays.centroid().unwrap(), millimeter!(1.5, 2.5, 0.));
    }
    #[test]
    fn beam_radius_geo() {
        let mut rays = Rays::default();
        assert!(rays.beam_radius_geo().is_none());
        rays.add_ray(
            Ray::new_collimated(millimeter!(1.0, 2.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(millimeter!(2.0, 3.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap(),
        );
        assert_eq!(rays.beam_radius_geo().unwrap(), millimeter!(0.5_f64.sqrt()));
        let mut ray =
            Ray::new_collimated(millimeter!(1.0, 15.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap();
        ray.set_invalid();
        assert_eq!(rays.beam_radius_geo().unwrap(), millimeter!(0.5_f64.sqrt()));
    }
    #[test]
    fn beam_radius_rms() {
        let mut rays = Rays::default();
        assert!(rays.beam_radius_rms().is_none());
        rays.add_ray(
            Ray::new_collimated(millimeter!(1.0, 1.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap(),
        );
        assert_eq!(rays.beam_radius_rms().unwrap(), Length::zero());
        rays.add_ray(
            Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap(),
        );
        assert_eq!(
            rays.beam_radius_rms().unwrap(),
            millimeter!(f64::sqrt(2.0) / 2.0)
        );
        let mut ray =
            Ray::new_collimated(millimeter!(1.0, 15.0, 0.), nanometer!(1053.0), joule!(1.0))
                .unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        assert_eq!(
            rays.beam_radius_rms().unwrap(),
            millimeter!(f64::sqrt(2.0) / 2.0)
        );
    }
    #[test]
    fn propagate_along_z_axis() {
        let mut rays = Rays::default();
        let ray0 = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        let ray1 =
            Ray::new_collimated(millimeter!(0., 1., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        rays.set_dist_to_next_surface(millimeter!(1.0));
        rays.propagate_along_z().unwrap();
        assert_eq!(rays.rays[0].position(), millimeter!(0., 0., 1.));
        assert_eq!(rays.rays[1].position(), millimeter!(0., 1., 1.));
    }
    #[test]
    fn refract_paraxial() {
        let mut rays = Rays::default();
        assert!(rays.refract_paraxial(millimeter!(0.0)).is_err());
        assert!(rays.refract_paraxial(millimeter!(f64::NAN)).is_err());
        assert!(rays.refract_paraxial(millimeter!(f64::INFINITY)).is_err());
        assert!(rays
            .refract_paraxial(millimeter!(f64::NEG_INFINITY))
            .is_err());
        assert!(rays.refract_paraxial(millimeter!(100.0)).is_ok());
        let ray0 =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        let ray1 =
            Ray::new_collimated(millimeter!(0., 1., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray0.clone());
        rays.add_ray(ray1.clone());
        rays.refract_paraxial(millimeter!(100.0)).unwrap();
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
        let mut rays = Rays::default();
        assert!(rays.filter_energy(&FilterType::Constant(-0.1)).is_err());
        let mut rays = Rays::default();
        assert!(rays.filter_energy(&FilterType::Constant(1.1)).is_err());
        let mut ray =
            Ray::new_collimated(millimeter!(0., 1., 0.), nanometer!(1054.0), joule!(1.0)).unwrap();
        rays.add_ray(ray.clone());
        let _ = ray.filter_energy(&FilterType::Constant(0.3)).unwrap();
        rays.filter_energy(&FilterType::Constant(0.3)).unwrap();
        assert_eq!(rays.rays[0].position(), ray.position());
        assert_eq!(rays.rays[0].direction(), ray.direction());
        assert_eq!(rays.rays[0].wavelength(), ray.wavelength());
        assert_eq!(rays.rays[0].energy(), ray.energy());
        assert_eq!(rays.rays.len(), 1);
    }
    #[test]
    fn invalidate_by_threshold() {
        testing_logger::setup();
        let mut rays = Rays::default();
        assert!(rays
            .invalidate_by_threshold_energy(joule!(f64::NAN))
            .is_err());
        assert!(rays
            .invalidate_by_threshold_energy(joule!(f64::INFINITY))
            .is_err());
        assert!(rays.invalidate_by_threshold_energy(joule!(-0.1)).is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "negative threshold energy given. Ray bundle unmodified."
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
        assert!(rays.invalidate_by_threshold_energy(joule!(0.0)).is_ok());
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.1)).unwrap();
        rays.add_ray(ray);
        rays.invalidate_by_threshold_energy(joule!(0.1)).unwrap();
        assert_eq!(rays.nr_of_rays(true), 2);
        rays.invalidate_by_threshold_energy(joule!(0.5)).unwrap();
        assert_eq!(rays.nr_of_rays(true), 1);
        rays.invalidate_by_threshold_energy(joule!(1.1)).unwrap();
        assert_eq!(rays.nr_of_rays(true), 0);
    }
    #[test]
    fn apodize() {
        let mut rays = Rays::default();
        let ray0 = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        let ray1 = Ray::new_collimated(millimeter!(1.0, 1.0, 0.), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        assert_eq!(rays.total_energy(), joule!(2.0));
        let circle_config = CircleConfig::new(millimeter!(0.5), millimeter!(0.0, 0.0)).unwrap();
        let aperture = Aperture::BinaryCircle(circle_config);
        rays.apodize(&aperture).unwrap();
        assert_eq!(rays.total_energy(), joule!(1.0));
    }
    #[test]
    fn wavelength_range() {
        let e = joule!(1.0);
        let mut rays = Rays::default();
        assert_eq!(rays.wavelength_range(), None);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), e).unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), e).unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(nanometer!(1053.0)..nanometer!(1053.0))
        );
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1050.0), e).unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(nanometer!(1050.0)..nanometer!(1053.0))
        );
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1051.0), e).unwrap();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(nanometer!(1050.0)..nanometer!(1053.0))
        );
        let mut ray = Ray::new_collimated(Point3::origin(), nanometer!(1000.0), e).unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        assert_eq!(
            rays.wavelength_range(),
            Some(nanometer!(1050.0)..nanometer!(1053.0))
        );
    }
    #[test]
    fn to_spectrum() {
        let e = joule!(1.0);
        let mut rays = Rays::default();
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), e).unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), e).unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1052.0), e).unwrap();
        rays.add_ray(ray);
        let ray = Ray::new_collimated(Point3::origin(), nanometer!(1052.1), e).unwrap();
        rays.add_ray(ray);
        let spectrum = rays.to_spectrum(&nanometer!(0.5)).unwrap();
        assert_abs_diff_eq!(
            spectrum.total_energy(),
            4.0,
            epsilon = 100000.0 * f64::EPSILON
        );
        let mut ray = Ray::new_collimated(Point3::origin(), nanometer!(1052.1), e).unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        let spectrum = rays.to_spectrum(&nanometer!(0.5)).unwrap();
        assert_abs_diff_eq!(
            spectrum.total_energy(),
            4.0,
            epsilon = 100000.0 * f64::EPSILON
        );
    }
    #[test]
    fn split() {
        let ray1 = Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(1.0)).unwrap();
        let ray2 = Ray::new_collimated(Point3::origin(), nanometer!(1050.0), joule!(2.0)).unwrap();
        let mut rays = Rays::default();
        let z_position = millimeter!(10.0);
        rays.z_position = z_position;
        rays.add_ray(ray1.clone());
        rays.add_ray(ray2.clone());
        assert!(rays.split(&SplittingConfig::Ratio(1.1)).is_err());
        assert!(rays.split(&SplittingConfig::Ratio(-0.1)).is_err());
        let split_rays = rays.split(&SplittingConfig::Ratio(0.2)).unwrap();
        assert_eq!(rays.absolute_z_of_last_surface(), z_position);
        assert_eq!(split_rays.absolute_z_of_last_surface(), z_position);
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(
            split_rays.total_energy().get::<joule>(),
            2.4,
            epsilon = 10.0 * f64::EPSILON
        );
        let mut rays = Rays::default();
        rays.add_ray(ray1.clone());
        rays.add_ray(ray2.clone());
        let mut ray =
            Ray::new_collimated(Point3::origin(), nanometer!(1053.0), joule!(5.0)).unwrap();
        ray.set_invalid();
        rays.add_ray(ray);
        assert_abs_diff_eq!(
            split_rays.total_energy().get::<joule>(),
            2.4,
            epsilon = 10.0 * f64::EPSILON
        );
    }

    #[test]
    fn get_rays_position_history_in_mm() {
        let ray_vec = vec![Ray::new(
            Point3::origin(),
            Vector3::new(0., 1., 2.),
            nanometer!(1053.),
            joule!(1.),
        )
        .unwrap()];
        let mut rays = Rays::from(ray_vec);
        rays.set_dist_to_next_surface(millimeter!(1.));
        let _ = rays.propagate_along_z();
        rays.set_dist_to_next_surface(millimeter!(2.));
        let _ = rays.propagate_along_z();

        let pos_hist_comp = vec![MatrixXx3::from_vec(vec![
            0., 0., 0., 0., 0.5, 1.5, 0., 1., 3.,
        ])];

        let pos_hist = rays.get_rays_position_history().unwrap();
        for (ray_pos, ray_pos_calc) in izip!(
            pos_hist_comp.iter(),
            pos_hist.rays_pos_history[0].get_history().iter()
        ) {
            for (row, row_calc) in izip!(ray_pos.row_iter(), ray_pos_calc.row_iter()) {
                assert_eq!(row[0], row_calc[0].get::<millimeter>());
                assert_eq!(row[1], row_calc[1].get::<millimeter>());
                assert_eq!(row[2], row_calc[2].get::<millimeter>());
            }
        }
    }
    #[test]
    fn get_wavefront_data_in_units_of_wvl() {
        //empty rays vector
        let rays = Rays::from(Vec::<Ray>::new());
        let wf_data = rays.get_wavefront_data_in_units_of_wvl(true, nanometer!(10.));
        assert!(wf_data.is_err());

        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            5,
            nanometer!(1000.),
            joule!(1.),
        )
        .unwrap();

        rays.set_dist_to_next_surface(millimeter!(10.));
        let _ = rays.propagate_along_z();
        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(true, nanometer!(10.))
            .unwrap();
        assert!(wf_data.wavefront_error_maps.len() == 1);
        rays.add_ray(
            Ray::new(
                Point3::origin(),
                Vector3::new(0., 1., 0.),
                nanometer!(1005.),
                joule!(1.),
            )
            .unwrap(),
        );
        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, nanometer!(10.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 1);

        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, nanometer!(3.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 2);
        rays.add_ray(
            Ray::new(
                Point3::origin(),
                Vector3::new(0., 1., 0.),
                nanometer!(1007.),
                joule!(1.),
            )
            .unwrap(),
        );

        let wf_data = rays
            .get_wavefront_data_in_units_of_wvl(false, nanometer!(3.))
            .unwrap();

        assert!(wf_data.wavefront_error_maps.len() == 3);
    }
    #[test]
    fn wavefront_error_at_pos_in_units_of_wvl() {
        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            1,
            nanometer!(1000.),
            joule!(1.),
        )
        .unwrap();
        rays.set_dist_to_next_surface(millimeter!(10.));
        let _ = rays.propagate_along_z();

        let wf_error = rays.wavefront_error_at_pos_in_units_of_wvl(nanometer!(1000.));

        for (i, val) in wf_error.column(2).iter().enumerate() {
            if i != 0 {
                assert!((val - (1. - f64::sqrt(2.)) * 10000.).abs() < f64::EPSILON * val.abs())
            } else {
                assert!(val.abs() < f64::EPSILON)
            }
        }
        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            1,
            nanometer!(500.),
            joule!(1.),
        )
        .unwrap();
        rays.set_dist_to_next_surface(millimeter!(10.));
        let _ = rays.propagate_along_z();

        let wf_error = rays.wavefront_error_at_pos_in_units_of_wvl(nanometer!(500.));

        for (i, val) in wf_error.column(2).iter().enumerate() {
            if i != 0 {
                assert!((val - (1. - f64::sqrt(2.)) * 20000.).abs() < f64::EPSILON * val.abs())
            } else {
                assert!(val.abs() < f64::EPSILON)
            }
        }
    }
    #[test]
    fn get_xy_rays_pos() {
        let rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            1,
            nanometer!(1000.),
            joule!(1.),
        )
        .unwrap();

        let xy_pos = rays.get_xy_rays_pos(false);
        for val in xy_pos.row_iter() {
            assert!(val[(0, 0)].abs() < f64::EPSILON);
            assert!(val[(0, 1)].abs() < f64::EPSILON);
        }

        let pos_xy = MatrixXx2::from_vec(vec![1., 2., -10., -2000., 1., 2., -10., -2000.]);

        let ray_vec = vec![
            Ray::new(
                millimeter!(1.0, 1.0, 0.),
                Vector3::new(0., 1., 0.),
                nanometer!(1000.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(2.0, 2.0, 0.),
                Vector3::new(0., 1., 0.),
                nanometer!(1000.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(-10.0, -10.0, 0.),
                Vector3::new(0., 1., 0.),
                nanometer!(1000.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(-2000.0, -2000.0, 0.),
                Vector3::new(0., 1., 0.),
                nanometer!(1000.),
                joule!(1.),
            )
            .unwrap(),
        ];

        let rays = Rays::from(ray_vec);
        let xy_pos = rays.get_xy_rays_pos(false);

        for (val_is, val_got) in izip!(pos_xy.row_iter(), xy_pos.row_iter()) {
            assert!((val_is[(0, 0)] - val_got[(0, 0)]).abs() < f64::EPSILON * val_is[(0, 0)].abs());
            assert!((val_is[(0, 1)] - val_got[(0, 1)]).abs() < f64::EPSILON * val_is[(0, 1)].abs());
        }
    }
    #[test]
    fn calc_fluence_at_position_test() {
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &FibonacciRectangle::new(centimeter!(1.), centimeter!(1.), 1000).unwrap(),
        )
        .unwrap();

        let fluence = rays.calc_fluence_at_position().unwrap();
        assert!(approx::RelativeEq::relative_eq(
            &fluence.get_average_fluence(),
            &1.,
            0.01,
            0.01
        ));

        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &FibonacciRectangle::new(centimeter!(1.), centimeter!(2.), 10000).unwrap(),
        )
        .unwrap();

        let fluence = rays.calc_fluence_at_position().unwrap();
        assert!(approx::RelativeEq::relative_eq(
            &fluence.get_average_fluence(),
            &0.5,
            0.01,
            0.01
        ));
    }

    #[test]
    fn energy_centroid_test() {
        let rays = Rays::from(vec![
            Ray::new(
                millimeter!(-1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(1.),
            )
            .unwrap(),
        ]);
        let centroid = rays.energy_weighted_centroid();
        assert!(centroid.is_some());
        assert_relative_eq!(centroid.unwrap().x.get::<millimeter>(), 0.);
        assert_relative_eq!(centroid.unwrap().y.get::<millimeter>(), 0.);
        assert_relative_eq!(centroid.unwrap().z.get::<millimeter>(), 0.);

        let rays = Rays::from(vec![
            Ray::new(
                millimeter!(-1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(0.5),
            )
            .unwrap(),
        ]);
        let centroid = rays.energy_weighted_centroid();
        assert!(centroid.is_some());
        assert_relative_eq!(centroid.unwrap().x.get::<millimeter>(), -1. / 3.);
        assert_relative_eq!(centroid.unwrap().y.get::<millimeter>(), 0.);
        assert_relative_eq!(centroid.unwrap().z.get::<millimeter>(), 0.);

        let mut rays = Rays::from(vec![
            Ray::new(
                millimeter!(-1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(1.),
            )
            .unwrap(),
            Ray::new(
                millimeter!(1., 0., 0.),
                Vector3::new(0., 0., 1.),
                nanometer!(1054.),
                joule!(0.5),
            )
            .unwrap(),
        ]);

        rays.rays[1].set_invalid();
        let centroid = rays.energy_weighted_centroid();
        assert!(centroid.is_some());
        assert_relative_eq!(centroid.unwrap().x.get::<millimeter>(), -1.);
        assert_relative_eq!(centroid.unwrap().y.get::<millimeter>(), 0.);
        assert_relative_eq!(centroid.unwrap().z.get::<millimeter>(), 0.);

        let rays = Rays::default();
        let centroid = rays.energy_weighted_centroid();
        assert!(centroid.is_none());
    }
}
