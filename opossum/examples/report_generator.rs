use std::path::Path;

use chrono::DateTime;
use opossum::{
    error::OpmResult,
    properties::Properties,
    reporter::{AnalysisReport, NodeReport, ReportGenerator},
    spectrum::create_he_ne_spec,
};

fn main() -> OpmResult<()> {
    let mut report = AnalysisReport::new("abc123".into(), DateTime::default());
    let mut props = Properties::default();
    props
        .create("name", "blah", None, "my name".into())
        .unwrap();
    props
        .create("total energy", "energy of detector", None, 1.012345.into())
        .unwrap();
    props.create("my bool", "", None, true.into()).unwrap();
    props
        .create("spectrum", "", None, create_he_ne_spec(1.0).unwrap().into())
        .unwrap();
    let detector = NodeReport::new("powermeter".into(), "my powermeter".into(), props);
    report.add_detector(detector);
    let generator = ReportGenerator::new(report);
    generator.generate_pdf(Path::new("./opossum/playground/output.pdf"))?;
    Ok(())
}
