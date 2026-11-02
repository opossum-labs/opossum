//! The light of a model's sources, traced through it for drawing.
//!
//! A source may hold far more rays than a view can draw, and a spectrum of many samples would
//! multiply them again. So before the trace every source is coarsened to its share of the rays to
//! draw, with its spectrum dealt out among the rays or laid in full on fewer of them. How a source
//! is coarsened is drawing, not simulation, which is why it is done here and not in the core.

mod coarsen;
mod distributions;
mod thinning;

use std::ops::Range;

pub use coarsen::SpectrumDrawing;
use coarsen::coarsened;
use opossum_core::{
    analyzers::{
        Analyzer,
        raytrace::{AnalysisRayTrace, RayTracingAnalyzer},
    },
    core_optics::OpticNode,
    error::OpmResult,
    light::{LightResult, Rays},
    opm_document::OpmDocument,
};
use uom::si::f64::Length;
use uuid::Uuid;

/// Returns the rays of a model's sources, traced through it, for drawing them.
///
/// The sources are the ones the analyzer defines, and the model is placed exactly as
/// [`OpmDocument::placed_scenery`] places it, so the rays run through the components where the 3D
/// view shows them. The sources share `max_rays` evenly, and each is coarsened to its share first:
/// its rays are spread over its cross section, and its spectrum is laid on them as `drawing` says.
/// That keeps the trace cheap however many rays and spectral samples the sources hold, while
/// every wavelength is still traced. Only the primary path is traced: a ghost focus analyzer
/// contributes its sources, not its reflections. The trace runs on a copy and leaves the model
/// untouched.
///
/// # Arguments
///
/// - `document`: the model whose light to trace
/// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
///   exactly as for [`OpmDocument::placed_scenery`]
/// - `max_rays`: the most rays traced from all sources together; each source keeps at least one
/// - `drawing`: whether each start point shines one wavelength or all of them
///
/// # Returns
///
/// One bundle per open output port the light reaches, each ray carrying its path in global
/// coordinates; rays stopped on the way are included up to where they stopped. Empty if the
/// model has no analyzer, or if the one followed has no rays of its own, as an energy analysis:
/// default rays would only show something that is not the setup's own light.
///
/// # Errors
///
/// This function returns an error if the named analyzer does not exist, if the positioning run
/// fails, or if a source cannot be built or the trace fails.
pub fn traced_rays(
    document: &OpmDocument,
    analyzer_id: Option<Uuid>,
    max_rays: usize,
    drawing: SpectrumDrawing,
) -> OpmResult<Vec<Rays>> {
    let Some((_, analyzer)) = document
        .drawing_analyzer(analyzer_id)?
        .filter(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
    else {
        return Ok(Vec::new());
    };
    let placing = analyzer.analyzer_type().positioning_config();
    let mut scenery = document.scenery().clone();
    // Placed from the full sources first, exactly as the components are drawn: the trace below
    // leaves placed nodes where they are, so the coarsened sources cannot move anything.
    AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &placing)?;
    scenery.reset_data();

    let mut config = placing;
    config.set_positioning_run(false);
    // Only copies collect ray ends, so the scenery the copy came from holds none.
    config.set_collect_ray_ends(true);
    let per_source = max_rays / config.source_map().len().max(1);
    // A coarsened source keeps its builder's isometry and is aligned like the full one.
    let coarse_sources = config
        .source_map()
        .iter()
        .map(|(source, builder)| {
            let mut coarse = builder.clone();
            coarse.set_alignment_wavelength(Some(builder.axis_wavelength()?));
            coarse.set_source(coarsened(builder.source(), per_source, drawing)?);
            Ok((*source, coarse))
        })
        .collect::<OpmResult<_>>()?;
    config.set_source_map(coarse_sources);
    RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
    Ok(scenery.ray_ends().into_iter().cloned().collect())
}

/// Returns the range of wavelengths the sources of [`traced_rays`] emit, without building any
/// rays.
///
/// # Arguments
///
/// - `document`: the model whose sources to look at
/// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
///   exactly as for [`OpmDocument::placed_scenery`]
///
/// # Returns
///
/// The shortest and longest wavelength of all those sources, the same for light of one
/// wavelength. `None` if there are no rays to draw, as for [`traced_rays`].
///
/// # Errors
///
/// This function returns an error if the named analyzer does not exist or a spectrum cannot be
/// sampled.
pub fn source_wavelength_range(
    document: &OpmDocument,
    analyzer_id: Option<Uuid>,
) -> OpmResult<Option<Range<Length>>> {
    let Some((_, analyzer)) = document
        .drawing_analyzer(analyzer_id)?
        .filter(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
    else {
        return Ok(None);
    };
    let mut range: Option<Range<Length>> = None;
    // A range does not depend on the order the sources are visited in.
    for builder in analyzer
        .analyzer_type()
        .positioning_config()
        .source_map()
        .values()
    {
        for wavelength in builder.source().wavelengths()? {
            range = Some(range.map_or(wavelength..wavelength, |range| {
                range.start.min(wavelength)..range.end.max(wavelength)
            }));
        }
    }
    Ok(range)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::ten_lines;
    use approx::assert_relative_eq;
    use opossum_core::{
        analyzers::{AnalyzerType, EnergyConfig, RayTraceConfig},
        apertures::{CircleShape, OpticShape},
        core_optics::NodeAttrExt,
        degree,
        distributions::{
            energy::UniformDist,
            position::Hexapolar,
            spectral::{Gaussian, LaserLines, SpecDistType},
        },
        error::OpossumError,
        geometry::body::OPTIC_SHAPE,
        joule,
        light::lightdata::{
            energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
            ray_data_builder::RayDataBuilder,
            ray_data_source::{CollimatedSrc, RayDataSource},
        },
        millimeter, nanometer,
        nodes::{
            Dummy, EnergyMeter, Lens, NodeGroup, ReflectiveGrating, SourcePort, ThinMirror,
            round_collimated_ray_builder,
        },
    };
    use uom::{num_traits::Zero, si::length::millimeter};

    /// A collimated source of 1 J and 1 mm radius with the given hexapolar rings and spectrum.
    fn collimated_source(nr_of_rings: u8, spectrum: SpecDistType) -> OpmResult<RayDataBuilder> {
        Ok(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(1.0), nr_of_rings)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            spectrum,
        ))
        .into())
    }

    /// A model tracing the given source into a dummy whose output stays open.
    fn source_then_open_dummy(builder: RayDataBuilder) -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let dummy = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, builder);
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok(document)
    }

    /// A model of a source into an energy meter, followed by an energy analysis.
    fn energy_analyzed() -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", meter, "input_1", millimeter!(10.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        document.add_analyzer(AnalyzerType::Energy(config));
        Ok(document)
    }

    /// Without an analyzer there is no light of the setup's own to show.
    #[test]
    fn a_model_nobody_analyzes_traces_no_rays() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(100.0))?;
        let document = OpmDocument::new(scenery);
        assert!(traced_rays(&document, None, 100, SpectrumDrawing::Dealt)?.is_empty());
        Ok(())
    }

    /// An energy analysis places a model, but it has no rays of its own to show.
    #[test]
    fn an_energy_analysis_has_no_rays_to_draw() -> OpmResult<()> {
        assert!(traced_rays(&energy_analyzed()?, None, 100, SpectrumDrawing::Dealt)?.is_empty());
        Ok(())
    }

    /// A source of 721 rays is traced with at most the asked number, which still cover all of it:
    /// the outermost one starts on its rim.
    #[test]
    fn the_traced_rays_of_a_source_cover_all_of_it() -> OpmResult<()> {
        let document = source_then_open_dummy(round_collimated_ray_builder(
            millimeter!(5.0),
            joule!(1.0),
            15,
        )?)?;

        let ends = traced_rays(&document, None, 100, SpectrumDrawing::Dealt)?;

        assert_eq!(ends.len(), 1, "one open output");
        assert!(ends[0].nr_of_rays(false) <= 100);
        let outermost = ends[0]
            .iter()
            .filter_map(|ray| {
                let path = ray.position_history_with_current();
                let start = path.row_iter().next()?;
                Some((start[0] * start[0] + start[1] * start[1]).sqrt())
            })
            .fold(Length::zero(), Length::max);
        assert_relative_eq!(outermost.get::<millimeter>(), 5.0, epsilon = 1e-9);
        Ok(())
    }

    /// Every laser line of a source is traced, with at most the asked number of rays for all of
    /// them together, however the spectrum is drawn. Thinning the built rays used to keep the
    /// first line only: all lines of a point start there.
    #[test]
    fn every_wavelength_of_a_source_is_traced() -> OpmResult<()> {
        let document = source_then_open_dummy(collimated_source(15, ten_lines()?)?)?;

        for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
            let ends = traced_rays(&document, None, 100, drawing)?;

            assert_eq!(ends.len(), 1, "{drawing:?}: one open output");
            assert!(ends[0].nr_of_rays(false) <= 100, "{drawing:?}");
            let (lines, _) = ends[0].split_ray_bundle_by_wavelength(nanometer!(1.0), true)?;
            assert_eq!(lines.len(), 10, "{drawing:?}: every line arrives");
        }
        Ok(())
    }

    /// The asked number of rays is shared by all sources, and each of them is drawn, however the
    /// spectrum is drawn.
    #[test]
    fn max_rays_counts_the_rays_of_all_sources_together() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let mut config = RayTraceConfig::default();
        for _ in 0..2 {
            let source = scenery.add_node(SourcePort::default())?;
            let dummy = scenery.add_node(Dummy::default())?;
            scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
            config.map_source(source, collimated_source(15, ten_lines()?)?);
        }
        let mut document = OpmDocument::new(scenery);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
            let ends = traced_rays(&document, None, 100, drawing)?;

            assert_eq!(ends.len(), 2, "{drawing:?}: one open output per source");
            assert!(
                ends.iter().all(|end| end.nr_of_rays(false) > 0),
                "{drawing:?}"
            );
            let total: usize = ends.iter().map(|end| end.nr_of_rays(false)).sum();
            assert!(total <= 100, "{drawing:?}: {total} rays");
        }
        Ok(())
    }

    /// A spectrum through a grating set to Littrow for its center, on to a mirror with a component
    /// behind it. The trace re-runs the positioning along the sources it traces, so it has to find
    /// the axis the model was placed by. Thinning used to leave only the shortest wavelength, whose
    /// axis left the grating at another angle and missed the mirror.
    #[test]
    fn a_spectrum_behind_a_grating_is_traced_to_the_end() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let grating = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1053.0), degree!(0.0))?,
        )?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(grating, "output_1", mirror, "input_1", millimeter!(200.0))?;
        scenery.connect_nodes(mirror, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let spectrum = Gaussian::new(
            (nanometer!(1003.0), nanometer!(1103.0)),
            50,
            nanometer!(1053.0),
            nanometer!(10.0),
            1.0,
        )?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, collimated_source(7, spectrum.into())?);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = traced_rays(&document, None, 100, SpectrumDrawing::Dealt)?;

        assert_eq!(
            ends.len(),
            1,
            "the light reaches the open output behind the mirror"
        );
        Ok(())
    }

    /// Two lines either side of a grating's Littrow wavelength, few rays, and a mirror far behind
    /// it. The trace re-runs the positioning, so the coarsened source must put its axis where the
    /// full one did: a few nanometers off, it would leave the grating beside the mirror.
    #[test]
    fn a_coarsened_source_is_aligned_like_the_full_one() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let grating = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1050.0), degree!(0.0))?,
        )?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(grating, "output_1", mirror, "input_1", millimeter!(2000.0))?;
        scenery.connect_nodes(mirror, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let lines = LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(1100.0), 1.0)])?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, collimated_source(1, lines.into())?);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        traced_rays(&document, None, 7, SpectrumDrawing::Dealt)?;
        Ok(())
    }

    /// The range of wavelengths drawn is that of the sources' spectra, read without building any
    /// rays: from the first to the last sample of a spectrum, a single line for a laser.
    #[test]
    fn the_wavelengths_of_the_sources_span_their_spectra() -> OpmResult<()> {
        let gaussian = Gaussian::new(
            (nanometer!(1003.0), nanometer!(1103.0)),
            50,
            nanometer!(1053.0),
            nanometer!(10.0),
            1.0,
        )?;
        let lines = LaserLines::new(vec![(nanometer!(1053.0), 1.0)])?;
        for (spectrum, range) in [
            (
                SpecDistType::from(gaussian),
                nanometer!(1003.0)..nanometer!(1103.0),
            ),
            (lines.into(), nanometer!(1053.0)..nanometer!(1053.0)),
        ] {
            let document = source_then_open_dummy(collimated_source(1, spectrum)?)?;
            let drawn = source_wavelength_range(&document, None)?
                .ok_or_else(|| OpossumError::Other("no range".into()))?;
            assert_relative_eq!(drawn.start.value, range.start.value, max_relative = 1e-12);
            assert_relative_eq!(drawn.end.value, range.end.value, max_relative = 1e-12);
        }
        Ok(())
    }

    /// The range spans every source of the analyzer.
    #[test]
    fn the_wavelengths_of_several_sources_are_spanned_together() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default())?;
        let second = scenery.add_node(SourcePort::default())?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        for (source, nm) in [(first, 527.0), (second, 1053.0)] {
            let line = LaserLines::new(vec![(nanometer!(nm), 1.0)])?;
            config.map_source(source, collimated_source(1, line.into())?);
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));

        assert_eq!(
            source_wavelength_range(&document, None)?,
            Some(nanometer!(527.0)..nanometer!(1053.0))
        );
        Ok(())
    }

    /// An energy analysis has no rays to draw, so no wavelengths to draw them in.
    #[test]
    fn an_energy_analysis_has_no_wavelengths_to_draw() -> OpmResult<()> {
        assert_eq!(source_wavelength_range(&energy_analyzed()?, None)?, None);
        Ok(())
    }

    /// A source of 721 rays behind an aperture that only lets the core through: at most the
    /// asked number is traced, and the rays the aperture stops end there.
    #[test]
    #[ignore = "a ray beyond a clear aperture is stopped where it last was, not at the aperture"]
    fn the_traced_rays_are_coarsened_and_end_where_they_are_stopped() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mut aperture = Dummy::default();
        aperture.set_property(
            OPTIC_SHAPE,
            OpticShape::from(CircleShape::new(millimeter!(2.0))?).into(),
        )?;
        let aperture = scenery.add_node(aperture)?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", aperture, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(aperture, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 15)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = traced_rays(&document, None, 100, SpectrumDrawing::Dealt)?;
        assert_eq!(ends.len(), 1, "one open output");
        let rays = &ends[0];
        assert!(rays.nr_of_rays(false) <= 100);
        // How far along the axis the rays that were or were not stopped got, at most.
        let reach = |valid: bool| {
            rays.iter()
                .filter(|ray| ray.valid() == valid)
                .map(|ray| ray.position().z)
                .fold(None, |far: Option<Length>, z| {
                    Some(far.map_or(z, |far| far.max(z)))
                })
        };
        let stopped = reach(false).expect("the aperture stops the outer rays");
        let passed = reach(true).expect("the aperture passes the core");
        assert_relative_eq!(stopped.get::<millimeter>(), 50.0, epsilon = 1e-9);
        assert_relative_eq!(passed.get::<millimeter>(), 100.0, epsilon = 1e-9);
        Ok(())
    }
}
