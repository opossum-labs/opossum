use opossum::optic_scenery::*;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("opticscenery example");
    let mut scenery = OpticScenery::default();
    scenery.set_description("OpticScenery demo".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot());
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}
