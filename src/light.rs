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

    pub fn src_port(&self) -> &str {
        self.src_port.as_ref()
    }
    

    pub fn target_port(&self) -> &str {
        self.target_port.as_ref()
    }
}
