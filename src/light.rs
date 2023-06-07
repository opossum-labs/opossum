#[derive(Debug)]
pub struct Light {
    src_port: String,
    target_port: String,
}

impl Light {
    pub fn new(src_port: &str, target_port: &str) -> Self {
        Self {
            src_port: src_port.into(),
            target_port: target_port.into(),
        }
    }
}
