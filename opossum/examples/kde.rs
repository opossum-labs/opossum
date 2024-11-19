use opossum::{
    error::OpmResult,
    joule, millimeter,
    plottable::Plottable,
    position_distributions::{Hexapolar, PositionDistribution},
    surface::hit_map::{FluenceEstimator, HitPoint, RaysHitMap},
};
use std::path::Path;
use uom::si::f64::Ratio;

fn main() -> OpmResult<()> {
    let distribution = Hexapolar::new(millimeter!(50.0), 5)?;
    //let distribution = FibonacciEllipse::new(meter!(5.0), meter!(5.0), 91)?;
    let points = distribution.generate();
    let weight = joule!(1.0) / Ratio::new::<uom::si::ratio::ratio>(points.len() as f64);
    let mut hit_map = RaysHitMap::default();
    for p in points {
        let hit_point = HitPoint::new(p, weight)?;
        hit_map.add_hit_point(hit_point);
    }
    dbg!("Done Add HitMap");
    let fluence_data = hit_map.calc_fluence_map((100, 100), &FluenceEstimator::KDE)?;
    dbg!("Done KDE");
    fluence_data.to_plot(
        Path::new("./opossum/playground/kde.png"),
        opossum::plottable::PltBackEnd::Bitmap,
    )?;
    dbg!("Done PNG");
    Ok(())
}
