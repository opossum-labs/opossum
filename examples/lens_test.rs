use opossum::nodes::{RealLens, Source};
use opossum::{nodes::Detector, OpticScenery};
use std::fs::File;
use std::io::Write;

use opossum::error::OpossumError;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into());
    let src = scenery.add_node(Source::default());
    let l1 = scenery.add_node(RealLens::default()); // Lens 1, 
    let l2 = scenery.add_node(RealLens::default()); // Lens 2
    let det = scenery.add_node(Detector::default());

    scenery.connect_nodes(src, "out1", l1, "in1")?;
    scenery.connect_nodes(l1, "out1", l2, "in1")?;
    scenery.connect_nodes(l2, "out1", det, "in1")?;

    let path = "lens_system.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()?).unwrap();
    Ok(())
}
