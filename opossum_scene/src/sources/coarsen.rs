//! Making a source drawable: at most so many rays, with every wavelength of its spectrum drawn.

use opossum_core::{
    distributions::spectral::LaserLines,
    error::{OpmResult, OpossumError},
    light::{Ray, Rays, lightdata::ray_data_source::RayDataSource},
};
use uom::si::f64::Length;

use super::{distributions::coarsened_distribution, thinning::thinned};

/// How the spectrum of a source is laid on the rays drawn of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrumDrawing {
    /// Each start point shines one wavelength, the wavelengths dealt out evenly among them. Many
    /// start points fit, and every wavelength is drawn without the rays multiplying with the
    /// samples of the spectrum.
    #[default]
    Dealt,
    /// Each start point shines every wavelength, as in the analysis. Fewer start points fit, and a
    /// spectrum of more samples than rays is thinned evenly.
    Full,
}

/// Returns the rays of a source, at most `max_rays` of them, for drawing it.
///
/// A collimated or point source keeps its energy and the area it covers but gets a coarser
/// position distribution (see [`coarsened_distribution`]), and its spectrum is laid on the points
/// as `drawing` says: one wavelength to each point (see [`dealt_over_spectrum`]), or every
/// wavelength at every point, with as many fewer points (see [`with_every_wavelength`]). Raw rays
/// and images are built and thinned to `max_rays` rays instead (see [`thinned`]).
///
/// # Arguments
///
/// - `source`: the source to coarsen
/// - `max_rays`: the most rays; zero counts as one
/// - `drawing`: how the spectrum is laid on the rays
///
/// # Returns
///
/// Raw rays, built from the coarser source.
///
/// # Errors
///
/// This function returns an error if the coarser distribution, the spectrum or the rays cannot
/// be built.
pub(super) fn coarsened(
    source: &RayDataSource,
    max_rays: usize,
    drawing: SpectrumDrawing,
) -> OpmResult<RayDataSource> {
    let max = max_rays.max(1);
    let (Some(pos_dist), Some(spectrum)) = (
        source.get_position_distribution_type(),
        source.get_spectral_distribution_type(),
    ) else {
        return Ok(RayDataSource::Raw(thinned(&source.clone().build()?, max)));
    };
    let samples = spectrum.generate().generate()?;
    let Some(&(any_wavelength, _)) = samples.first() else {
        return Err(OpossumError::Other(
            "a spectrum without a wavelength cannot be drawn".into(),
        ));
    };
    let points = match drawing {
        SpectrumDrawing::Dealt => max,
        SpectrumDrawing::Full => max / samples.len().min(max),
    };
    // Built at one wavelength, one ray per point; the spectrum is laid on afterwards.
    let mut one_line = source.clone();
    one_line.set_pos_dist(coarsened_distribution(&pos_dist, points)?);
    one_line.set_spectral_dist(LaserLines::new(vec![(any_wavelength, 1.0)])?.into());
    let rays = one_line.build()?;
    Ok(RayDataSource::Raw(match drawing {
        SpectrumDrawing::Dealt => dealt_over_spectrum(&rays, &samples)?,
        SpectrumDrawing::Full => with_every_wavelength(&rays, &samples, max)?,
    }))
}

/// Returns rays with the wavelengths of a spectrum dealt out among them, one to each.
///
/// A source of many spectral samples drawn with all of them at every point would multiply its
/// rays by their number. Dealt out, every wavelength is still drawn:
///
/// - Each wavelength goes to equally many rays, give or take one. With fewer rays than
///   wavelengths, the rays take wavelengths spread evenly over the spectrum (see
///   [`spread_samples`]).
/// - Which ray gets which wavelength is shuffled with a fixed seed. The colors then follow no
///   pattern of how the rays were laid out - rings, a grid, a Fibonacci spiral - and the same
///   rays always get the same wavelengths.
///
/// Each ray is made anew (see [`ray_at`]), so these are meant to be rays fresh from a source.
///
/// # Arguments
///
/// - `rays`: the rays to deal the wavelengths to
/// - `spectrum`: the wavelengths and their shares, as a spectral distribution samples them
///
/// # Errors
///
/// This function returns an error if a ray cannot be made at its wavelength.
fn dealt_over_spectrum(rays: &Rays, spectrum: &[(Length, f64)]) -> OpmResult<Rays> {
    /// Any seed does, as long as it stays the same.
    const SHUFFLE_SEED: u64 = 0x5eed;
    let mut samples = spread_samples(spectrum, rays.iter().len());
    fastrand::Rng::with_seed(SHUFFLE_SEED).shuffle(&mut samples);
    let dealt = rays
        .iter()
        .zip(samples)
        .map(|(ray, sample)| ray_at(ray, sample))
        .collect::<OpmResult<Vec<_>>>()?;
    Ok(dealt.into())
}

/// Returns rays with every wavelength of a spectrum at each of their start points, as the full
/// source would have them.
///
/// A spectrum of more samples than `max_rays` is thinned to that many first, spread evenly over
/// it (see [`spread_samples`]). Each ray is made anew (see [`ray_at`]), so these are meant to be
/// rays fresh from a source.
///
/// # Arguments
///
/// - `rays`: the rays to lay the spectrum on, one per start point
/// - `spectrum`: the wavelengths and their shares, as a spectral distribution samples them
/// - `max_rays`: the most wavelengths kept of the spectrum
///
/// # Errors
///
/// This function returns an error if a ray cannot be made at its wavelength.
fn with_every_wavelength(
    rays: &Rays,
    spectrum: &[(Length, f64)],
    max_rays: usize,
) -> OpmResult<Rays> {
    let samples = spread_samples(spectrum, spectrum.len().min(max_rays));
    let every = rays
        .iter()
        .flat_map(|ray| samples.iter().map(|sample| ray_at(ray, *sample)))
        .collect::<OpmResult<Vec<_>>>()?;
    Ok(every.into())
}

/// Returns `count` samples of a spectrum, spread evenly over it.
///
/// The spectrum is cut into `count` equal parts, and the sample in the middle of each is taken:
/// every sample once if `count` is the number of samples, each about equally often if it is more,
/// and samples evenly apart if it is fewer.
///
/// # Arguments
///
/// - `spectrum`: the wavelengths and their shares, as a spectral distribution samples them
/// - `count`: how many samples to take
///
/// # Returns
///
/// The samples taken, in the order of the spectrum; none if it has no sample.
fn spread_samples(spectrum: &[(Length, f64)], count: usize) -> Vec<(Length, f64)> {
    (0..count)
        .filter_map(|k| spectrum.get((2 * k + 1) * spectrum.len() / (2 * count)))
        .copied()
        .collect()
}

/// Makes a ray anew where `ray` starts and in its direction, at the wavelength of a sample and
/// with its energy weighted by the sample's share, as the full source would weight it.
///
/// # Arguments
///
/// - `ray`: the ray whose start and direction to take, at one wavelength of share one
/// - `sample`: the wavelength and its share
///
/// # Errors
///
/// This function returns an error if the ray cannot be made at the wavelength.
fn ray_at(ray: &Ray, (wavelength, share): (Length, f64)) -> OpmResult<Ray> {
    Ray::new(
        ray.position(),
        ray.direction(),
        wavelength,
        ray.energy() * share,
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::ten_lines;
    use approx::assert_relative_eq;
    use opossum_core::{
        distributions::{
            energy::{EnergyDistType, General2DGaussian, UniformDist},
            position::{Hexapolar, PosDistType},
            spectral::{Gaussian, SpecDistType},
        },
        joule,
        light::lightdata::ray_data_source::{CollimatedSrc, PointSrc},
        meter, millimeter, nanometer,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use strum::IntoEnumIterator;

    /// A collimated source of 1 J spread evenly over `pos_dist`, shining `spectrum`.
    fn collimated(pos_dist: PosDistType, spectrum: SpecDistType) -> OpmResult<RayDataSource> {
        Ok(RayDataSource::Collimated(CollimatedSrc::new(
            pos_dist,
            UniformDist::new(joule!(1.0))?.into(),
            spectrum,
        )))
    }

    /// How many rays shine each wavelength, keyed by the wavelength's bits.
    fn rays_per_wavelength(rays: &Rays) -> BTreeMap<u64, usize> {
        let mut counts = BTreeMap::new();
        for ray in rays.iter() {
            *counts.entry(ray.wavelength().value.to_bits()).or_insert(0) += 1;
        }
        counts
    }

    /// Raw rays have no distribution to coarsen, so they are thinned.
    #[test]
    fn coarsened_raw_rays_are_thinned() -> OpmResult<()> {
        let mut rays = Rays::default();
        for x in 0..10 {
            rays.add_ray(Ray::new_collimated(
                meter!(f64::from(x), 0.0, 0.0),
                nanometer!(1000.0),
                joule!(1.0),
            )?);
        }
        let raw = RayDataSource::Raw(rays);
        let RayDataSource::Raw(coarse) = coarsened(&raw, 3, SpectrumDrawing::Dealt)? else {
            panic!("coarsened raw rays are still raw rays");
        };
        assert!((1..=3).contains(&coarse.nr_of_rays(false)));
        Ok(())
    }

    /// Asked for no rays at all, a source still keeps one ray, however its spectrum is drawn.
    #[test]
    fn a_source_coarsened_to_nothing_keeps_one_ray() -> OpmResult<()> {
        for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
            let coarse = coarsened(&RayDataSource::default(), 0, drawing)?.build()?;
            assert_eq!(coarse.nr_of_rays(false), 1, "{drawing:?}");
        }
        Ok(())
    }

    /// Coarsened, a collimated source has at most the asked number of rays and one wavelength at
    /// each start point, and every line of its spectrum shines about equally often.
    #[test]
    fn each_point_of_a_coarsened_source_shines_one_wavelength() -> OpmResult<()> {
        let fine = Hexapolar::new(millimeter!(1.0), 15)?;
        let source = collimated(fine.into(), ten_lines()?)?;
        let rays = coarsened(&source, 100, SpectrumDrawing::Dealt)?.build()?;

        assert!(rays.nr_of_rays(false) <= 100);
        let starts: BTreeSet<_> = rays
            .iter()
            .map(|ray| {
                let p = ray.position();
                (
                    p.x.value.to_bits(),
                    p.y.value.to_bits(),
                    p.z.value.to_bits(),
                )
            })
            .collect();
        assert_eq!(
            starts.len(),
            rays.nr_of_rays(false),
            "one ray per start point"
        );
        let counts = rays_per_wavelength(&rays);
        assert_eq!(counts.len(), 10, "every line shines");
        let fewest = counts.values().min().copied().unwrap_or(0);
        let most = counts.values().max().copied().unwrap_or(0);
        assert!(most - fewest <= 1, "{fewest}..{most} rays per line");
        Ok(())
    }

    /// The colors follow no pattern of the layout: the center and the two rings around it - the
    /// first 19 points of a hexapolar distribution - shine many lines, not the shortest ones only.
    #[test]
    fn the_wavelengths_do_not_follow_the_layout_of_the_points() -> OpmResult<()> {
        let fine = Hexapolar::new(millimeter!(1.0), 15)?;
        let source = collimated(fine.into(), ten_lines()?)?;
        let rays = coarsened(&source, 100, SpectrumDrawing::Dealt)?.build()?;

        let mut by_radius: Vec<_> = rays
            .iter()
            .map(|ray| {
                let p = ray.position();
                (p.x.value.hypot(p.y.value), ray.wavelength().value.to_bits())
            })
            .collect();
        by_radius.sort_by(|a, b| a.0.total_cmp(&b.0));
        let inner: BTreeSet<_> = by_radius.iter().take(19).map(|(_, bits)| *bits).collect();
        assert!(inner.len() >= 5, "{} lines inside", inner.len());
        Ok(())
    }

    /// A point source sends one wavelength in each direction.
    #[test]
    fn each_direction_of_a_coarsened_point_source_shines_one_wavelength() -> OpmResult<()> {
        let source = RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(1.0), 15)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            ten_lines()?,
            meter!(1.0),
        )?);
        let rays = coarsened(&source, 100, SpectrumDrawing::Dealt)?.build()?;

        assert!(rays.nr_of_rays(false) <= 100);
        let directions: BTreeSet<_> = rays
            .iter()
            .map(|ray| {
                let d = ray.direction();
                (d.x.to_bits(), d.y.to_bits(), d.z.to_bits())
            })
            .collect();
        assert_eq!(directions.len(), rays.nr_of_rays(false));
        assert_eq!(rays_per_wavelength(&rays).len(), 10, "every line shines");
        Ok(())
    }

    /// However the points are laid out, the rays stay within the asked number. Dealt out, they
    /// shine as many lines as there are lines, or rays if those are fewer; in full, every line.
    #[test]
    fn every_distribution_is_coarsened_over_the_spectrum() -> OpmResult<()> {
        for dist in PosDistType::iter() {
            for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
                let rays = coarsened(&collimated(dist, ten_lines()?)?, 50, drawing)?.build()?;
                let nr_of_rays = rays.nr_of_rays(false);
                assert!(
                    (1..=50).contains(&nr_of_rays),
                    "{dist}, {drawing:?}: {nr_of_rays}"
                );
                let lines = match drawing {
                    SpectrumDrawing::Dealt => nr_of_rays.min(10),
                    SpectrumDrawing::Full => 10,
                };
                assert_eq!(
                    rays_per_wavelength(&rays).len(),
                    lines,
                    "{dist}, {drawing:?}"
                );
            }
        }
        Ok(())
    }

    /// In full, every start point shines every line of the spectrum, and the rays still stay
    /// within the asked number: ten lines leave room for at most ten start points.
    #[test]
    fn in_full_each_point_shines_every_wavelength() -> OpmResult<()> {
        let fine = Hexapolar::new(millimeter!(1.0), 15)?;
        let source = collimated(fine.into(), ten_lines()?)?;
        let rays = coarsened(&source, 100, SpectrumDrawing::Full)?.build()?;

        assert!(rays.nr_of_rays(false) <= 100);
        let mut lines_at: BTreeMap<(u64, u64), BTreeSet<u64>> = BTreeMap::new();
        for ray in rays.iter() {
            let p = ray.position();
            lines_at
                .entry((p.x.value.to_bits(), p.y.value.to_bits()))
                .or_default()
                .insert(ray.wavelength().value.to_bits());
        }
        assert!(lines_at.len() > 1, "more than the center fits");
        assert!(lines_at.values().all(|lines| lines.len() == 10));
        Ok(())
    }

    /// In full, the rays are those the source itself builds at the coarser points: the same
    /// starts, wavelengths and energies, in the same order, also where the energy is spread
    /// unevenly over the cross section.
    #[test]
    fn in_full_the_rays_are_those_of_the_source_at_fewer_points() -> OpmResult<()> {
        let lines = LaserLines::new(vec![(nanometer!(500.0), 1.0), (nanometer!(800.0), 3.0)])?;
        let fine = PosDistType::from(Hexapolar::new(millimeter!(1.0), 15)?);
        let source = RayDataSource::Collimated(CollimatedSrc::new(
            fine,
            EnergyDistType::General2DGaussian(General2DGaussian::default()),
            lines.into(),
        ));
        let drawn = coarsened(&source, 100, SpectrumDrawing::Full)?.build()?;

        let mut coarse = source.clone();
        coarse.set_pos_dist(coarsened_distribution(&fine, 50)?);
        let built = coarse.build()?;
        let rays_of = |rays: &Rays| {
            rays.iter()
                .map(|ray| (ray.position(), ray.wavelength(), ray.energy()))
                .collect::<Vec<_>>()
        };
        assert_eq!(rays_of(&drawn), rays_of(&built));
        Ok(())
    }

    /// With fewer rays than samples in the spectrum, the rays take wavelengths spread over all of
    /// it: each of as many equal parts of the spectrum as there are rays gives one. Dealt out,
    /// those are seven points of one wavelength each; in full, one point of seven wavelengths.
    #[test]
    fn fewer_rays_than_wavelengths_spread_over_the_spectrum() -> OpmResult<()> {
        let spectrum = SpecDistType::from(Gaussian::new(
            (nanometer!(1000.0), nanometer!(1100.0)),
            50,
            nanometer!(1050.0),
            nanometer!(20.0),
            1.0,
        )?);
        let samples: Vec<_> = spectrum
            .generate()
            .generate()?
            .into_iter()
            .map(|(wavelength, _)| wavelength)
            .collect();
        let source = collimated(Hexapolar::new(millimeter!(1.0), 1)?.into(), spectrum)?;
        for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
            let rays = coarsened(&source, 7, drawing)?.build()?;

            let nr_of_rays = rays.nr_of_rays(false);
            assert_eq!(nr_of_rays, 7, "{drawing:?}");
            let n = samples.len();
            for part in 0..nr_of_rays {
                let slice = &samples[part * n / nr_of_rays..(part + 1) * n / nr_of_rays];
                let in_part = rays
                    .iter()
                    .filter(|ray| slice.contains(&ray.wavelength()))
                    .count();
                assert_eq!(in_part, 1, "{drawing:?}: part {part} of the spectrum");
            }
        }
        Ok(())
    }

    /// Each ray carries the energy of its start point, weighted by its wavelength's share of the
    /// spectrum, as in the full source.
    #[test]
    fn a_coarsened_ray_carries_its_wavelengths_share_of_the_energy() -> OpmResult<()> {
        let lines = LaserLines::new(vec![(nanometer!(500.0), 1.0), (nanometer!(800.0), 3.0)])?;
        let source = collimated(Hexapolar::new(millimeter!(1.0), 1)?.into(), lines.into())?;
        let rays = coarsened(&source, 7, SpectrumDrawing::Dealt)?.build()?;

        for ray in rays.iter() {
            let share = if ray.wavelength() == nanometer!(500.0) {
                0.25
            } else {
                0.75
            };
            assert_relative_eq!(ray.energy().value, share / 7.0, epsilon = 1e-12);
        }
        Ok(())
    }
}
