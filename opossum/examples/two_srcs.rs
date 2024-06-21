use opossum::{
  error::OpmResult,
  joule, millimeter,
  nodes::{collimated_line_ray_source, BeamSplitter, RayPropagationVisualizer},
  OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
  let mut scenery = OpticScenery::default();
  let i_src1 = scenery.add_node(collimated_line_ray_source(
      millimeter!(20.0),
      joule!(1.0),
      21,
  )?);
  let i_src2 = scenery.add_node(collimated_line_ray_source(
    millimeter!(20.0),
    joule!(1.0),
    21,
)?);
  let i_bs = scenery.add_node(BeamSplitter::default());
  let i_sd = scenery.add_node(RayPropagationVisualizer::default());

  scenery.connect_nodes(i_src1, "out1", i_bs, "input1", millimeter!(100.0))?;
  scenery.connect_nodes(i_src2, "out1", i_bs, "input2", millimeter!(110.0))?;
  scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_sd, "in1", millimeter!(150.0))?;

  scenery.save_to_file(Path::new("./opossum/playground/two_srcs.opm"))?;
  Ok(())
}
