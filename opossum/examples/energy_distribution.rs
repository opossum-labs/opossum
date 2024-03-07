use nalgebra::Point2;
use opossum::{
    energy_distributions::general_gaussian::General2DGaussian,
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotType},
    position_distributions::FibonacciRectangle,
    rays::Rays,
};
use uom::si::{
    angle::degree,
    energy::joule,
    f64::{Angle, Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let rays = Rays::new_collimated(
        Length::new::<nanometer>(1000.),
        &General2DGaussian::new(
            Energy::new::<joule>(3.),
            Point2::new(0., 0.),
            Point2::new(2., 2.),
            1.,
            Angle::new::<degree>(0.),
            false,
        )?,
        &FibonacciRectangle::new(
            Length::new::<millimeter>(10.),
            Length::new::<millimeter>(10.),
            1000,
        )?,
    )?;

    let fluence_data = rays.calc_fluence_at_position()?;
    let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("gaussian.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::FigSize((1000, 800)))
        .unwrap();
    let plt_dat = PlotData::ColorMesh(fl_x, fl_y, fl_d);
    let plt_type = PlotType::ColorMesh(plt_params);
    let _ = plt_type.plot(&plt_dat);

    Ok(())
}
