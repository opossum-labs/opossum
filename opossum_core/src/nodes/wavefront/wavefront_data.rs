use approx::relative_eq;
use log::warn;
use nalgebra::{DVector, DVectorView, MatrixXx2, MatrixXx3, Vector2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{millimeter, nanometer},
};

use crate::{
    error::{OpmResult, OpossumError},
    light::Rays,
    nanometer,
    reporting::plottable::{
        AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable,
    },
    utils::{
        geom_transformation::Isometry,
        griddata::{create_linspace_axes, grid_interpolate_3d_scatter_data},
        to_f64,
    },
};

/// This [`WaveFrontData`] struct holds a vector of wavefront-error maps.
/// The vector of [`WaveFrontMap`] is necessary, e.g., to store the wavefront data for each spectral component of a pulse.

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct WaveFrontData {
    /// vector of [`WaveFrontMap`]. May contain only a single [`WaveFrontMap`] if only calculated for a single wavelength
    pub wavefront_error_maps: Vec<WaveFrontMap>,
}
impl WaveFrontData {
    /// Creates [`WaveFrontData`] from a bundle of rays, supporting both center-wavelength
    /// approximations and full spectral analysis.
    ///
    /// /// Returns the wavefront of the bundle of [`Rays`] at the center wavelength or at each band of the spectrum with a defined resolution.
    /// This function calculates the wavefront of a ray bundle as multiple of its wavelength with reference to the ray that is closest to the optical axis.
    ///
    ///
    /// # Attributes
    /// - `center_wavelength_flag`: flag to define if the center wavelength should be used for calculation or if a wavefront for all spectral components should be analyzed
    ///
    /// # Errors
    /// This function errors for the moment if `center_wavelength_flag` is set to false
    ///
    /// # Panics
    /// This method panics if the usize `to_f64()`conversion fails. This is not expected.
    pub fn from_rays(
        rays: &Rays,
        center_wavelength_flag: bool,
        average_flag: bool,
        monitor_isometry: &Isometry,
    ) -> OpmResult<Self> {
        if center_wavelength_flag {
            let center_wavelength = rays.get_center_wavelength().ok_or_else(|| {
                OpossumError::Other("Cannot determine center wavelength of empty ray bundle".into())
            })?;
            let wvls = rays.get_unique_wavelengths(true)?;

            if average_flag {
                warn!(
                    "Averaging wavefronts over the spectrum is not yet implemented. Using the center wavelength only is an approximation that assumes all wavefronts are similar in shape. This may not be accurate for broad spectra or systems with significant chromatic aberrations."
                );
                Err(OpossumError::Other(
                    "Averaging wavefronts over the spectrum is not yet implemented".into(),
                ))
            } else {
                // Find the closest available wavelength to the center wavelength
                let closest_wvl = wvls.iter().copied().fold(wvls[0], |a, b| {
                    if (b - center_wavelength).abs() < (a - center_wavelength).abs() {
                        b
                    } else {
                        a
                    }
                });

                let mut rays_at_closest_wvl = Rays::default();
                for ray in rays {
                    if relative_eq!(
                        ray.wavelength().value,
                        closest_wvl.value,
                        epsilon = 10. * f64::EPSILON
                    ) {
                        rays_at_closest_wvl.add_ray(ray.clone());
                    }
                }

                let map =
                    WaveFrontMap::from_rays(&rays_at_closest_wvl, closest_wvl, monitor_isometry)?;
                Ok(Self {
                    wavefront_error_maps: vec![map],
                })
            }
        } else {
            let (rays_sorted_by_spectrum, wvls) = rays.split_ray_bundle_by_wavelength(
                Length::new::<nanometer>(10. * f64::EPSILON),
                true,
            )?;

            let mut wf_error_maps = Vec::with_capacity(rays_sorted_by_spectrum.len());
            for (bundle, &wvl) in rays_sorted_by_spectrum.iter().zip(wvls.iter()) {
                if !bundle.is_empty() {
                    wf_error_maps.push(WaveFrontMap::from_rays(bundle, wvl, monitor_isometry)?);
                }
            }

            Ok(Self {
                wavefront_error_maps: wf_error_maps,
            })
        }
    }
}
/// A struct which holds the necessary data to describe the wavefront as well as some statistical values:
/// - `wavelength`: the wavelength that was used to calculate this wavefront map in units of a specific wavelength
/// - `ptv`: the peak-to-valley value of the wavefront map in units of milli-lambda
/// - `rms`: the root-mean-square value of the wavefront map in units of milli-lambda
/// - `x`: the x axis of the wavefront map
/// - `y`: the y axis of the wavefront map
/// - `wf_map`: the wavefront map
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WaveFrontMap {
    wavelength: Length,
    ptv: f64,
    rms: f64,
    x: Vec<f64>,
    y: Vec<f64>,
    opd: Vec<f64>,
}

impl Default for WaveFrontMap {
    fn default() -> Self {
        Self {
            wavelength: nanometer!(1054.),
            ptv: 0.0,
            rms: 0.0,
            x: vec![0.0],
            y: vec![0.],
            opd: vec![0.0],
        }
    }
}

impl WaveFrontMap {
    /// Creates a new [`WaveFrontMap`]
    /// # Attributes
    /// - `wf_dat`: wavefront data as Matrix with 3 columns and dynamix number of rows. Columns are used as 1:x, 2:y, 3:z
    /// - `wavelength`: wavelength that is used for this `WavefrontErrorMap`
    ///
    /// # Returns
    /// This method returns a [`WaveFrontMap`] struct
    ///
    /// # Errors
    /// This method will return an error if the wavefront data is empty or if `calc_wavefront_statistics()` fails.
    pub fn new(wf_dat: &MatrixXx3<f64>, wavelength: Length) -> OpmResult<Self> {
        if wf_dat.is_empty() {
            Err(OpossumError::Other("Empty wavefront-data vector!".into()))
        } else {
            let len_wf_dat = wf_dat.len();
            let mut x = Vec::<f64>::with_capacity(len_wf_dat);
            let mut y = Vec::<f64>::with_capacity(len_wf_dat);
            let mut wf_map = Vec::<f64>::with_capacity(len_wf_dat);
            for row in wf_dat.row_iter() {
                x.push(row[0]);
                y.push(row[1]);
                wf_map.push(row[2]);
            }
            let mut wf_map = Self {
                wavelength,
                ptv: f64::NAN,
                rms: f64::NAN,
                x,
                y,
                opd: wf_map,
            };
            wf_map.calc_wavefront_statistics()?;
            Ok(wf_map)
        }
    }
    /// Generates a [`WaveFrontMap`] directly from a ray bundle for a specific wavelength.
    ///
    /// This function projects the ray positions onto the monitor plane (X/Y) and calculates
    /// the optical path difference relative to the chief ray. Because rays have already
    /// intersected the detector surface, their `path_length` inherently accounts for
    /// the surface geometry (e.g., flat or spherical).
    ///     
    /// # Attributes
    /// - `wavelength`: wave length that is used for this wavefront calculation
    ///
    /// # Errors
    /// This function returns an error if the given ray bundle is empty.
    pub fn from_rays(
        rays: &Rays,
        wavelength: Length,
        monitor_isometry: &Isometry,
    ) -> OpmResult<Self> {
        let wvl_nm = wavelength.get::<nanometer>();
        let nr_of_valid_rays = rays.nr_of_rays(true);

        if nr_of_valid_rays == 0 {
            return Err(OpossumError::Other("Empty ray bundle!".into()));
        }

        let mut wf_dat = MatrixXx3::from_element(nr_of_valid_rays, 0.);
        let mut min_radius = f64::INFINITY;
        let mut path_length_at_center = 0.;

        for (i, ray) in rays.iter().filter(|r| r.valid()).enumerate() {
            // Transform position into the local monitor frame
            let pos_in_monitor_frame = monitor_isometry.inverse_transform_point(&ray.position());
            let position = Vector2::new(
                pos_in_monitor_frame.x.get::<millimeter>(),
                pos_in_monitor_frame.y.get::<millimeter>(),
            );

            wf_dat[(i, 0)] = position.x;
            wf_dat[(i, 1)] = position.y;
            // The wavefront error has the negative sign of the optical path difference
            wf_dat[(i, 2)] = -ray.path_length().get::<nanometer>();

            // Find the chief ray (closest to the optical axis)
            let radius = position.y.mul_add(position.y, position.x * position.x);
            if radius < min_radius {
                min_radius = radius;
                path_length_at_center = wf_dat[(i, 2)];
            }
        }

        // Subtract the chief ray's path length to get relative OPD, then convert to fractions of wavelength
        for mut row in wf_dat.row_iter_mut() {
            row[2] -= path_length_at_center;
            row[2] /= wvl_nm;
        }
        Self::new(&wf_dat, wavelength)
    }

    /// Calculate the `PtV` and `RMS` values of this [`WaveFrontMap`].
    ///
    /// Note: RMS calculation is performed from wavefront data - avg. OPD !!! (compatible with ZEMAX)
    ///
    /// # Errors
    ///
    /// This function returns an error if the wavefront map is empty or contains non-finite values.
    pub fn calc_wavefront_statistics(&mut self) -> OpmResult<()> {
        let wf_map = &self.opd;
        if wf_map.is_empty() {
            return Err(OpossumError::Other("Empty wavefront-data vector!".into()));
        }
        let min_val = wf_map.iter().copied().reduce(f64::min);
        let max_val = wf_map.iter().copied().reduce(f64::max);

        let (Some(min), Some(max)) = (min_val, max_val) else {
            return Err(OpossumError::Other(
                "undefined wavefront data. cannot calculate PtV & RMS".into(),
            ));
        };
        let ptv = max - min;
        let Some(avg) = Some(wf_map.iter().sum::<f64>() / to_f64(wf_map.len())) else {
            return Err(OpossumError::Other(
                "undefined wavefront data. cannot calculate mean values".into(),
            ));
        };
        let rms = f64::sqrt(
            self.opd.iter().map(|l| (l - avg) * (l - avg)).sum::<f64>() / to_f64(wf_map.len()),
        );
        self.ptv = ptv;
        self.rms = rms;
        Ok(())
    }

    /// Returns the wavelength of this [`WaveFrontMap`].
    #[must_use]
    pub const fn wavelength(&self) -> Length {
        self.wavelength
    }

    /// Returns the rms of this [`WaveFrontMap`].
    #[must_use]
    pub const fn rms(&self) -> f64 {
        self.rms
    }

    /// Returns the ptv of this [`WaveFrontMap`].
    #[must_use]
    pub const fn ptv(&self) -> f64 {
        self.ptv
    }
}

impl Plottable for WaveFrontMap {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            // 2D Map Case
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("y position in mm".into()))?
                .set(&PlotArgs::CBarLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::ExpandBounds(false))?
                .set(&PlotArgs::AxisEqual(true))?
                .set(&PlotArgs::PlotAutoSize(true))?;
        } else if range_x > eps {
            // 1D Line Cut (X varies)
            plt_params
                .set(&PlotArgs::XLabel("x position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        } else if range_y > eps {
            // 1D Line Cut (Y varies)
            plt_params
                .set(&PlotArgs::XLabel("y position in mm".into()))?
                .set(&PlotArgs::YLabel("wavefront error in λ".into()))?
                .set(&PlotArgs::PlotSize((1200, 800)))?
                .set(&PlotArgs::AxisEqual(false))?;
        }
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        if range_x > eps && range_y > eps {
            // 2D Map Case
            let mut plt_type = PlotType::ColorMesh(plt_params.clone());
            let legend = plt_params.get_legend_flag().unwrap_or(false);

            // Adjust Z-axis bounds for 2D plots if data is nearly flat
            if let Some(plt_series) = &self.get_plot_series(&mut plt_type, legend).unwrap_or(None)
                && !plt_series.is_empty()
            {
                let ranges = plt_series[0].define_data_based_axes_bounds(false);
                let z_bounds = ranges
                    .get_z_bounds()
                    .unwrap_or_else(|| AxLims::new(-0.5e-3, 0.5e-3).unwrap());
                if z_bounds.min > -1e-3 && z_bounds.max < 1e-3 {
                    _ = plt_type.set_plot_param(&PlotArgs::ZLim(Some(AxLims {
                        min: -1e-3,
                        max: 1e-3,
                    })));
                }
            }
            plt_type
        } else {
            // 1D Line or Point Case
            PlotType::Line2D(plt_params.clone())
        }
    }
    fn get_plot_series(
        &self,
        _plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let min_x = self.x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = self.x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_x = max_x - min_x;

        let min_y = self.y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = self.y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range_y = max_y - min_y;

        let eps = 1e-12;

        // Case 0: Single Point (0D)
        if range_x <= eps && range_y <= eps {
            warn!("Wavefront data has zero dimension in X and Y. Cannot plot.");
            return Ok(None);
        }

        // Case 1: 1D Line Cut (One dimension is effectively zero)
        if range_x <= eps || range_y <= eps {
            // Select the varying axis and corresponding values
            let mut data: Vec<(f64, f64)> = if range_x > eps {
                // X varies, Y is constant
                self.x
                    .iter()
                    .zip(self.opd.iter())
                    .map(|(&x, &z)| (x, z))
                    .collect()
            } else {
                // Y varies, X is constant
                self.y
                    .iter()
                    .zip(self.opd.iter())
                    .map(|(&y, &z)| (y, z))
                    .collect()
            };

            // Sort data by the independent axis to ensure correct line plotting
            data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut mat = MatrixXx2::zeros(data.len());
            for (i, (coord, val)) in data.iter().enumerate() {
                mat[(i, 0)] = *coord;
                mat[(i, 1)] = *val;
            }

            let plt_series = PlotSeries::new(
                &PlotData::Dim2 { xy_data: mat },
                RGBAColor(255, 0, 0, 1.),
                None,
            );
            return Ok(Some(vec![plt_series]));
        }

        // Case 2: 2D Map (Original Logic)
        if let (Ok((x_interp, _)), Ok((y_interp, _))) = (
            create_linspace_axes(DVectorView::from(&DVector::from_vec(self.x.clone())), 100),
            create_linspace_axes(DVectorView::from(&DVector::from_vec(self.y.clone())), 100),
        ) {
            let scattered_data = MatrixXx3::from_columns(&[
                DVector::from_vec(self.x.clone()),
                DVector::from_vec(self.y.clone()),
                DVector::from_vec(self.opd.clone()),
            ]);
            if let Ok((interp_dat, _)) =
                grid_interpolate_3d_scatter_data(&scattered_data, &x_interp, &y_interp)
            {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: x_interp,
                    y_dat_m: y_interp,
                    z_dat_nxm: interp_dat,
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            } else {
                warn!(
                    "Could not create interpolated wavefront map for plotting! Returning no plot data."
                );
                Ok(None)
            }
        } else {
            warn!("Could not create axes from provided data! Returning no plot data.");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        analyzers::propagation_strategy::MissedSurfaceStrategy,
        core_optics::optic_surface::OpticSurface,
        degree,
        error::OpmResult,
        joule,
        light::{Ray, Rays},
        millimeter, nanometer,
        nodes::{WaveFrontData, WaveFrontMap},
        refractive_index::refr_index_vaccuum,
        utils::geom_transformation::Isometry,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::{MatrixXx3, Point3, Vector3};
    use uom::si::f64::Length;

    #[test]
    fn wavefront_map_from_rays() -> OpmResult<()> {
        assert!(
            WaveFrontMap::from_rays(&Rays::default(), nanometer!(1000.), &Isometry::identity())
                .is_err()
        );

        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            1,
            nanometer!(1000.),
            joule!(1.),
        )?;

        let mut s = OpticSurface::default();
        s.set_isometry(Isometry::new_along_z(millimeter!(10.0))?);
        rays.refract_on_surface(
            &mut s,
            Some(&refr_index_vaccuum()),
            true,
            &MissedSurfaceStrategy::Stop,
        )?;
        let wf_map = WaveFrontMap::from_rays(&rays, nanometer!(1000.), &Isometry::identity())?;

        for (i, val) in wf_map.opd.into_iter().enumerate() {
            if i != 0 {
                assert_relative_eq!(
                    val,
                    &(10000. * (1. - f64::sqrt(2.))),
                    epsilon = 100000. * f64::EPSILON
                );
            } else {
                assert_abs_diff_eq!(val, &0.0)
            }
        }
        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            1,
            nanometer!(500.),
            joule!(1.),
        )?;
        rays.refract_on_surface(
            &mut s,
            Some(&refr_index_vaccuum()),
            true,
            &MissedSurfaceStrategy::Stop,
        )?;
        let wf_map = WaveFrontMap::from_rays(&rays, nanometer!(500.), &Isometry::identity())?;
        for (i, val) in wf_map.opd.into_iter().enumerate() {
            if i != 0 {
                assert_relative_eq!(
                    val,
                    &(20000. * (1. - f64::sqrt(2.))),
                    epsilon = 100000. * f64::EPSILON
                );
            } else {
                assert_abs_diff_eq!(val, &0.0)
            }
        }
        Ok(())
    }
    fn propagate(rays: &mut Rays, distance: Length) -> OpmResult<()> {
        for ray in rays {
            if ray.valid() {
                ray.propagate(distance)?;
            }
        }
        Ok(())
    }
    #[test]
    fn wavefront_data_from_rays() -> OpmResult<()> {
        //empty rays vector
        assert!(
            WaveFrontData::from_rays(&Rays::default(), true, false, &Isometry::identity()).is_err()
        );

        let mut rays = Rays::new_hexapolar_point_source(
            Point3::origin(),
            degree!(90.),
            5,
            nanometer!(1000.),
            joule!(1.),
        )?;
        propagate(&mut rays, millimeter!(1.0))?;
        let wf_data = WaveFrontData::from_rays(&rays, true, false, &Isometry::identity())?;
        assert!(wf_data.wavefront_error_maps.len() == 1);
        rays.add_ray(Ray::new(
            Point3::origin(),
            Vector3::y(),
            nanometer!(1005.),
            joule!(1.),
        )?);
        let wf_data = WaveFrontData::from_rays(&rays, false, false, &Isometry::identity())?;

        assert!(wf_data.wavefront_error_maps.len() == 2);
        rays.add_ray(Ray::new(
            Point3::origin(),
            Vector3::y(),
            nanometer!(1007.),
            joule!(1.),
        )?);
        let wf_data = WaveFrontData::from_rays(&rays, false, false, &Isometry::identity())?;

        assert!(wf_data.wavefront_error_maps.len() == 3);
        Ok(())
    }
    #[test]
    fn test_wavefront_statistics_math() -> OpmResult<()> {
        // Erstelle eine Matrix mit fixen Werten: X, Y, Z (OPD)
        // Z-Werte: -1.0, 0.0, 1.0 -> PtV sollte 2.0 sein.
        // Durchschnitt ist 0.0. Varianz = ((-1-0)^2 + (0-0)^2 + (1-0)^2) / 3 = 2/3.
        // RMS = sqrt(2/3) ≈ 0.81649658

        let mut wf_dat = MatrixXx3::zeros(3);
        wf_dat[(0, 2)] = -1.0;
        wf_dat[(1, 2)] = 0.0;
        wf_dat[(2, 2)] = 1.0;

        let wvl = nanometer!(1000.0);
        let wf_map = WaveFrontMap::new(&wf_dat, wvl)?;

        assert_eq!(wf_map.ptv(), 2.0);
        assert_relative_eq!(wf_map.rms(), f64::sqrt(2.0 / 3.0), epsilon = 1e-8);
        Ok(())
    }
    #[test]
    fn calc_wavefront_statistics() -> OpmResult<()> {
        let wvl = nanometer!(1000.);
        let en = joule!(1.);

        let mut rays = Rays::from(Ray::new_collimated(Point3::origin(), wvl, en)?);
        let mut ray = Ray::new_collimated(Point3::origin(), wvl, en)?;
        ray.propagate(wvl)?;
        rays.add_ray(ray);
        let wvf_map = WaveFrontMap::from_rays(&rays, wvl, &Isometry::identity())?;
        assert_eq!(wvf_map.ptv, 1.0);
        assert_abs_diff_eq!(wvf_map.rms, 0.5);
        Ok(())
    }
}
