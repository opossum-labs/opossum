use crate::lightdata::LightData;

#[derive(Debug, Clone)]
pub struct Light {
    src_port: String,
    target_port: String,
    data: Option<LightData>
}

impl Light {
    pub fn new(src_port: &str, target_port: &str) -> Self {
        Self {
            src_port: src_port.into(),
            target_port: target_port.into(),
            data: None
        }
    }
    pub fn src_port(&self) -> &str {
        self.src_port.as_ref()
    }
    pub fn target_port(&self) -> &str {
        self.target_port.as_ref()
    }
    pub fn set_src_port(&mut self, src_port: String) {
        self.src_port = src_port;
    }
    pub fn set_target_port(&mut self, target_port: String) {
        self.target_port = target_port;
    }
    pub fn data(&self) -> Option<&LightData> {
        self.data.as_ref()
    }
    pub fn set_data(&mut self, data: LightData) {
        self.data = Some(data);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.src_port, "test1");
        assert_eq!(light.target_port, "test2");
        assert_eq!(light.data, None);
    }
    #[test]
    fn src_port() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.src_port(), "test1");
    }
    #[test]
    fn target_port() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.target_port(), "test2");
    }
    #[test]
    fn set_src_port() {
        let mut light = Light::new("test1", "test2");
        light.set_src_port("test3".into());
        assert_eq!(light.src_port, "test3");
    }
    #[test]
    fn set_target_port() {
        let mut light = Light::new("test1", "test2");
        light.set_target_port("test3".into());
        assert_eq!(light.target_port, "test3");
    }
}
