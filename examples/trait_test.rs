trait Analyzer {}

struct AnalyzerEnergy{}
impl Analyzer for AnalyzerEnergy {}

struct AnalyzerRay{}
impl Analyzer for AnalyzerRay {}

trait Analyzable<T: Analyzer> {
  fn analyze(&self, _analyzer: T) {
    println!("Default Analyze");
  }
}

trait Optical {}
struct Lens {}

impl Analyzable<AnalyzerEnergy> for Lens {
    fn analyze(&self, _analyzer: AnalyzerEnergy) {
        println!("Lens Analyze Energy");
      }
}

impl Analyzable<AnalyzerRay> for Lens {
  fn analyze(&self, _analyzer: AnalyzerRay) {
      println!("Lens Analyze Ray");
    }
}
impl Optical for Lens {}
struct Mirror {}

impl Analyzable<AnalyzerEnergy> for Mirror {
  fn analyze(&self, _analyzer: AnalyzerEnergy) {
    println!("Mirror Analyze Energy");
  }
}

impl Analyzable<AnalyzerRay> for Mirror {
  fn analyze(&self, _analyzer: AnalyzerRay) {
    println!("Mirror Analyze Ray");
  }
}

impl Optical for Mirror {}
fn main() {
  let lens= Lens{}; 
  let mirror= Mirror{};

  let _comp: Vec<Box<dyn Optical>> = vec![Box::new(lens), Box::new(mirror)];

  //comp[0].analyze(AnalyzerEnergy{});
  // lens.analyze(AnalyzerRay{});
  // mirror.analyze(AnalyzerEnergy{});
  // mirror.analyze(AnalyzerRay{});
}