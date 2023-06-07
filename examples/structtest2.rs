use petgraph::prelude::DiGraph;
trait Optical {
  fn analyze(&self) {
    println!("generic analyze");
  }
}

struct OpticIdealLens {
  focal_length: f64,
}

impl OpticIdealLens {
  pub fn new(focal_length: f64) -> Self {
    Self{focal_length}
  }
}

impl Optical for OpticIdealLens {
  fn analyze(&self) {
      println!("ideal lens analyze: f={}",self.focal_length);
  }
}
enum NodeType {
  Dummy,
  IdealLens(OpticIdealLens),
  SimpleElement(f64)
}
impl NodeType {
  fn analyze(&self) {
    match self {
        NodeType::Dummy => println!("dummy -> nothing to do here"),
        NodeType::IdealLens(n) => n.analyze(),
        _ => println!("not covered")
    }
  }
}
struct OpticNode {
  name: String,
  node: NodeType
}

impl OpticNode {
  fn new(name: &str, node_type: NodeType) -> Self {
    Self{name: name.into(), node: node_type}
  }
  fn analyze(&self) {
    print!("Analyze {}: ",self.name);
    self.node.analyze();
  }
}
fn main() {
  let node=OpticNode::new("Test1",NodeType::Dummy);
  node.analyze();
  let node=OpticNode::new("Test2",NodeType::IdealLens(OpticIdealLens::new(1.23)));
  node.analyze();
  let node=OpticNode::new("Test2",NodeType::SimpleElement(1.23));
  node.analyze();

  let _p:DiGraph<OpticNode,()> = DiGraph::new();
}