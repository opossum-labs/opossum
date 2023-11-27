use opossum::error::OpmResult;
use opossum::nodes::Dummy;
use opossum::optic_ref::OpticRef;
use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use uuid::uuid;

fn main() -> OpmResult<()> {
    let optic_ref = OpticRef::new(
        Rc::new(RefCell::new(Dummy::default())),
        Some(uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb")),
    );
    let serialized = serde_json::to_string_pretty(&optic_ref).unwrap();
    let mut f = File::create(Path::new("./opossum/playground/opticref.opm")).unwrap();
    write!(f, "{}", serialized).unwrap();
    Ok(())
}
