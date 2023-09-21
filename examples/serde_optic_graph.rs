use std::{cell::RefCell, rc::Rc};

use opossum::{optical::OpticGraph, nodes::Dummy, error::OpossumError};

fn main() -> Result<(), OpossumError>{
  let optic_graph=OpticGraph::default();

  let serialized= serde_yaml::to_string(&optic_graph).unwrap();

  println!("serialized:\n{}", serialized);

  let restored_ref = serde_yaml::from_str::<OpticGraph>(&serialized).unwrap();

  println!("restored:\n{:?}", restored_ref);

  Ok(())
}