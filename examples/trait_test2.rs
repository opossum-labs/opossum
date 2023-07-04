type Scenery = Vec<Box<dyn Optical>>;

pub enum AnalyzerType {
    Energy,
    Ray,
}

trait Analyzer {
    fn analyze(&self, _scenery: &Scenery) {
        println!("No implemented");
    }
}

struct AnalyzerEnergy {}

impl Analyzer for AnalyzerEnergy {
    fn analyze(&self, scenery: &Scenery) {
        for element in scenery.iter() {
            element.analyze(AnalyzerType::Energy)
        }
    }
}

struct AnalyzerRay {}

impl Analyzer for AnalyzerRay {
  fn analyze(&self, scenery: &Scenery) {
    for element in scenery.iter() {
        element.analyze(AnalyzerType::Ray)
    }
}
}
trait Optical {
    fn analyze(&self, _anatype: AnalyzerType) {
        println!("Default");
    }
}
struct Lens {}

impl Optical for Lens {
    fn analyze(&self, anatype: AnalyzerType) {
        print!("Lens: ");
        match anatype {
            AnalyzerType::Energy => println!("Energy"),
            AnalyzerType::Ray => println!("Ray"),
        }
    }
}
struct Mirror {}

impl Optical for Mirror {
    fn analyze(&self, anatype: AnalyzerType) {
        print!("Mirror: ");
        match anatype {
            AnalyzerType::Energy => println!("Energy"),
            AnalyzerType::Ray => println!("Ray"),
        }
    }
}
fn main() {
    let lens = Lens {};
    let mirror = Mirror {};

    let comp: Scenery = vec![Box::new(lens), Box::new(mirror)];

    let a1 = AnalyzerEnergy {};
    let a2 = AnalyzerRay {};

    a1.analyze(&comp);
    a2.analyze(&comp);
}
