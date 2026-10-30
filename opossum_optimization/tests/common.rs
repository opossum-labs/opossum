use opossum_core::{
    apertures::CircleShape,
    core_optics::NodeAttrExt,
    distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
    error::OpmResult,
    geometry::body::OPTIC_SHAPE,
    joule, millimeter, nanometer,
    prelude::*,
};
use uom::si::f64::Length;
use uuid::Uuid;

/// Constructs a Keplerian optical system based on workshop_04_kepler_imaging_point.
///
/// Returns the initialized `OpmDocument` together with the analyzer UUID,
/// the lens2 UUID, and the spot diagram UUID.
pub fn build_kepler_setup(
    defocused_image_distance: Length,
) -> OpmResult<(OpmDocument, Uuid, Uuid, Uuid)> {
    let mut scenery = NodeGroup::new("Kepler optimization setup");

    // 1. Define point light source
    let i_src = scenery.add_node(SourcePort::new("point source"))?;

    // 2. Define wavelength-dependent refractive index for HZF52 glass
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.267_600_58,
        -2.053_845_66e-2,
        3.515_076_72e-2,
        7.701_513_48e-3,
        -9.081_398_17e-4,
        7.526_495_55e-5,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;

    // 3. Define first lens: 75 mm focal length, 10 mm thickness, 25 mm radius aperture
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    lens1.set_property(
        OPTIC_SHAPE,
        OpticShape::from(CircleShape::new(millimeter!(25.0))?).into(),
    )?;
    let i_pl1 = scenery.add_node(lens1)?;

    // 4. Define second lens: 50 mm focal length, 10 mm thickness, 25 mm radius aperture
    let mut lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    lens2.set_property(
        OPTIC_SHAPE,
        OpticShape::from(CircleShape::new(millimeter!(25.0))?).into(),
    )?;
    let i_pl2 = scenery.add_node(lens2)?;

    // 5. Define spot diagram detector at image position
    let i_sd_img = scenery.add_node(SpotDiagram::new("spot at image")?)?;

    // 6. Connect optical path: point source -> lens1 -> lens2 -> spot detector
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(
        i_pl2,
        "output_1",
        i_sd_img,
        "input_1",
        defocused_image_distance,
    )?;

    // 7. Configure point-source ray-tracing analysis
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(15.0), 5)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(70.0),
        )?)
        .into(),
    );

    // 8. Add analyzer to document
    let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(config));

    Ok((doc, analyzer_id, i_pl2, i_sd_img))
}

/// Assembles a Kepler telescope setup with two distinct analyzers:
/// 1. Collimated beam -> Wavefront evaluation
/// 2. Point source -> Spot diagram evaluation
pub fn build_dual_analyzer_kepler_setup() -> OpmResult<(OpmDocument, Uuid, Uuid, Uuid, Uuid, Uuid)>
{
    let mut scenery = NodeGroup::default();

    let source_id = scenery.add_node(SourcePort::default())?;
    let lens1_id = scenery.add_node(ParaxialSurface::new("lens1", millimeter!(100.0))?)?;
    let lens2_id = scenery.add_node(ParaxialSurface::new("lens2", millimeter!(50.0))?)?;
    let wf_id = scenery.add_node(WaveFront::new("wf_monitor")?)?;
    let spot_id = scenery.add_node(SpotDiagram::new("spot_detector")?)?;

    scenery.connect_nodes(
        source_id,
        "output_1",
        lens1_id,
        "input_1",
        millimeter!(70.0),
    )?;
    scenery.connect_nodes(
        lens1_id,
        "output_1",
        lens2_id,
        "input_1",
        millimeter!(150.0),
    )?;
    scenery.connect_nodes(lens2_id, "output_1", wf_id, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(wf_id, "output_1", spot_id, "input_1", millimeter!(59.0))?;

    let mut doc = OpmDocument::new(scenery);

    let mut rt_config_1 = RayTraceConfig::default();
    rt_config_1.map_source(
        source_id,
        RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(15.0), 5)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        ))
        .into(),
    );
    let analyzer_wf_id = doc.add_analyzer(AnalyzerType::RayTrace(rt_config_1));

    let mut rt_config_2 = RayTraceConfig::default();
    rt_config_2.map_source(
        source_id,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(15.0), 5)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(70.0),
        )?)
        .into(),
    );
    let analyzer_spot_id = doc.add_analyzer(AnalyzerType::RayTrace(rt_config_2));

    Ok((
        doc,
        analyzer_wf_id,
        analyzer_spot_id,
        lens1_id,
        wf_id,
        spot_id,
    ))
}
