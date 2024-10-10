use super::{PropCondition, Proptype};
use crate::error::{OpmResult, OpossumError};
use plotters::coord::combinators::LogScalable;
use serde::{Deserialize, Serialize};
use std::mem;

/// (optical) Property
///
/// A property consists of the actual value (stored as [`Proptype`]), a description and optionally a list of value conditions
/// (such as `GreaterThan`, `NonEmptyString`, etc.)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct Property {
    prop: Proptype,
    #[serde(skip)]
    description: String,
    #[serde(skip)]
    conditions: Option<Vec<PropCondition>>,
}
impl Property {
    #[must_use]
    pub const fn new(
        prop: Proptype,
        description: String,
        conditions: Option<Vec<PropCondition>>,
    ) -> Self {
        Self {
            prop,
            description,
            conditions,
        }
    }

    /// Returns a reference to the actual property value (expressed as [`Proptype`] prop of this [`Property`].
    #[must_use]
    pub const fn prop(&self) -> &Proptype {
        &self.prop
    }
    /// Returns a reference to the description of this [`Property`].
    #[must_use]
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    /// Sets the value of this [`Property`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property conditions are  not met.
    pub fn set_value(&mut self, prop: Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            if conditions.contains(&PropCondition::InternalOnly) {
                return Err(OpossumError::Properties(
                    "property is internally used and public read-only".into(),
                ));
            }
        }
        if mem::discriminant(&self.prop) != mem::discriminant(&prop) {
            return Err(OpossumError::Properties("incompatible value types".into()));
        }
        self.check_conditions(&prop)?;
        self.prop = prop;
        Ok(())
    }
    /// Sets the value unchecked of this [`Property`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`Proptype`]s [`PropCondition`]s are not met.
    pub fn set_value_unchecked(&mut self, prop: Proptype) -> OpmResult<()> {
        self.check_conditions(&prop)?;
        self.prop = prop;
        Ok(())
    }
    fn check_conditions(&self, prop: &Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            for condition in conditions {
                match condition {
                    PropCondition::NonEmptyString => {
                        if let Proptype::String(s) = prop.clone() {
                            if s.is_empty() {
                                return Err(OpossumError::Properties(
                                    "string value must not be empty".into(),
                                ));
                            }
                        }
                    }
                    PropCondition::GreaterThan(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() <= *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be > {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val <= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be > {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThan(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() >= *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val >= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::GreaterThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() < *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val < limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() > *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val > limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::InternalOnly => {}
                    PropCondition::ReadOnly => {
                        return Err(OpossumError::Properties("property is read-only".into()));
                    }
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn property_description() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
            conditions: None,
        };
        assert_eq!(prop.description(), "my description");
    }
    #[test]
    fn property_set_different_type() {
        let mut prop = Property {
            prop: Proptype::Bool(true),
            description: "".into(),
            conditions: None,
        };
        assert!(prop.set_value(Proptype::Bool(false)).is_ok());
        assert!(prop.set_value(Proptype::F64(3.14)).is_err());
    }
}
