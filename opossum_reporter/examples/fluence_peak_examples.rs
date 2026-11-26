//! Computes the example cuts of the handbook section "Peak Fluence and Interpretation": cuts
//! through fluence maps of OPOSSUM's estimators along the x axis, next to the exact fluence,
//! written as SVG into `doc/book/src/images`. Run it from the workspace root:
//!
//! `cargo run -p opossum_reporter --example fluence_peak_examples`
use nalgebra::{MatrixXx2, Point3};
use opossum_core::{
    core_optics::hit_map::{
        fluence_estimator::FluenceEstimator,
        rays_hit_map::{HitPoint, RaysHitMap},
    },
    distributions::{
        energy::General2DGaussian,
        position::{FibonacciEllipse, Hexapolar},
    },
    light::{RayMesh, Rays},
    nodes::fluence_detector::fluence_data::FluenceData,
    prelude::*,
    utils::AxLims,
};
use opossum_reporter::plotting::{
    PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, PltBackEnd,
};
use plotters::style::RGBAColor;
use std::{f64::consts::PI, sync::Arc};
use uom::si::{f64::Length, length::millimeter, radiant_exposure::joule_per_square_centimeter};

/// Where the figures go, seen from the workspace root.
const IMAGES: &str = "./doc/book/src/images/";
/// The exact fluence.
const EXACT: RGBAColor = RGBAColor(26, 32, 44, 1.0);
/// An estimate that goes wrong.
const BAD: RGBAColor = RGBAColor(229, 62, 62, 1.0);
/// An estimate on the way.
const MIDDLE: RGBAColor = RGBAColor(221, 107, 32, 1.0);
/// An estimate that goes well.
const GOOD: RGBAColor = RGBAColor(43, 108, 176, 1.0);

/// A cut through a fluence map: points of x, in mm, and the fluence there, in J/cm².
type Cut = Vec<(f64, f64)>;

fn main() -> OpmResult<()> {
    few_and_many_rays()?;
    binning_grids()?;
    kde_kernels()?;
    an_edge()?;
    a_fold()?;
    Ok(())
}

/// A Gaussian beam whose peak lies 1 mm off the axis, drawn over its ray tubes from few and from
/// many rays: with few rays the peak falls between them and comes out low, as between two rays
/// the map runs linearly.
fn few_and_many_rays() -> OpmResult<()> {
    let map_of = |rays: usize| -> OpmResult<FluenceData> {
        let x = millimeter!(-8.0)..millimeter!(8.0);
        let y = millimeter!(-0.001)..millimeter!(0.001);
        hits(&gaussian_beam(rays, 1.0)?, Some)?.calc_fluence_map(
            (401, 3),
            &FluenceEstimator::Voronoi,
            Some(&x),
            Some(&y),
        )
    };
    let few = map_of(50)?;
    plot(
        "fluence_peak_rays",
        (-8.0, 8.0),
        (0.0, 4.5),
        &[
            (&row_through_axis(&few)?, BAD, "Voronoi, 50 rays"),
            (
                &corrected_row_through_axis(&few)?,
                MIDDLE,
                "Voronoi, 50 rays, plus the estimated deviation",
            ),
            (
                &row_through_axis(&map_of(2000)?)?,
                GOOD,
                "Voronoi, 2000 rays",
            ),
            (&exact(|x| gaussian(x - 1.0), -8.0, 8.0), EXACT, "exact"),
        ],
    )
}

/// A Gaussian beam of 10000 rays binned on a coarse, a fitting and a fine grid.
fn binning_grids() -> OpmResult<()> {
    let hits = hits(&gaussian_beam(10_000, 0.0)?, Some)?;
    let binned = |points: usize| -> OpmResult<Cut> {
        let range = millimeter!(-8.5)..millimeter!(8.5);
        let map = hits.calc_fluence_map(
            (points, points),
            &FluenceEstimator::Binning,
            Some(&range),
            Some(&range),
        )?;
        row_through_axis(&map)
    };
    plot(
        "fluence_peak_binning",
        (-8.5, 8.5),
        (0.0, 8.0),
        &[
            (&binned(301)?, MIDDLE, "Binning, 301 × 301 points: too fine"),
            (&binned(9)?, BAD, "Binning, 9 × 9 points: too coarse"),
            (&binned(41)?, GOOD, "Binning, 41 × 41 points"),
            (&exact(gaussian, -8.5, 8.5), EXACT, "exact"),
        ],
    )
}

/// A Gaussian beam by KDE, whose kernel follows the spread and the number of the hits.
fn kde_kernels() -> OpmResult<()> {
    let kde = |rays: usize| {
        cut(
            &hits(&gaussian_beam(rays, 0.0)?, Some)?,
            FluenceEstimator::KDE,
            8.0,
            201,
        )
    };
    plot(
        "fluence_peak_kde",
        (-8.0, 8.0),
        (0.0, 4.5),
        &[
            (&kde(100)?, BAD, "KDE, 100 rays"),
            (&kde(1000)?, MIDDLE, "KDE, 1000 rays"),
            (&kde(10000)?, GOOD, "KDE, 10000 rays"),
            (&exact(gaussian, -8.0, 8.0), EXACT, "exact"),
        ],
    )
}

/// A flat-top beam of 1 J/cm² whose middle a bore of 2.75 mm radius cut away, drawn from rays
/// 1 mm and 0.2 mm apart: the edge of the map lies within half a ray spacing of the cut.
fn an_edge() -> OpmResult<()> {
    let bore = 2.75;
    let ring = |rings: u8| -> OpmResult<Cut> {
        let beam = flat_top_beam(rings)?;
        let hits = hits(&beam, |p| {
            (p.x.get::<millimeter>().hypot(p.y.get::<millimeter>()) >= bore).then_some(p)
        })?;
        cut(&hits, FluenceEstimator::Voronoi, 6.0, 601)
    };
    let cut_away = |x: f64| if x.abs() < bore { 0.0 } else { 1.0 };
    plot(
        "fluence_peak_edge",
        (-6.0, 6.0),
        (0.0, 1.3),
        &[
            (&ring(8)?, BAD, "Voronoi, rays 1 mm apart"),
            (&ring(40)?, GOOD, "Voronoi, rays 0.2 mm apart"),
            (&exact(cut_away, -6.0, 6.0), EXACT, "exact"),
        ],
    )
}

/// A flat-top beam of 1 J/cm², folded over itself by moving each ray from x to x − x³/48 (in
/// mm), as near a caustic: at the fold, x = 4 mm moves to 2.67 mm, the exact fluence has no
/// finite peak, and the peak of the map grows the denser the rays are.
fn a_fold() -> OpmResult<()> {
    let fold = |x: f64| x - x.powi(3) / 48.0;
    let folded = |rings: u8| -> OpmResult<Cut> {
        let beam = flat_top_beam_of(5.0, rings)?;
        let hits = hits(&beam, |p| {
            Some(Point3::new(
                millimeter!(fold(p.x.get::<millimeter>())),
                p.y,
                p.z,
            ))
        })?;
        cut(&hits, FluenceEstimator::Voronoi, 3.5, 701)
    };
    // the exact fluence: the light of every x that moves to the given place, over how much the
    // move stretched it there
    let exact_fold = |to: f64| {
        let at = |step: u32| f64::from(step).mul_add(0.0005, -5.0);
        (0..20_000)
            .filter_map(|step| {
                let (a, b) = (at(step), at(step + 1));
                ((fold(a) - to) * (fold(b) - to) <= 0.0).then(|| {
                    let x = f64::midpoint(a, b);
                    1.0 / (1.0 - x * x / 16.0).abs()
                })
            })
            .sum::<f64>()
    };
    plot(
        "fluence_peak_fold",
        (1.5, 3.2),
        (0.0, 32.0),
        &[
            (&folded(10)?, BAD, "Voronoi, rays 0.5 mm apart"),
            (&folded(50)?, GOOD, "Voronoi, rays 0.1 mm apart"),
            (&exact(exact_fold, -3.5, 3.5), EXACT, "exact"),
        ],
    )
}

/// The given rays as an analysis sends them out: their energy spread over the area each stands
/// for, spanning their ray tubes.
fn as_sent_out(mut rays: Rays) -> OpmResult<Rays> {
    let mesh = Arc::new(RayMesh::new(&rays)?);
    rays.spread_energy_over_cells(&mesh)?;
    rays.attach(&mesh);
    Ok(rays)
}
/// A collimated Gaussian beam of 1 J with a standard deviation of 2 mm, its peak `center` mm
/// along x, of the given number of rays spread evenly over a disk of 8 mm radius, as an analysis
/// sends it out.
fn gaussian_beam(rays: usize, center: f64) -> OpmResult<Rays> {
    as_sent_out(Rays::new_collimated(
        nanometer!(1000.0),
        &General2DGaussian::new(
            joule!(1.0),
            millimeter!(center, 0.0),
            millimeter!(2.0, 2.0),
            1.0,
            degree!(0.0),
            false,
        )?,
        &FibonacciEllipse::new(millimeter!(8.0), millimeter!(8.0), rays)?,
    )?)
}
/// The exact fluence of [`gaussian_beam`] at `x` mm on the x axis, in J/cm².
fn gaussian(x: f64) -> f64 {
    let sigma = 0.2;
    (-(x / 10.0).powi(2) / (2.0 * sigma * sigma)).exp() / (2.0 * PI * sigma * sigma)
}
/// A collimated flat-top beam of 1 J/cm² in hexapolar rings out to 8 mm, spanning its ray tubes.
fn flat_top_beam(rings: u8) -> OpmResult<Rays> {
    flat_top_beam_of(8.0, rings)
}
/// A collimated flat-top beam of 1 J/cm² in hexapolar rings out to the given radius in mm, as
/// an analysis sends it out. Its outermost ring stands for half a ring spacing beyond it, so the
/// beam carries 1 J/cm² over a disk that much wider.
fn flat_top_beam_of(radius: f64, rings: u8) -> OpmResult<Rays> {
    let reach = radius * (1.0 + 0.5 / f64::from(rings)) / 10.0;
    as_sent_out(Rays::new_uniform_collimated(
        nanometer!(1000.0),
        joule!(PI * reach * reach),
        &Hexapolar::new(millimeter!(radius), rings)?,
    )?)
}
/// The hits of the given rays where `place` puts the point they start at; a ray it puts nowhere
/// does not hit.
fn hits(
    rays: &Rays,
    place: impl Fn(Point3<Length>) -> Option<Point3<Length>>,
) -> OpmResult<RaysHitMap> {
    let mut hits = RaysHitMap::default();
    for ray in rays {
        if let Some(at) = place(ray.position()) {
            hits.add_hit_point(HitPoint::new(at, ray.energy())?, ray.source())?;
        }
    }
    Ok(hits)
}
/// The cut along the x axis of a map of the given estimator, of `points` points from
/// −`half_width` to `half_width` mm.
fn cut(
    hits: &RaysHitMap,
    estimator: FluenceEstimator,
    half_width: f64,
    points: usize,
) -> OpmResult<Cut> {
    let x = millimeter!(-half_width)..millimeter!(half_width);
    let y = millimeter!(-0.001)..millimeter!(0.001);
    row_through_axis(&hits.calc_fluence_map((points, 3), &estimator, Some(&x), Some(&y))?)
}
/// The middle row of the given map, which runs along the x axis.
fn row_through_axis(map: &FluenceData) -> OpmResult<Cut> {
    let (xs, _, values) = map.get_fluence_distribution()?;
    let row = map.len_y() / 2;
    Ok(xs
        .iter()
        .enumerate()
        .map(|(column, x)| {
            (
                x.get::<millimeter>(),
                values[(row, column)].get::<joule_per_square_centimeter>(),
            )
        })
        .collect())
}
/// The middle row of the given map plus the deviation of the fluence from it that the map
/// estimates (see the handbook, "Uncertainty").
fn corrected_row_through_axis(map: &FluenceData) -> OpmResult<Cut> {
    let deviation = map
        .deviation()
        .ok_or_else(|| OpossumError::Other("the map estimates no deviation".into()))?;
    let row = map.len_y() / 2;
    Ok(row_through_axis(map)?
        .into_iter()
        .enumerate()
        .map(|(column, (x, fluence))| {
            let deviation = deviation
                .get((row, column))
                .map_or(0.0, |d| d.get::<joule_per_square_centimeter>());
            (x, fluence + deviation)
        })
        .collect())
}
/// The given exact fluence from `from` to `to` mm, in 801 points.
fn exact(fluence: impl Fn(f64) -> f64, from: f64, to: f64) -> Cut {
    (0..=800)
        .map(|i| {
            let x = from + (to - from) * f64::from(i) / 800.0;
            (x, fluence(x))
        })
        .collect()
}
/// Writes the given curves into one plot `name`.svg among the images of the handbook, showing
/// `x` from the first to the second value in mm and the fluence from the first to the second
/// value in J/cm². Prints the peak of each curve.
fn plot(
    name: &str,
    (x_from, x_to): (f64, f64),
    (fluence_from, fluence_to): (f64, f64),
    curves: &[(&Cut, RGBAColor, &str)],
) -> OpmResult<()> {
    let mut params = PlotParameters::default();
    params
        .set(&PlotArgs::FName(format!("{name}.svg")))?
        .set(&PlotArgs::FDir(IMAGES.into()))?
        .set(&PlotArgs::Backend(PltBackEnd::SVG))?
        .set(&PlotArgs::PlotAutoSize(false))?
        .set(&PlotArgs::PlotSize((720, 400)))?
        .set(&PlotArgs::AxisEqual(false))?
        .set(&PlotArgs::XLim(AxLims::new(x_from, x_to)))?
        .set(&PlotArgs::YLim(AxLims::new(fluence_from, fluence_to)))?
        .set(&PlotArgs::XLabel("x in mm".into()))?
        .set(&PlotArgs::YLabel("fluence in J/cm²".into()))?;
    let series = curves
        .iter()
        .map(|(cut, color, label)| {
            let (at, peak) = cut
                .iter()
                .copied()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap_or_default();
            let low = cut.iter().map(|(_, f)| *f).fold(f64::INFINITY, f64::min);
            println!("{name}: {label}: peak {peak:.3} J/cm² at {at:.3} mm, lowest {low:.3}");
            let shown = cut
                .iter()
                .filter(|(x, _)| (x_from..=x_to).contains(x))
                .collect::<Vec<_>>();
            let points = MatrixXx2::from_row_iterator(
                shown.len(),
                shown.iter().flat_map(|(x, f)| [*x, f.min(1e3)]),
            );
            Ok(PlotSeries::new(
                &PlotData::new_dim2(points)?,
                *color,
                Some((*label).into()),
            ))
        })
        .collect::<OpmResult<Vec<_>>>()?;
    PlotType::Line2D(params).plot(&series)?;
    Ok(())
}
