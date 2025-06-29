use std::path::Path;

use opossum::{OpmDocument, error::OpmResult};

fn main() -> OpmResult<()> {
    let document = OpmDocument::default();
    document.save_to_file(Path::new("./opossum/playground/opm_document.opm"))?;
    Ok(())
}
