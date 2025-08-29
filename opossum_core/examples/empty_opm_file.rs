use std::path::Path;

use opossum_core::{OpmDocument, error::OpmResult};

fn main() -> OpmResult<()> {
    let document = OpmDocument::default();
    document.save_to_file(Path::new("./opossum_core/playground/opm_document.opm"))?;
    Ok(())
}
