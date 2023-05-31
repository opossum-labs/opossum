use petgraph::prelude::DiGraph;
use petgraph::dot::Dot;

#[derive(Default, Debug)]
pub struct Opticscenery {
  g: DiGraph<i32,()>
}

impl Opticscenery {
  pub fn to_dot(self: &Self) -> String {
    format!("{:?}",Dot::new(&self.g))
  }
}