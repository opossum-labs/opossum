use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};
struct OpticDummy;

struct OpticIdealLens {
  focal_length: f64,
}
impl OpticIdealLens {
  pub fn new(focal_length: f64) -> Self {
    Self{focal_length}
  }
}
trait Optical {
    fn analyze(&self) {
      println!("generic analyze");
    }
}
impl Optical for OpticDummy {
  fn analyze(&self) {
      println!("optic dummy analyze");
  }
}
impl Optical for OpticIdealLens {
  fn analyze(&self) {
      println!("ideal lens analyze. f={}",self.focal_length);
  }
}
struct OpticNode<T: Optical> {
  name: String,
  node: T
}
impl <T: Optical> OpticNode<T> {
  pub fn new(name: &str, t: T) -> Self {
    Self{name: name.into(), node : t}
  }
  // pub fn node_mut(&mut self) -> &mut T {
  //   &mut self.node
  // }
  pub fn analyze(&mut self) {
    print!("Analyze element {}: ",self.name); 
    self.node.analyze();
  }
}

impl OpticNode<OpticIdealLens> {
  pub fn set_focal_length(&mut self, f: f64)  {
    self.node.focal_length=f;
  }
}
fn main() {
  let mut node=OpticNode::new("Test1", OpticDummy);
  node.analyze();

  let mut node=OpticNode::new("Test2", OpticIdealLens::new(1.23));
  node.analyze();
  node.set_focal_length(3.45);
  node.analyze();

  // let g: DiGraph<OpticNode<>,()>=DiGraph::new(); // does not work since it needs a concrete type here....
}