use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use nalgebra::{Point3, vector};
use num_traits::Zero;
use opossum_core::{
    J_per_cm2, analyzers::propagation_strategy::MissedSurfaceStrategy,
    core_optics::optic_surface::OpticSurface, degree, error::OpossumError, joule, light::Ray,
    millimeter, nanometer, utils::geom_transformation::Isometry,
};
use uom::si::f64::Length;

/// Configures Criterion for more stable measurements by increasing measurement time and sample size.
fn configure_criterion() -> Criterion {
    Criterion::default()
        // Increase the measurement time for each sample to average out short-term noise.
        // 5 seconds is a good starting point for more stable results.
        .measurement_time(Duration::from_secs(10))
        // Increase the number of samples collected. More samples provide better statistical data.
        .sample_size(200)
}

/// Benchmarks the `propagate` function, a fundamental operation.
fn bench_propagate(c: &mut Criterion) {
    c.bench_function("ray_propagate", |b| {
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            vector![0.1, 0.2, 1.0], // A non-trivial direction
            nanometer!(1053.0),
            joule!(1.0),
        )
        .expect("Setup of ray failed");
        let length = millimeter!(10.0);

        // Use iter_with_setup to exclude the clone operation from the measurement.
        b.iter_with_setup(
            || ray.clone(),
            |mut cloned_ray| {
                std::hint::black_box(cloned_ray.propagate(length))?;
                Ok::<(), OpossumError>(())
            },
        );
    });
}

/// Benchmarks the `refract_paraxial` function for ideal lens simulation.
fn bench_refract_paraxial(c: &mut Criterion) {
    c.bench_function("ray_refract_paraxial", |b| {
        let ray = Ray::new_collimated(
            millimeter!(1.0, 2.0, 0.0), // Off-axis ray
            nanometer!(1053.0),
            joule!(1.0),
        )
        .expect("Setup of ray failed");
        let focal_length = millimeter!(100.0);
        let iso = Isometry::identity();

        b.iter_with_setup(
            || ray.clone(),
            |mut cloned_ray| {
                // Use criterion::black_box to prevent the optimizer from removing the call.
                std::hint::black_box(cloned_ray.refract_paraxial(focal_length, &iso))?;
                Ok::<(), OpossumError>(())
            },
        );
    });
}

/// Benchmarks `refract_on_surface`, a complex and common operation involving Snell's law.
fn bench_refract_on_surface(c: &mut Criterion) {
    c.bench_function("ray_refract_on_surface", |b| {
        // Setup function to create fresh data for each measurement run.
        let setup = || {
            let ray = Ray::new_collimated(Point3::origin(), nanometer!(1054.0), joule!(1.0))
                .expect("error setting up ray");
            let plane_z_pos = millimeter!(10.0);
            let isometry = Isometry::new(
                Point3::new(Length::zero(), Length::zero(), plane_z_pos),
                degree!(0.0, 0.0, 0.0),
            )
            .expect("Setup of ray failed");
            let surface = OpticSurface::default();
            surface.set_isometry(isometry);
            let n2 = Some(1.5);
            let strategy = MissedSurfaceStrategy::Stop;
            (ray, surface, n2, strategy)
        };

        // iter_with_setup is used because the function requires mutable access to the ray and surface.
        b.iter_with_setup(setup, |(mut ray, mut surface, n2, strategy)| {
            std::hint::black_box(ray.refract_on_surface(&mut surface, n2, &strategy))?;
            Ok::<(), OpossumError>(())
        });
    });
}

/// Benchmarks `diffract_on_periodic_structure` for grating simulations.
fn bench_diffract_on_periodic_structure(c: &mut Criterion) {
    c.bench_function("ray_diffract_on_periodic_structure", |b| {
        let ray = Ray::new(
            Point3::origin(),
            vector![0.1, 0.0, 1.0], // Slight angle to avoid edge cases
            nanometer!(633.0),
            joule!(1.0),
        )
        .expect("Setup of ray failed");

        let plane_z_pos = millimeter!(10.0);
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), plane_z_pos),
            degree!(0.0, 0.0, 0.0),
        )
        .expect("Setup of isometry failed");
        let surface = OpticSurface::default();
        surface.set_isometry(isometry);

        let n2 = 1.0; // Reflection grating
        let grating_period = nanometer!(1000.0); // 1 micron period, 1000 lines/mm
        let grating_vector = vector![2.0 * std::f64::consts::PI / grating_period.value, 0.0, 0.0];
        let diffraction_order = 1;

        b.iter_with_setup(
            || (ray.clone(), surface.clone()),
            |(mut cloned_ray, cloned_surface)| {
                std::hint::black_box(cloned_ray.diffract_on_periodic_surface(
                    &cloned_surface,
                    n2,
                    grating_vector,
                    &diffraction_order,
                ))?;
                Ok::<(), OpossumError>(())
            },
        );
    });
}

/// Benchmarks `helper_ray_fluence`, which involves vector math on helper rays.
fn bench_helper_ray_fluence(c: &mut Criterion) {
    c.bench_function("ray_helper_ray_fluence", |b| {
        let mut ray = Ray::new_collimated_w_fluence_helper(
            Point3::origin(),
            nanometer!(1000.),
            joule!(1.),
            J_per_cm2!(1.),
        )
        .expect("Setup of ray failed");

        // Propagate the helper rays a bit to make the calculation non-trivial
        if let Some(helpers) = ray.helper_rays_mut() {
            for r in helpers {
                r.propagate(millimeter!(10.0))
                    .expect("Error while propagating");
                r.set_direction(vector![0.1, 0.1, 1.0])
                    .expect("Error setting ray direction");
                r.propagate(millimeter!(5.0))
                    .expect("Error while propagating");
            }
        }

        b.iter(|| {
            std::hint::black_box(ray.helper_ray_fluence());
        });
    });
}

// Define the group of benchmarks, now applying the custom configuration.
criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_propagate, bench_refract_paraxial, bench_refract_on_surface, bench_diffract_on_periodic_structure, bench_helper_ray_fluence
}
criterion_main!(benches);
