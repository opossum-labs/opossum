//! Models and readers the tests of every module share.

use nalgebra::Point2;
use opossum_core::{
    analyzers::{AnalyzerType, RayTraceConfig},
    core_optics::OpticNode,
    degree,
    error::OpmResult,
    light::lightdata::ray_data_builder::RayDataBuilder,
    material::default_reference_wavelength,
    millimeter,
    nodes::{
        CylindricLens, EnergyMeter, Lens, NodeGroup, NodeReference, SourcePort, ThinMirror, Wedge,
    },
    opm_document::OpmDocument,
    utils::geom_transformation::Isometry,
};
use uom::si::f64::Length;

use crate::scene_glb;

/// A model holding one of everything: three components that enclose a volume, two that are a
/// single surface (a mirror and a detector), a source port that is neither, and a reference,
/// which is the same lens passed through a second time rather than another component. The
/// mirror folds the beam back, so that the light can take that second pass at all.
pub fn a_bit_of_everything() -> OpmResult<OpmDocument> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let wedge = scenery.add_node(Wedge::default())?;
    let cylinder = scenery.add_node(CylindricLens::default())?;
    let lens = scenery.add_node(Lens::default())?;
    let mirror = scenery.add_node(ThinMirror::default())?;
    let mut lens_again = NodeReference::from_node(&scenery.node_recursive(lens)?.0)?;
    lens_again.set_inverted(true)?;
    let lens_again = scenery.add_node(lens_again)?;
    let detector = scenery.add_node(EnergyMeter::default())?;
    let hop = millimeter!(50.0);
    scenery.connect_nodes(source, "output_1", wedge, "input_1", hop)?;
    scenery.connect_nodes(wedge, "output_1", cylinder, "input_1", hop)?;
    scenery.connect_nodes(cylinder, "output_1", lens, "input_1", hop)?;
    scenery.connect_nodes(lens, "output_1", mirror, "input_1", hop)?;
    // Passed backwards, the lens is entered through what is its output port.
    scenery.connect_nodes(mirror, "output_1", lens_again, "output_1", hop)?;
    scenery.connect_nodes(lens_again, "input_1", detector, "input_1", hop)?;

    let mut document = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(source, RayDataBuilder::default());
    document.add_analyzer(AnalyzerType::RayTrace(config));
    Ok(document)
}

/// Read the JSON chunk out of a binary glTF file.
///
/// A GLB is a 12 byte header followed by length-prefixed chunks, the first of which describes
/// the scene. Everything a test wants to know about an exported scene is in there.
pub fn glb_json(glb: &[u8]) -> serde_json::Value {
    let length_bytes: [u8; 4] = glb[12..16].try_into().expect("a glTF header");
    let json_length = u32::from_le_bytes(length_bytes) as usize;
    serde_json::from_slice(&glb[20..20 + json_length]).expect("readable glTF JSON")
}

/// Export a model and read back what the exported scene says about itself.
pub fn exported(document: &OpmDocument) -> OpmResult<serde_json::Value> {
    let glb = scene_glb(
        &document.placed_scenery(None)?,
        default_reference_wavelength(),
    )?;
    assert!(glb.starts_with(b"glTF"));
    Ok(glb_json(&glb))
}

/// The components a scene actually draws.
///
/// `optoscene` wraps them in a root node and one group per layer, and neither of those carries
/// any geometry — counting all nodes would count the scaffolding too.
pub fn drawn_components(scene: &serde_json::Value) -> Vec<&serde_json::Value> {
    scene["nodes"]
        .as_array()
        .expect("a scene has nodes")
        .iter()
        .filter(|node| node.get("mesh").is_some())
        .collect()
}

/// Two lenses a given distance apart, so that moving one is a one-line change.
pub fn two_lenses(spacing: Length) -> OpmResult<OpmDocument> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let first = scenery.add_node(Lens::default())?;
    let second = scenery.add_node(Lens::default())?;
    scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(first, "output_1", second, "input_1", spacing)?;

    let mut document = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(source, RayDataBuilder::default());
    document.add_analyzer(AnalyzerType::RayTrace(config));
    Ok(document)
}

/// A placement away from the origin and turned about all three axes, to check that something does
/// not depend on where a node sits.
pub fn somewhere_else() -> OpmResult<Isometry> {
    Isometry::new(millimeter!(12.0, -4.0, 80.0), degree!(3.0, -7.0, 20.0))
}

/// The corners of an L, 20 mm along each arm and 5 mm wide.
///
/// Counter-clockwise, and with one corner the outline turns *into* — which is what makes it worth
/// testing against anything that walks or fills an outline.
pub fn l_shape_corners() -> Vec<Point2<Length>> {
    vec![
        millimeter!(0.0, 0.0),
        millimeter!(20.0, 0.0),
        millimeter!(20.0, 5.0),
        millimeter!(5.0, 5.0),
        millimeter!(5.0, 20.0),
        millimeter!(0.0, 20.0),
    ]
}
