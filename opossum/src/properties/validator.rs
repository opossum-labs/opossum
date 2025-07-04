use crate::{error::OpmResult, properties::Proptype};
use std::fmt::Debug;

pub trait DynClone {
    fn dyn_clone(&self) -> Box<dyn Validator>;
}
impl<T> DynClone for T
where
    T: 'static + Validator + Clone,
{
    fn dyn_clone(&self) -> Box<dyn Validator> {
        Box::new(self.clone())
    }
}
// Implement Clone for Box<dyn Validator>
impl Clone for Box<dyn Validator> {
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}
/// The main Validator trait. It requires DynClone and Debug.
/// It has a single method to perform validation.
pub trait Validator: DynClone + Debug + Send + Sync {
    fn validate(&self, prop: &Proptype) -> OpmResult<()>;
}

impl<F> Validator for F
where
    F: Fn(&Proptype) -> OpmResult<()> + Clone + Debug + Send + Sync + 'static,
{
    fn validate(&self, prop: &Proptype) -> OpmResult<()> {
        // Just call the closure.
        self(prop)
    }
}
