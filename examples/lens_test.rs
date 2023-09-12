use opossum::nodes::{RealLens, Source};
use opossum::{nodes::Detector, OpticScenery};
use std::fs::File;
use std::io::Write;

use opossum::error::OpossumError;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into());
    let src = scenery.add_element("Source", Source::default());
    let l1 = scenery.add_element("Lens 1", RealLens::default());
    let l2 = scenery.add_element("Lens 2", RealLens::default());
    let det = scenery.add_element("Detector", Detector::default());

    scenery.connect_nodes(src, "out1", l1, "in1")?;
    scenery.connect_nodes(l1, "out1", l2, "in1")?;
    scenery.connect_nodes(l2, "out1", det, "in1")?;

    let path = "lens_system.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();
    Ok(())
}
