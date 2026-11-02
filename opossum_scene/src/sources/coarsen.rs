//! Making a source drawable: at most so many rays, each shining one wavelength.

use opossum_core::{
    distributions::spectral::LaserLines,
    error::{OpmResult, OpossumError},
    light::{Ray, Rays, lightdata::ray_data_source::RayDataSource},
};
use uom::si::f64::Length;

use super::{distributions::coarsened_distribution, thinning::thinned};

/// Returns the rays of a source, at most `max_rays` of them, for drawing it.
///
/// A collimated or point source keeps its energy and the area it covers but gets a coarser
/// position distribution (see [`coarsened_distribution`]), and each of its points shines one
/// wavelength of the spectrum instead of all of them (see [`dealt_over_spectrum`]). So every
/// wavelength is still drawn, while the rays do not multiply with the samples of the spectrum.
/// Raw rays and images are built and thinned to `max_rays` rays instead (see [`thinned`]).
///
/// # Arguments
///
/// - `source`: the source to coarsen
/// - `max_rays`: the most rays; zero counts as one
///
/// # Returns
///
/// Raw rays, built from the coarser source.
///
/// # Errors
///
/// This function returns an error if the coarser distribution, the spectrum or the rays cannot
/// be built.
pub(super) fn coarsened(source: &RayDataSource, max_rays: usize) -> OpmResult<RayDataSource> {
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
    // Built at one wavelength, one ray per point; the spectrum is dealt out afterwards.
    let mut one_line = source.clone();
    one_line.set_pos_dist(coarsened_distribution(&pos_dist, max)?);
    one_line.set_spectral_dist(LaserLines::new(vec![(any_wavelength, 1.0)])?.into());
    Ok(RayDataSource::Raw(dealt_over_spectrum(
        &one_line.build()?,
        &samples,
    )?))
}

/// Returns rays with the wavelengths of a spectrum dealt out among them, one to each.
///
/// A source of many spectral samples drawn with all of them at every point would multiply its
/// rays by their number. Dealt out, every wavelength is still drawn:
///
/// - Each wavelength goes to equally many rays, give or take one. With fewer rays than
///   wavelengths, the spectrum is cut into as many equal parts as there are rays, and each ray
///   takes the wavelength in the middle of its part.
/// - Which ray gets which wavelength is shuffled with a fixed seed. The colors then follow no
///   pattern of how the rays were laid out - rings, a grid, a Fibonacci spiral - and the same
///   rays always get the same wavelengths.
///
/// Each ray is made anew where it starts and in its direction, with its energy weighted by its
/// wavelength's share, as the full source would weight it. Anything else a ray carries is
/// dropped, so these are meant to be rays fresh from a source.
///
/// # Arguments
///
/// - `rays`: the rays to deal the wavelengths to
/// - `spectrum`: the wavelengths and their shares, as a spectral distribution samples them
///
/// # Errors
///
/// This function returns an error if the spectrum holds no wavelength while there are rays, or
/// if a ray cannot be made at its wavelength.
fn dealt_over_spectrum(rays: &Rays, spectrum: &[(Length, f64)]) -> OpmResult<Rays> {
    /// Any seed does, as long as it stays the same.
    const SHUFFLE_SEED: u64 = 0x5eed;
    let nr_of_rays = rays.iter().len();
    let mut picks: Vec<usize> = (0..nr_of_rays)
        .map(|k| (2 * k + 1) * spectrum.len() / (2 * nr_of_rays))
        .collect();
    fastrand::Rng::with_seed(SHUFFLE_SEED).shuffle(&mut picks);
    let dealt = rays
        .iter()
        .zip(picks)
        .map(|(ray, pick)| {
            let (wavelength, share) = spectrum.get(pick).ok_or_else(|| {
                OpossumError::Other("a spectrum without a wavelength cannot be dealt".into())
            })?;
            Ray::new(
                ray.position(),
                ray.direction(),
                *wavelength,
                ray.energy() * *share,
            )
        })
        .collect::<OpmResult<Vec<_>>>()?;
    Ok(dealt.into())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::ten_lines;
    use approx::assert_relative_eq;
    use opossum_core::{
        distributions::{
            energy::UniformDist,
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
        let RayDataSource::Raw(coarse) = coarsened(&RayDataSource::Raw(rays), 3)? else {
            panic!("coarsened raw rays are still raw rays");
        };
        assert!((1..=3).contains(&coarse.nr_of_rays(false)));
        Ok(())
    }

    /// Asked for no rays at all, a source still keeps one ray.
    #[test]
    fn a_source_coarsened_to_nothing_keeps_one_ray() -> OpmResult<()> {
        let coarse = coarsened(&RayDataSource::default(), 0)?.build()?;
        assert_eq!(coarse.nr_of_rays(false), 1);
        Ok(())
    }

    /// Coarsened, a collimated source has at most the asked number of rays and one wavelength at
    /// each start point, and every line of its spectrum shines about equally often.
    #[test]
    fn each_point_of_a_coarsened_source_shines_one_wavelength() -> OpmResult<()> {
        let fine = Hexapolar::new(millimeter!(1.0), 15)?;
        let rays = coarsened(&collimated(fine.into(), ten_lines()?)?, 100)?.build()?;

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
        let rays = coarsened(&collimated(fine.into(), ten_lines()?)?, 100)?.build()?;

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
        let rays = coarsened(&source, 100)?.build()?;

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

    /// However the points are laid out, the rays stay within the asked number and shine as many
    /// lines as there are lines, or rays if those are fewer.
    #[test]
    fn every_distribution_is_coarsened_over_the_spectrum() -> OpmResult<()> {
        for dist in PosDistType::iter() {
            let rays = coarsened(&collimated(dist, ten_lines()?)?, 50)?.build()?;
            let nr_of_rays = rays.nr_of_rays(false);
            assert!((1..=50).contains(&nr_of_rays), "{dist}: {nr_of_rays}");
            assert_eq!(
                rays_per_wavelength(&rays).len(),
                nr_of_rays.min(10),
                "{dist}"
            );
        }
        Ok(())
    }

    /// With fewer rays than samples in the spectrum, the rays take wavelengths spread over all of
    /// it: each of as many equal parts of the spectrum as there are rays gives one.
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
        let rays = coarsened(&source, 7)?.build()?;

        let nr_of_rays = rays.nr_of_rays(false);
        assert_eq!(nr_of_rays, 7);
        let n = samples.len();
        for part in 0..nr_of_rays {
            let slice = &samples[part * n / nr_of_rays..(part + 1) * n / nr_of_rays];
            let in_part = rays
                .iter()
                .filter(|ray| slice.contains(&ray.wavelength()))
                .count();
            assert_eq!(in_part, 1, "part {part} of the spectrum");
        }
        Ok(())
    }

    /// Each ray carries the energy of its start point, weighted by its wavelength's share of the
    /// spectrum, as in the full source.
    #[test]
    fn a_coarsened_ray_carries_its_wavelengths_share_of_the_energy() -> OpmResult<()> {
        let lines = LaserLines::new(vec![(nanometer!(500.0), 1.0), (nanometer!(800.0), 3.0)])?;
        let source = collimated(Hexapolar::new(millimeter!(1.0), 1)?.into(), lines.into())?;
        let rays = coarsened(&source, 7)?.build()?;

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
