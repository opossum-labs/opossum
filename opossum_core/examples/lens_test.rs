use opossum_core::prelude::*;
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Lens Ray-trace test");
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;

    let lens1 = Wedge::new(
        "Wedge",
        millimeter!(10.0),
        degree!(0.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(15.0, 0.0, 0.0))?;
    let l1 = scenery.add_node(lens1)?;
    let lens2 = Lens::new(
        "Lens 2",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(15.0, 0.0, 0.0))?;
    let l2 = scenery.add_node(lens2)?;
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot", None)?)?;
    scenery.connect_nodes(i_src, "output_1", l1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(l1, "output_1", l2, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(l2, "output_1", det, "input_1", millimeter!(50.0))?;
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 6)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/lens_test.opm"))
}
