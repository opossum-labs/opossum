use std::path::Path;

use opossum::{error::OpmResult, rays::Rays, plottable::Plottable};
use uom::si::{f64::{Length, Energy}, length::nanometer, energy::joule};

fn main() -> OpmResult<()> {
  let rays=Rays::new_uniform_collimated(1.0, Length::new::<nanometer>(1054.0), Energy::new::<joule>(1.0),opossum::rays::DistributionStrategy::Random(500));
  rays.to_svg_plot(Path::new("./playground/rays.svg"))?;
  Ok(())
}