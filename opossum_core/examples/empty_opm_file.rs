use opossum_core::prelude::*;
use std::path::Path;

fn main() -> OpmResult<()> {
    let document = OpmDocument::default();
    document.save_to_file(Path::new("./opossum_core/playground/opm_document.opm"))?;
    Ok(())
}
