use opossum::{error::OpmResult, position_distributions::Random, rays::Rays};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let _rays = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1054.0),
        Energy::new::<joule>(1.0),
        &Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(1.0),
            200,
        )?,
    )?;
    Ok(())
    // rays.to_svg_plot(Path::new("./opossum/playground/rays.svg"))?;
}
