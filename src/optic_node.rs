#[derive(Debug, Default)]
pub struct OpticNode {
    name: String,
}

impl OpticNode {
    /// Creates a new [`OpticNode`].
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Sets the name of this [`OpticNode`].
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Returns a reference to the name of this [`OpticNode`].
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
