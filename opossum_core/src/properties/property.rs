use super::Proptype;
use crate::{
    error::{OpmResult, OpossumError},
    properties::validator::Validator,
};
use serde::{Deserialize, Serialize};
use std::mem;

/// (optical) Property
///
/// A property consists of the actual value (stored as [`Proptype`]), a description and optionally a validator.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(transparent)]
pub struct Property {
    prop: Proptype,
    #[serde(skip)]
    description: String,
    #[serde(skip)]
    validator: Option<Validator>,
    #[serde(skip)]
    read_only: bool,
}
impl Property {
    /// Create a new mutable `Property`.
    ///
    /// # Errors
    /// Returns an error if the initial value fails validation.
    pub fn new(
        prop: Proptype,
        description: String,
        validator: Option<Validator>,
    ) -> OpmResult<Self> {
        Self::new_with_access(prop, description, validator, false)
    }
    /// Create a new `Property` with an explicit read-only configuration.
    ///
    /// # Errors
    /// Returns an error if the initial value fails validation.
    pub fn new_with_access(
        prop: Proptype,
        description: String,
        validator: Option<Validator>,
        read_only: bool,
    ) -> OpmResult<Self> {
        if let Some(validator) = &validator {
            validator.validate(&prop)?;
        }
        Ok(Self {
            prop,
            description,
            validator,
            read_only,
        })
    }
    /// Returns a reference to the underlying [`Proptype`].
    #[must_use]
    pub const fn prop(&self) -> &Proptype {
        &self.prop
    }
    /// Returns a reference to the property description.
    #[must_use]
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    /// Returns `true` if this property is marked as read-only.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets the value of this [`Property`].
    ///
    /// # Errors
    /// Returns an error if:
    /// - The property is read-only.
    /// - The types do not match.
    /// - The validator rejects the new value.
    pub fn set_value(&mut self, prop: Proptype) -> OpmResult<()> {
        if self.read_only {
            return Err(OpossumError::Properties(
                "Attempted to mutate a read-only property".into(),
            ));
        }
        self.set_internal_value(prop)
    }
    /// Directly sets the scalar value as `f64`.
    ///
    /// # Errors
    /// Returns an error if the property is read-only, not numeric, or fails validation.
    pub fn set_f64(&mut self, value: f64) -> OpmResult<()> {
        if self.read_only {
            return Err(OpossumError::Properties(
                "Attempted to mutate a read-only property".into(),
            ));
        }
        self.set_internal_f64(value)
    }

    /// Sets the value regardless of the `read_only` flag.
    ///
    /// Intended for internal engine components (e.g. detectors, analyzers).
    ///
    /// # Errors
    /// Returns an error if the types do not match or the validator rejects the value.
    pub fn set_internal_value(&mut self, prop: Proptype) -> OpmResult<()> {
        if mem::discriminant(&self.prop) != mem::discriminant(&prop) {
            return Err(OpossumError::Properties("Incompatible value types".into()));
        }
        if let Some(validator) = &self.validator {
            validator.validate(&prop)?;
        }
        self.prop = prop;
        Ok(())
    }

    /// Sets the scalar value directly regardless of the `read_only` flag.
    ///
    /// # Errors
    /// Returns an error if the variant is not numeric or fails validation.
    pub fn set_internal_f64(&mut self, value: f64) -> OpmResult<()> {
        let mut updated = self.prop.clone();
        updated.set_f64(value)?;
        if let Some(validator) = &self.validator {
            validator.validate(&updated)?;
        }
        self.prop = updated;
        Ok(())
    }
    /// Validate a [`Proptype`]
    ///
    /// # Errors
    ///  Returns an error if validation fails.
    pub fn validate_proptype(&self, prop: &Proptype) -> OpmResult<()> {
        self.validator
            .as_ref()
            .map_or(Ok(()), |validator| validator.validate(prop))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn prop_struct() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
            validator: None,
            read_only: false,
        };
        assert_eq!(prop.description, "my description");
    }
    #[test]
    fn new() {
        let prop = Property::new(true.into(), "my description".into(), None);
        assert!(prop.is_ok());
    }
    #[test]
    fn new_with_validator() {
        let prop = Property::new(
            1.0.into(),
            "my description".into(),
            Some(Validator::NumericIsPositive),
        );
        assert!(prop.is_ok());
        let prop = Property::new(
            (-0.1).into(),
            "my description".into(),
            Some(Validator::NumericIsPositive),
        );
        assert!(prop.is_err());
    }
    #[test]
    fn description() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
            validator: None,
            read_only: false,
        };
        assert_eq!(prop.description(), "my description");
    }
    #[test]
    fn set_different_type() {
        let mut prop = Property {
            prop: Proptype::Bool(true),
            description: "".into(),
            validator: None,
            read_only: false,
        };
        assert!(prop.set_value(Proptype::Bool(false)).is_ok());
        assert!(prop.set_value(Proptype::F64(1.23)).is_err());
    }
    #[test]
    fn read_only_property_enforcement() -> OpmResult<()> {
        let mut prop = Property::new_with_access(
            Proptype::F64(1.5),
            "Focal length".into(),
            Some(Validator::NumericIsPositive),
            true, // read_only
        )?;

        assert!(prop.is_read_only());

        // Regular mutation must be rejected
        assert!(prop.set_value(Proptype::F64(2.0)).is_err());
        assert!(prop.set_f64(2.5).is_err());

        // Internal engine update must succeed
        assert!(prop.set_internal_value(Proptype::F64(3.0)).is_ok());
        assert_eq!(prop.prop().as_f64(), Some(3.0));

        // Internal scalar update must succeed
        assert!(prop.set_internal_f64(4.0).is_ok());
        assert_eq!(prop.prop().as_f64(), Some(4.0));

        // Validation is still enforced internally
        assert!(prop.set_internal_f64(-1.0).is_err());
        Ok(())
    }
}
