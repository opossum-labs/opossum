#[derive(Debug, PartialEq, Clone)]
pub enum OpticPortDirection {
    Incoming,
    Outgoing,
}
impl OpticPortDirection {
    fn invert(self) -> OpticPortDirection {
        if self == OpticPortDirection::Incoming {
            OpticPortDirection::Outgoing
        } else {
            OpticPortDirection::Incoming
        }
    }
}
#[derive(Debug)]
pub struct OpticPort {
    name: String,
    direction: OpticPortDirection,
}

impl OpticPort {
    pub fn new(name: &str, direction: OpticPortDirection) -> Self {
        Self{ name: name.into(), direction: direction }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.into();
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn set_direction(&mut self, direction: OpticPortDirection) {
        self.direction = direction;
    }

    pub fn direction(&self) -> &OpticPortDirection {
        &self.direction
    }

    pub fn invert(&mut self) {
        self.direction = self.direction.clone().invert();
    }
}

#[cfg(test)]
mod test {
    use crate::optic_port::{OpticPort,OpticPortDirection};
    #[test]
    fn new() {
        let port = OpticPort::new("Test", OpticPortDirection::Incoming);
        assert_eq!(port.name, "Test");
        assert_eq!(port.direction, OpticPortDirection::Incoming);
    }
    #[test]
    fn set_name() {
        let mut port = OpticPort::new("Test", OpticPortDirection::Incoming);
        port.set_name("Test2");
        assert_eq!(port.name, "Test2");
        assert_eq!(port.direction, OpticPortDirection::Incoming);
    }
    #[test]
    fn name() {
        let port = OpticPort::new("Test", OpticPortDirection::Incoming);
        assert_eq!(port.name(), "Test");
    }
    #[test]
    fn set_direction() {
        let mut port = OpticPort::new("Test", OpticPortDirection::Incoming);
        port.set_direction(OpticPortDirection::Outgoing);
        assert_eq!(port.name, "Test");
        assert_eq!(port.direction, OpticPortDirection::Outgoing);
    }
    #[test]
    fn direction() {
        let port = OpticPort::new("Test", OpticPortDirection::Incoming);
        assert_eq!(port.direction(), &OpticPortDirection::Incoming);
    }
    #[test]
    fn invert() {
        let mut port = OpticPort::new("Test", OpticPortDirection::Incoming);
        port.invert();
        assert_eq!(port.direction, OpticPortDirection::Outgoing);
        assert_eq!(port.name, "Test");
    }
}
