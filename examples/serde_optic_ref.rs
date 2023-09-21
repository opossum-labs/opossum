use std::{cell::RefCell, rc::Rc};

use opossum::{optical::OpticRef, nodes::{Dummy, Detector}, error::OpossumError};

fn main() -> Result<(), OpossumError>{
  let optic_ref=OpticRef(Rc::new(RefCell::new(Detector::default())));

  let serialized= serde_yaml::to_string(&optic_ref).unwrap();

  println!("serialized:\n{}", serialized);

  let restored_ref = serde_yaml::from_str::<OpticRef>(&serialized).unwrap();

  println!("restored:\n{:?}", restored_ref);

  Ok(())
}