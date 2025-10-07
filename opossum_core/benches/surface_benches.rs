// benches/surface_benches.rs

use criterion::{Criterion, criterion_group, criterion_main};
use nalgebra::Vector3;
use opossum_core::{
    joule, millimeter, nanometer,
    ray::Ray,
    surface::{Cylinder, Parabola, Plane, Sphere, geo_surface::GeoSurface},
    utils::geom_transformation::Isometry,
};
use std::time::Duration;

// Function to configure Criterion for more stable measurements
fn criterion_config() -> Criterion {
    Criterion::default()
        // We can increase the measurement time from the default of 5s to 15s.
        // This gives the benchmark more time to collect data.
        .measurement_time(Duration::from_secs(15))
        // We can also increase the warm-up time from the default of 3s to 5s.
        // This helps the CPU caches to get "hot" before measurements start.
        .warm_up_time(Duration::from_secs(5))
        // We can increase the sample size from the default of 100 to 150.
        // More samples can lead to better statistical analysis.
        .sample_size(150)
}

/// Benchmarks for the Plane surface
fn plane_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Plane Intersection");
    let surface = Plane::new(Isometry::new_along_z(millimeter!(10.0)).unwrap());

    // --- Scenarios ---
    let on_axis_ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    let off_axis_ray =
        Ray::new_collimated(millimeter!(2.0, 2.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();
    let angled_ray = Ray::new(
        millimeter!(0.0, 0.0, 0.0),
        Vector3::new(0.1, -0.1, 1.0),
        nanometer!(1053.0),
        joule!(1.0),
    )
    .unwrap();
    let miss_ray =
        Ray::new_collimated(millimeter!(0.0, 0.0, 20.0), nanometer!(1053.0), joule!(1.0)).unwrap();

    // --- Benchmarks ---
    group.bench_function("on-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&on_axis_ray))
    });
    group.bench_function("off-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&off_axis_ray))
    });
    group.bench_function("angled hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&angled_ray))
    });
    group.bench_function("miss", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&miss_ray))
    });
    group.finish();
}

/// Benchmarks for the Sphere surface
fn sphere_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Sphere Intersection");
    // Sphere with vertex at z=20mm
    let surface = Sphere::new(
        millimeter!(10.0),
        Isometry::new_along_z(millimeter!(20.0)).unwrap(),
    )
    .unwrap();

    // --- Scenarios ---
    let on_axis_ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    let off_axis_ray =
        Ray::new_collimated(millimeter!(5.0, 0.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();
    let angled_ray = Ray::new(
        millimeter!(0.0, 0.0, 0.0),
        Vector3::new(0.1, 0.0, 1.0), // Angled towards the surface
        nanometer!(1053.0),
        joule!(1.0),
    )
    .unwrap();
    // This ray is outside the sphere's radius of 10mm
    let miss_ray =
        Ray::new_collimated(millimeter!(10.1, 0.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();

    // --- Benchmarks ---
    group.bench_function("on-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&on_axis_ray))
    });
    group.bench_function("off-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&off_axis_ray))
    });
    group.bench_function("angled hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&angled_ray))
    });
    group.bench_function("miss", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&miss_ray))
    });
    group.finish();
}

/// Benchmarks for the Cylinder surface
fn cylinder_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cylinder Intersection");
    let surface = Cylinder::new(
        millimeter!(10.0),
        Isometry::new_along_z(millimeter!(20.0)).unwrap(),
    )
    .unwrap();

    // --- Scenarios ---
    let on_axis_ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    let off_axis_ray =
        Ray::new_collimated(millimeter!(5.0, 5.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();
    let angled_ray = Ray::new(
        millimeter!(0.0, 0.0, 0.0),
        Vector3::new(0.2, 0.2, 1.0), // Angled towards the surface
        nanometer!(1053.0),
        joule!(1.0),
    )
    .unwrap();
    // This ray is outside the cylinder's radius of 10mm on the curved axis
    let miss_ray =
        Ray::new_collimated(millimeter!(10.1, 0.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();

    // --- Benchmarks ---
    group.bench_function("on-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&on_axis_ray))
    });
    group.bench_function("off-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&off_axis_ray))
    });
    group.bench_function("angled hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&angled_ray))
    });
    group.bench_function("miss", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&miss_ray))
    });
    group.finish();
}

/// Benchmarks for the Parabola surface
fn parabola_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Parabola Intersection");
    let surface = Parabola::new(
        millimeter!(20.0),                                // focal length
        Isometry::new_along_z(millimeter!(5.0)).unwrap(), // vertex at z=5mm
    )
    .unwrap();

    // --- Scenarios ---
    // All rays start behind the parabola's vertex
    let on_axis_ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    let off_axis_ray =
        Ray::new_collimated(millimeter!(5.0, -5.0, 0.0), nanometer!(1053.0), joule!(1.0)).unwrap();
    let angled_ray = Ray::new(
        millimeter!(0.0, 0.0, 0.0),
        Vector3::new(-0.1, 0.1, 1.0), // Angled towards the surface
        nanometer!(1053.0),
        joule!(1.0),
    )
    .unwrap();
    // This ray starts "behind" the surface and travels away from it
    let miss_ray = Ray::new(
        millimeter!(0.0, 0.0, 6.0),
        Vector3::z(),
        nanometer!(1053.0),
        joule!(1.0),
    )
    .unwrap();

    // --- Benchmarks ---
    group.bench_function("on-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&on_axis_ray))
    });
    group.bench_function("off-axis hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&off_axis_ray))
    });
    group.bench_function("angled hit", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&angled_ray))
    });
    group.bench_function("miss", |b| {
        b.iter(|| surface.calc_intersect_and_normal(&miss_ray))
    });
    group.finish();
}

// Register the benchmark groups using the custom configuration from the function above.
criterion_group!(
    name = benches;
    config = criterion_config();
    targets = plane_benchmarks, sphere_benchmarks, cylinder_benchmarks, parabola_benchmarks
);
criterion_main!(benches);
