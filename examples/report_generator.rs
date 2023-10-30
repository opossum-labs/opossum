use opossum::{error::OpmResult, reporter::ReportGenerator};

fn main() -> OpmResult<()> {
    ReportGenerator::generate_pdf();
    Ok(())
}
