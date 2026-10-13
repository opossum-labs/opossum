use nalgebra::Vector2;
use opossum_core::{
    apertures::CircleShape,
    coatings::CoatingConstantR,
    core_optics::{NodeAttrExt, OpticNodeExt},
    geometry::body::CLEAR_APERTURE,
    nodes::round_collimated_ray_builder,
    percent,
    prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let two_inch: ApertureShape = CircleShape::new(millimeter!(25.4))?.into();
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;

    // Turned by 45°, the mirror meets the beam of 20 mm radius up to 28.3 mm from its center: it
    // is a 3-inch mirror.
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(45., 0.0, 0.0))?;
    mirror1.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(38.1))?).into(),
    )?;
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(50.0))?.into(),
    )?;
    let i_m1 = scenery.add_node(mirror1.clone())?;
    let mut mirror2 = ThinMirror::new("mirror 2").with_tilt(degree!(-45., 0.0, 0.0))?;
    mirror2.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(50.0))?.into(),
    )?;
    // Both parabolas are 2 inches in diameter, measured along their parent axes, parallel to the
    // collimated beam.
    let mut parabola1 = ParabolicMirror::new("parabola 1", millimeter!(100.), false)?
        .with_oap_angle(degree!(-90.))?
        .with_oap_direction(Vector2::new(0., 1.))?;
    parabola1.set_property(CLEAR_APERTURE, two_inch.clone().into())?;
    let i_m2 = scenery.add_node(parabola1)?;
    let mut parabola2 = ParabolicMirror::new("parabola 2", millimeter!(100.), true)?
        .with_oap_angle(degree!(90.))?
        .with_oap_direction(Vector2::new(0., 1.))?;
    parabola2.set_property(CLEAR_APERTURE, two_inch.into())?;
    let i_m3 = scenery.add_node(parabola2)?;
    let i_rpv = scenery.add_node(RayPropagationVisualizer::default())?;

    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_m3, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(i_m3, "output_1", i_rpv, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(20.0), joule!(1.0), 10)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/parabolic_mirror_ghost_focus.opm",
    ))
}
