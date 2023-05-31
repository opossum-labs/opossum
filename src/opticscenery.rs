use petgraph::prelude::DiGraph;

#[derive(Default, Debug)]
pub struct Opticscenery {
  g: DiGraph<i32,()>
}