use opossum_core::{
    coatings::CoatingConstantR, core_optics::OpticNodeExt, nodes::round_collimated_ray_builder,
    percent, prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(22.5, 0.0, 0.0))?;
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(50.0))?.into(),
    )?;
    let i_m1 = scenery.add_node(mirror1)?;
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(-22.5, 0.0, 0.0))?,
    )?;
    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::default())?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    let i_wf = scenery.add_node(WaveFront::default())?;
    let i_pm = scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_prop_vis, "input_1", millimeter!(80.0))?;
    scenery.connect_nodes(i_prop_vis, "output_1", i_sd, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_sd, "output_1", i_wf, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_wf, "output_1", i_pm, "input_1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(20.0), joule!(1.0), 3)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/mirror.opm"))
}
