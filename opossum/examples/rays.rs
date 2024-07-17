use opossum::{
    error::OpmResult, joule, millimeter, nanometer, position_distributions::Random, rays::Rays,
};

fn main() -> OpmResult<()> {
    let _rays = Rays::new_uniform_collimated(
        nanometer!(1054.0),
        joule!(1.0),
        &Random::new(millimeter!(1.0), millimeter!(1.0), 200)?,
    )?;
    Ok(())
}
