use opossum::{error::OpmResult, plottable::Plottable, rays::Rays};
use std::path::Path;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let rays = Rays::new_uniform_collimated(
        Length::new::<millimeter>(1.0),
        Length::new::<nanometer>(1054.0),
        Energy::new::<joule>(1.0),
        &opossum::rays::DistributionStrategy::Random(200),
    )?;
    rays.to_svg_plot(Path::new("./opossum/playground/rays.svg"))?;
    Ok(())
}
