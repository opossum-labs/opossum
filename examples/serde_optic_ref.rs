use std::{cell::RefCell, rc::Rc};

use opossum::{error::OpossumError, nodes::Dummy, optical::OpticRef};

fn main() -> Result<(), OpossumError> {
    let optic_ref = OpticRef(Rc::new(RefCell::new(Dummy::default())));

    let serialized = serde_yaml::to_string(&optic_ref).unwrap();

    println!("serialized:\n{}", serialized);

    let restored_ref = serde_yaml::from_str::<OpticRef>(&serialized).unwrap();

    println!("restored:\n{:?}", restored_ref);

    Ok(())
}
