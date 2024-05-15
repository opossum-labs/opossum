use nalgebra::{DMatrix, DVector, Point2};
use opossum::{
    degree,
    energy_distributions::general_gaussian::General2DGaussian,
    error::OpmResult,
    joule, millimeter, nanometer,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType},
    position_distributions::FibonacciRectangle,
    rays::Rays,
};
use plotters::style::RGBAColor;
use uom::si::{length::millimeter, radiant_exposure::joule_per_square_centimeter};

fn main() -> OpmResult<()> {
    let rays = Rays::new_collimated(
        nanometer!(1000.),
        &General2DGaussian::new(
            joule!(3.),
            Point2::new(0., 0.),
            Point2::new(2., 2.),
            1.,
            degree!(0.),
            false,
        )?,
        &FibonacciRectangle::new(millimeter!(10.), millimeter!(10.), 1000)?,
    )?;

    let fluence_data = rays.calc_fluence_at_position()?;
    let (fl_x, fl_y, fl_d) = fluence_data.get_fluence_distribution();

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("gaussian.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::PlotSize((1000, 800)))
        .unwrap();
    let plt_dat = PlotData::ColorMesh {
        x_dat_n: DVector::from_iterator(
            fl_x.len(),
            fl_x.iter().map(uom::si::f64::Length::get::<millimeter>),
        ),
        y_dat_m: DVector::from_iterator(
            fl_y.len(),
            fl_y.iter().map(uom::si::f64::Length::get::<millimeter>),
        ),
        z_dat_nxm: DMatrix::from_iterator(
            fluence_data.len_y(),
            fluence_data.len_x(),
            fl_d.iter()
                .map(uom::si::f64::RadiantExposure::get::<joule_per_square_centimeter>),
        ),
    };
    let plt_type = PlotType::ColorMesh(plt_params);
    let plt_series = PlotSeries::new(&plt_dat, RGBAColor(0, 0, 0, 0.), None);

    let _ = plt_type.plot(&vec![plt_series]);

    Ok(())
}
