use criterion::{criterion_group, criterion_main, Criterion};
use opossum::{
    joule, millimeter,
    position_distributions::{Hexapolar, PositionDistribution, SobolDist},
    surface::hit_map::{
        fluence_estimator::FluenceEstimator,
        rays_hit_map::{EnergyHitPoint, HitPoint, RaysHitMap},
    },
};
use uom::si::f64::Ratio;

fn criterion_kde(c: &mut Criterion) {
    let distribution = Hexapolar::new(millimeter!(50.0), 3).unwrap();
    let points = distribution.generate();
    let weight = joule!(1.0) / Ratio::new::<uom::si::ratio::ratio>(points.len() as f64);
    let mut hit_map = RaysHitMap::default();
    for p in points {
        let hit_point = HitPoint::Energy(EnergyHitPoint::new(p, weight).unwrap());
        hit_map.add_hit_point(hit_point).unwrap();
    }
    c.bench_function("kde", |b| {
        b.iter(|| hit_map.calc_fluence_map((30, 30), &FluenceEstimator::KDE, None, None))
    });
}

fn criterion_binning(c: &mut Criterion) {
    let distribution = SobolDist::new(millimeter!(50.0), millimeter!(50.0), 10000).unwrap();
    let points = distribution.generate();
    let weight = joule!(1.0) / Ratio::new::<uom::si::ratio::ratio>(points.len() as f64);
    let mut hit_map = RaysHitMap::default();
    for p in points {
        let hit_point = HitPoint::Energy(EnergyHitPoint::new(p, weight).unwrap());
        hit_map.add_hit_point(hit_point).unwrap();
    }
    c.bench_function("binning", |b| {
        b.iter(|| hit_map.calc_fluence_map((30, 30), &FluenceEstimator::Binning, None, None))
    });
}

criterion_group!(benches, criterion_kde, criterion_binning);
criterion_main!(benches);
