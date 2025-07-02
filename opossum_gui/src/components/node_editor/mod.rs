mod accordion;
pub mod alignment_editor;
pub mod general_editor;
pub mod node_editor_component;
pub mod property_editor;
pub use node_editor_component::NodeEditor;
pub mod inputs;

use dioxus::prelude::*;
use std::{cell::RefCell, rc::Rc};

pub struct CallbackWrapper(Rc<RefCell<dyn FnMut(Event<FormData>) + 'static>>);

impl PartialEq for CallbackWrapper {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl CallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Event<FormData>) + 'static,
    {
        Self(Rc::new(RefCell::new(f)))
    }

    pub fn call(&self, e: Event<FormData>) {
        (self.0.borrow_mut())(e);
    }
    #[must_use]
    pub fn noop() -> Self {
        Self::new(|_| {})
    }
}

impl Clone for CallbackWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
