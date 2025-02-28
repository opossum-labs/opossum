use opossum::nodes::{Dummy, NodeGroup};

fn main() {
    let mut scenery = NodeGroup::default();
    scenery.add_node(&Dummy::new("Test")).unwrap();

    println!("before toplevel_dot");
    scenery.toplevel_dot("TB").unwrap();
    println!("after toplevel_dot");
}
