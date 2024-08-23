use opossum::{
    energy_distributions::UniformDist, error::OpmResult, joule, millimeter, nanometer,
    position_distributions::Hexapolar, rays::Rays,
};
use uom::si::radiant_exposure::joule_per_square_centimeter;

fn main() -> OpmResult<()> {
    let rays = Rays::new_collimated(
        nanometer!(1000.),
        &UniformDist::new(joule!(1.))?,
        &Hexapolar::new(millimeter!(10.), 10)?,
    )?;

    let fluence_data = rays.calc_fluence_at_position()?;
    // let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();
    println!(
        "{}",
        fluence_data
            .get_peak_fluence()
            .get::<joule_per_square_centimeter>()
    );
    println!(
        "{}",
        fluence_data
            .get_average_fluence()
            .get::<joule_per_square_centimeter>()
    );

    Ok(())
}
