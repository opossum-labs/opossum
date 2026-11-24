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
        RayTraceConfig,
        raytrace::{RayTracingAnalyzer, TracePurpose},
    },
    error::OpmResult,
    light::Rays,
    nodes::NodeGroup,
    opm_document::OpmDocument,
};
use uom::si::f64::Length;
use uuid::Uuid;

/// Returns the sources whose light a drawing of a model shows: those of the analyzer the drawing
/// follows, in the configuration the model is positioned with.
///
/// # Arguments
///
/// - `document`: the model whose sources to look at
/// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
///   exactly as for [`OpmDocument::run_positioning`]
///
/// # Returns
///
/// The configuration holding the sources, or `None` if the model has no analyzer, or if the one
/// followed has no rays of its own, as an energy analysis: default rays would only show something
/// that is not the setup's own light.
///
/// # Errors
///
/// This function returns an error if the named analyzer does not exist.
pub fn drawn_light_sources(
    document: &OpmDocument,
    analyzer_id: Option<Uuid>,
) -> OpmResult<Option<RayTraceConfig>> {
    Ok(document
        .drawing_analyzer(analyzer_id)?
        .filter(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
        .map(|(_, analyzer)| analyzer.analyzer_type().positioning_config()))
}

/// Returns the rays of a model's sources, traced through it, for drawing them.
///
/// The model is traced as its positioning run placed it and is not positioned again (see
/// [`RayTracingAnalyzer::trace_positioned`]), so the rays run through the components where the 3D
/// view shows them. The sources share `max_rays` evenly, and each is coarsened to its share first:
/// its rays are spread over its cross section, and its spectrum is laid on them as `drawing` says.
/// That keeps the trace cheap however many rays and spectral samples the sources hold, while
/// every wavelength is still traced. Only the primary path is traced: a ghost focus analyzer
/// contributes its sources, not its reflections. The trace runs on a copy of `positioned`.
///
/// # Arguments
///
/// - `positioned`: the model, positioned by
///   [`OpmDocument::run_positioning`](opossum_core::opm_document::OpmDocument::run_positioning)
/// - `sources`: the sources to trace, from [`drawn_light_sources`]
/// - `max_rays`: the most rays traced from all sources together; each source keeps at least one
/// - `drawing`: whether each start point shines one wavelength or all of them
///
/// # Returns
///
/// One bundle per open output port the light reaches, each ray carrying its path in global
/// coordinates; rays stopped on the way are included up to where they stopped.
///
/// # Errors
///
/// This function returns an error if a source cannot be built or the trace fails.
pub fn traced_rays(
    positioned: &NodeGroup,
    sources: &RayTraceConfig,
    max_rays: usize,
    drawing: SpectrumDrawing,
) -> OpmResult<Vec<Rays>> {
    let mut config = sources.clone();
    config.set_positioning_run(false);
    // Only copies collect ray ends, so the scenery the copy came from holds none.
    config.set_collect_ray_ends(true);
    let per_source = max_rays / config.source_map().len().max(1);
    // A coarsened source keeps its builder's isometry.
    let coarse_sources = config
        .source_map()
        .iter()
        .map(|(source, builder)| {
            let mut coarse = builder.clone();
            coarse.set_source(coarsened(builder.source(), per_source, drawing)?);
            Ok((*source, coarse))
        })
        .collect::<OpmResult<_>>()?;
    config.set_source_map(coarse_sources);
    let mut scenery = positioned.clone();
    // Drawn light shows no fluence map: its sources send it out as they build it.
    RayTracingAnalyzer::new(config).trace_positioned(&mut scenery, TracePurpose::Drawing)?;
    Ok(scenery.take_ray_ends())
}

/// Returns the range of wavelengths the sources of [`traced_rays`] emit, without building any
/// rays.
///
/// # Arguments
///
/// - `sources`: the sources whose light is drawn, from [`drawn_light_sources`]; `None` if there
///   is none
///
/// # Returns
///
/// The shortest and longest wavelength of all those sources, the same for light of one
/// wavelength. `None` if there are no rays to draw.
///
/// # Errors
///
/// This function returns an error if a spectrum cannot be sampled.
pub fn source_wavelength_range(
    sources: Option<&RayTraceConfig>,
) -> OpmResult<Option<Range<Length>>> {
    let mut range: Option<Range<Length>> = None;
    // A range does not depend on the order the sources are visited in.
    for builder in sources
        .into_iter()
        .flat_map(|sources| sources.source_map().values())
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
            Dummy, EnergyMeter, FluenceDetector, Lens, NodeGroup, ReflectiveGrating, SourcePort,
            ThinMirror, round_collimated_ray_builder,
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
        assert!(drawn_light_sources(&document, None)?.is_none());
        Ok(())
    }

    /// An energy analysis places a model, but it has no rays of its own to show.
    #[test]
    fn an_energy_analysis_has_no_rays_to_draw() -> OpmResult<()> {
        assert!(drawn_light_sources(&energy_analyzed()?, None)?.is_none());
        Ok(())
    }

    /// Traces the light of the analyzer `document` is drawn after as the 3D view does: through
    /// the model as its positioning run placed it.
    fn light_of(
        document: &OpmDocument,
        max_rays: usize,
        drawing: SpectrumDrawing,
    ) -> OpmResult<Vec<Rays>> {
        let sources = drawn_light_sources(document, None)?
            .ok_or_else(|| OpossumError::Other("the model has no light to draw".into()))?;
        let run = document.run_positioning(None)?;
        traced_rays(run.positioned_scenery()?, &sources, max_rays, drawing)
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

        let ends = light_of(&document, 100, SpectrumDrawing::Dealt)?;

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

    /// Drawing the light spans no ray tubes, even for a model whose fluence detector follows them
    /// in its analysis: the 3D view draws no fluence map.
    #[test]
    fn the_drawn_light_spans_no_ray_tubes() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let detector = scenery.add_node(FluenceDetector::default())?;
        scenery.connect_nodes(source, "output_1", detector, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 2)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = light_of(&document, 100, SpectrumDrawing::Dealt)?;

        assert_eq!(ends.len(), 1, "the light leaves the open detector");
        assert!(ends[0].iter().all(|ray| ray.source().is_none()));
        Ok(())
    }

    /// Every laser line of a source is traced, with at most the asked number of rays for all of
    /// them together, however the spectrum is drawn. Thinning the built rays used to keep the
    /// first line only: all lines of a point start there.
    #[test]
    fn every_wavelength_of_a_source_is_traced() -> OpmResult<()> {
        let document = source_then_open_dummy(collimated_source(15, ten_lines()?)?)?;

        for drawing in [SpectrumDrawing::Dealt, SpectrumDrawing::Full] {
            let ends = light_of(&document, 100, drawing)?;

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
            let ends = light_of(&document, 100, drawing)?;

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
    /// behind it: the coarsened light still reaches the end. Thinning used to leave only the
    /// shortest wavelength, which left the grating at another angle and missed the mirror.
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

        let ends = light_of(&document, 100, SpectrumDrawing::Dealt)?;

        assert_eq!(
            ends.len(),
            1,
            "the light reaches the open output behind the mirror"
        );
        Ok(())
    }

    /// Two lines either side of a grating's Littrow wavelength, few rays, and a mirror far behind
    /// it: the coarsened light still reaches the end.
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

        light_of(&document, 7, SpectrumDrawing::Dealt)?;
        Ok(())
    }

    /// The light runs through the model as its positioning run placed it, without positioning it
    /// again: a distance changed on the positioned copy afterwards moves nothing the light passes.
    #[test]
    fn the_light_runs_through_the_model_as_it_was_positioned() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let dummy = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, collimated_source(1, ten_lines()?)?);
        document.add_analyzer(AnalyzerType::RayTrace(config));
        let sources = drawn_light_sources(&document, None)?
            .ok_or_else(|| OpossumError::Other("the model has no light to draw".into()))?;
        let mut positioned = document
            .run_positioning(None)?
            .positioned_scenery()?
            .clone();
        positioned.update_connection_distance(source, "output_1", millimeter!(80.0))?;

        let ends = traced_rays(&positioned, &sources, 10, SpectrumDrawing::Dealt)?;

        let end = ends
            .first()
            .and_then(|end| end.iter().next())
            .ok_or_else(|| OpossumError::Other("no light left the dummy".into()))?;
        assert_relative_eq!(end.position().z.get::<millimeter>(), 50.0, epsilon = 1e-9);
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
            let drawn = source_wavelength_range(drawn_light_sources(&document, None)?.as_ref())?
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
            source_wavelength_range(drawn_light_sources(&document, None)?.as_ref())?,
            Some(nanometer!(527.0)..nanometer!(1053.0))
        );
        Ok(())
    }

    /// An energy analysis has no rays to draw, so no wavelengths to draw them in.
    #[test]
    fn an_energy_analysis_has_no_wavelengths_to_draw() -> OpmResult<()> {
        let sources = drawn_light_sources(&energy_analyzed()?, None)?;
        assert_eq!(source_wavelength_range(sources.as_ref())?, None);
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

        let ends = light_of(&document, 100, SpectrumDrawing::Dealt)?;
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
