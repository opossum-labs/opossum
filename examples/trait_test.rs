use std::{cell::RefCell, rc::Rc};

use serde::Serialize;
use serde_derive::Serialize;

#[derive(Default, Serialize)]
struct NodeTypeA {
    param_A: bool,
}

impl Optical for NodeTypeA {
    fn name(&self) -> &str {
        "NodeTypeA"
    }
}

#[derive(Default, Serialize)]
struct NodeTypeB {
    param_B: i32,
}

impl Optical for NodeTypeB {}

trait Optical: erased_serde::Serialize {
    fn name(&self) -> &str {
        "unknown"
    }
    fn analyze(&mut self) {
        println!("Analyze");
    }
}

#[derive(Default)]
struct Scenery {
    g: Vec<Rc<RefCell<dyn Optical>>>,
}

impl Scenery {
    fn add_optical<T: Optical + 'static>(&mut self, node: T) {
        self.g.push(Rc::new(RefCell::new(node)));
    }
    fn analyze(&mut self) {
        for node in self.g.iter_mut() {
            node.borrow_mut().analyze();
        }
    }
}

fn main() {
    let node_a = NodeTypeA::default();
    let node_b = NodeTypeB::default();
    println!("{}", serde_yaml::to_string(&node_a).unwrap());
    let mut scene = Scenery::default();
    scene.add_optical(node_a);
    scene.add_optical(node_b);
    scene.analyze();
}
