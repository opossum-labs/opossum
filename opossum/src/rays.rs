#![warn(missing_docs)]
//! Module for handling rays
use crate::aperture::Aperture;
use crate::error::{OpmResult, OpossumError};
use crate::plottable::Plottable;
use crate::properties::Proptype;
use crate::reporter::PdfReportable;
use image::DynamicImage;
use nalgebra::{point, Point2, Point3, Vector3};
use plotters::prelude::{ChartBuilder, Circle, EmptyElement};
use plotters::series::PointSeries;
use plotters::style::RED;
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use sobol::{params::JoeKuoD6, Sobol};
use uom::num_traits::Zero;
use uom::si::f64::{Energy, Length};

///Struct that contains all information about an optical ray
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Ray {
    ///Stores all positions of the ray
    pos: Point3<f64>, // this should be a vector of points?
    /// stores the current propagation direction of the ray (stored as direction cosine)
    dir: Vector3<f64>,
    // ///stores the polarization vector (Jones vector) of the ray
    // pol: Vector2<Complex<f64>>,
    ///energy of the ray
    e: Energy,
    ///Wavelength of the ray in nm
    wvl: Length,
    // ///id of the ray
    // id: usize,
    // ///Bounce count of the ray. Necessary to check if the maximum number of bounces is reached
    // bounce: usize,
    // //True if ray is allowd to further propagate, false else
    // //valid:  bool,
    path_length: f64,
}
impl Ray {
    /// Create a new collimated ray.
    ///
    /// Generate a ray a horizontally polarized ray collinear with the z axis (optical axis).
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given wavelength is <=0.0, `NaN` or +inf
    ///  - the given energy is <=0.0, `NaN` or +inf
    pub fn new_collimated(
        position: Point2<f64>,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        Self::new(position, Vector3::new(0.0, 0.0, 1.0), wave_length, energy)
    }
    /// Creates a new [`Ray`].
    ///
    /// The dircetion vector is normalized. The direction is thus stored aa (`direction cosine`)[https://en.wikipedia.org/wiki/Direction_cosine]
    ///
    /// # Error
    /// This function returns an error if
    ///  - the given wavelength is <=0.0, `NaN` or +inf
    ///  - the given energy is <=0.0, `NaN` or +inf
    ///  - the direction vector has a zero length
    pub fn new(
        position: Point2<f64>,
        direction: Vector3<f64>,
        wave_length: Length,
        energy: Energy,
    ) -> OpmResult<Self> {
        if wave_length.is_zero() || wave_length.is_sign_negative() || !wave_length.is_finite() {
            return Err(OpossumError::Other("wavelength must be >0".into()));
        }
        if energy.is_zero() || energy.is_sign_negative() || !energy.is_finite() {
            return Err(OpossumError::Other("energy must be >0".into()));
        }
        if direction.norm().is_zero() {
            return Err(OpossumError::Other("length of direction must be >0".into()));
        }
        Ok(Self {
            pos: Point3::new(position.x, position.y, 0.0),
            dir: direction.normalize(),
            //pol: Vector2::new(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)), // horizontal polarization
            e: energy,
            wvl: wave_length,
            //id: 0,
            //bounce: 0,
            path_length: 0.0,
        })
    }
    /// Returns the position of thi [`Ray`].
    #[must_use]
    pub fn position(&self) -> Point3<f64> {
        self.pos
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
    pub fn propagate_along_z(&self, length_along_z: f64) -> OpmResult<Self> {
        if self.dir[2].abs() < f64::EPSILON {
            return Err(OpossumError::Other(
                "z-Axis of direction vector must be != 0.0".into(),
            ));
        }
        let mut new_ray = self.clone();
        let length_in_ray_dir = length_along_z / self.dir[2];
        new_ray.pos = self.pos + length_in_ray_dir * self.dir;
        new_ray.path_length += length_in_ray_dir;
        Ok(new_ray)
    }
}
///Struct containing all relevant information of a created bundle of rays
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rays {
    ///vector containing rays
    rays: Vec<Ray>,
    //Maximum number of bounces
    //max_bounces:    usize, do we need this here?
}
impl Rays {
    /// Generate a set of collimated rays (collinear with optical axis).
    pub fn new_uniform_collimated(
        size: f64,
        wave_length: Length,
        energy: Energy,
        strategy: &DistributionStrategy,
    ) -> OpmResult<Self> {
        let points: Vec<Point2<f64>> = strategy.generate(size);
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
    /// Returns the total energy of this [`Rays`].
    ///
    /// This simply sums up all energies of the individual rays.
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        self.rays.iter().fold(Energy::zero(), |a, b| a + b.e)
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
    /// Add a single ray to the ray bundle.
    pub fn add_ray(&mut self, ray: Ray) {
        self.rays.push(ray);
    }
    /// Propagate a ray bundle along the z axis.
    ///
    /// # Errors
    /// This function returns an error if the z component of a ray direction is zero.
    pub fn propagate_along_z(&mut self, length_along_z: f64) -> OpmResult<()> {
        for ray in &mut self.rays {
            *ray=ray.propagate_along_z(length_along_z)?;
        }
        Ok(())
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
    pub fn generate(&self, size: f64) -> Vec<Point2<f64>> {
        match self {
            Self::Hexapolar(rings) => hexapolar(*rings, size),
            Self::Random(nr_of_rays) => random(*nr_of_rays, size),
            Self::Sobol(nr_of_rays) => sobol(*nr_of_rays, size),
        }
    }
}
fn hexapolar(rings: u8, radius: f64) -> Vec<Point2<f64>> {
    let mut points: Vec<Point2<f64>> = Vec::new();
    let radius_step = radius / f64::from(rings);
    points.push(point![0.0, 0.0]);
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
fn random(nr_of_rays: usize, side_length: f64) -> Vec<Point2<f64>> {
    let mut points: Vec<Point2<f64>> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..nr_of_rays {
        points.push(point![
            rng.gen_range(-side_length..side_length),
            rng.gen_range(-side_length..side_length)
        ]);
    }
    points
}
fn sobol(nr_of_rays: usize, side_length: f64) -> Vec<Point2<f64>> {
    let mut points: Vec<Point2<f64>> = Vec::new();
    let params = JoeKuoD6::minimal();
    let seq = Sobol::<f64>::new(2, &params);
    let offset = side_length / 2.0;
    for point in seq.take(nr_of_rays) {
        points.push(point!(point[0] - offset, point[1] - offset));
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
        if (x_max - x_min).abs() < f64::EPSILON {
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
        if (y_max - y_min).abs() < f64::EPSILON {
            y_max = 1.0;
            y_min = -1.0;
        }
        let mut chart = ChartBuilder::on(root)
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(|e| OpossumError::Other(format!("creation of plot failed: {e}")))?;

        chart
            .configure_mesh()
            .x_desc("x")
            .y_desc("y")
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
    use uom::si::{energy::joule, length::nanometer};
    #[test]
    fn ray_new_collimated() {
        let position = Point2::new(1.0, 2.0);
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
        assert_eq!(ray.path_length, 0.0);
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
        .is_err());
        assert!(Ray::new_collimated(
            position,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(-10.0)
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
        let position = Point2::new(1.0, 2.0);
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
        assert_eq!(ray.position(), Point3::new(1.0, 2.0, 0.0));
        assert_eq!(ray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(ray.wvl, Length::new::<nanometer>(1053.0));
        assert_eq!(ray.wavelength(), Length::new::<nanometer>(1053.0));
        assert_eq!(ray.e, Energy::new::<joule>(1.0));
        assert_eq!(ray.energy(), Energy::new::<joule>(1.0));
        assert_eq!(ray.path_length, 0.0);
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
            Energy::new::<joule>(0.0)
        )
        .is_err());
        assert!(Ray::new(
            position,
            direction,
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(-10.0)
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
            Point2::new(0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray.propagate_along_z(1.0).is_ok());
        let newray = ray.propagate_along_z(1.0).unwrap();
        assert_eq!(newray.wavelength(), wvl);
        assert_eq!(newray.energy(), energy);
        assert_eq!(newray.dir, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(
            ray.propagate_along_z(1.0).unwrap().position(),
            Point3::new(0.0, 0.0, 1.0)
        );
        assert_eq!(
            ray.propagate_along_z(2.0).unwrap().position(),
            Point3::new(0.0, 0.0, 2.0)
        );
        assert_eq!(
            ray.propagate_along_z(-1.0).unwrap().position(),
            Point3::new(0.0, 0.0, -1.0)
        );
        let ray = Ray::new(
            Point2::new(0.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            wvl,
            energy,
        )
        .unwrap();
        assert_eq!(
            ray.propagate_along_z(1.0).unwrap().position(),
            Point3::new(0.0, 1.0, 1.0)
        );
        assert_eq!(
            ray.propagate_along_z(2.0).unwrap().position(),
            Point3::new(0.0, 2.0, 2.0)
        );
        let ray = Ray::new(
            Point2::new(0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            wvl,
            energy,
        )
        .unwrap();
        assert!(ray.propagate_along_z(1.0).is_err());
    }
    #[test]
    fn strategy_hexapolar() {
        let strategy = DistributionStrategy::Hexapolar(0);
        assert_eq!(strategy.generate(1.0).len(), 1);
        let strategy = DistributionStrategy::Hexapolar(1);
        assert_eq!(strategy.generate(1.0).len(), 7);
        let strategy = DistributionStrategy::Hexapolar(5);
        assert_eq!(strategy.generate(1.0).len(), 91);
    }
    #[test]
    fn strategy_random() {
        let strategy = DistributionStrategy::Random(10);
        assert_eq!(strategy.generate(1.0).len(), 10);
    }
    #[test]
    fn rays_new_uniform_collimated() {
        let rays = Rays::new_uniform_collimated(
            1.0,
            Length::new::<nanometer>(1054.0),
            Energy::new::<joule>(1.0),
            &DistributionStrategy::Hexapolar(2),
        );
        assert!(rays.is_ok());
        let rays = rays.unwrap();
        assert_eq!(rays.rays.len(), 19);
        assert!(
            Energy::abs(rays.total_energy() - Energy::new::<joule>(1.0))
                < Energy::new::<joule>(10.0 * f64::EPSILON)
        );
    }
    #[test]
    fn rays_add_ray() {
        let mut rays = Rays::default();
        assert_eq!(rays.rays.len(), 0);
        let ray = Ray::new_collimated(
            Point2::new(0.0, 0.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray);
        assert_eq!(rays.rays.len(), 1);
    }
    #[test]
    fn rays_total_energy() {
        let mut rays = Rays::default();
        assert!(rays.total_energy().is_zero());
        let ray = Ray::new_collimated(
            Point2::new(0.0, 0.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), Energy::new::<joule>(1.0));
        rays.add_ray(ray.clone());
        assert_eq!(rays.total_energy(), Energy::new::<joule>(2.0));
    }
    #[test]
    fn rays_propagate_along_z_axis() {
        let mut rays = Rays::default();
        let ray0 = Ray::new_collimated(
            Point2::new(0.0, 0.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
    let ray1 = Ray::new_collimated(
        Point2::new(0.0, 1.0),
        Length::new::<nanometer>(1053.0),
        Energy::new::<joule>(1.0),
    )
    .unwrap();
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        rays.propagate_along_z(1.0).unwrap();
        assert_eq!(rays.rays[0].position(), Point3::new(0.0,0.0,1.0));
        assert_eq!(rays.rays[1].position(), Point3::new(0.0,1.0,1.0));
    }
}
