use std::path::Path;

use opossum_core::{error::OpmResult, opm_document::OpmDocument};

fn main() -> OpmResult<()> {
    let document = OpmDocument::default();
    document.save_to_file(Path::new("./opossum_core/playground/opm_document.opm"))?;
    Ok(())
}
