use std::{cell::RefCell, rc::Rc};

use serde_derive::Serialize;
use serde::Serialize;

#[derive(Default, Serialize)]
struct NodeTypeA {
  param_A: bool
}

impl Optical for NodeTypeA {
}

#[derive(Default, Serialize)]
struct NodeTypeB {
  param_B: i32
}

impl Optical for NodeTypeB {
}

trait Optical: erased_serde::Serialize {
}

#[derive(Default)]
struct Scenery {
  g: Vec<Rc<RefCell<i32>>>
}


fn main() {
  let node_a=NodeTypeA::default();
  let node_b=NodeTypeB::default();
  println!("{}",serde_yaml::to_string(&node_a).unwrap());
  let mut scene: Vec<Rc<RefCell<dyn Optical>>>=Vec::new();
  scene.push(Rc::new(RefCell::new(node_a)));
  scene.push(Rc::new(RefCell::new(node_b)));
 
}
