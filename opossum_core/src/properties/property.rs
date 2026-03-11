use super::Proptype;
use crate::{
    error::{OpmResult, OpossumError},
    plottable::Plottable,
    properties::validator::Validator,
};
use nalgebra::vector;
use serde::{Deserialize, Serialize};
use std::{mem, path::Path};

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
}
impl Property {
    /// Create a new `Property`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given initial value does not pass the validation.
    pub fn new(
        prop: Proptype,
        description: String,
        validator: Option<Validator>,
    ) -> OpmResult<Self> {
        if let Some(validator) = &validator {
            validator.validate(&prop)?;
        }
        Ok(Self {
            prop,
            description,
            validator,
        })
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
        if mem::discriminant(&self.prop) != mem::discriminant(&prop) {
            return Err(OpossumError::Properties("incompatible value types".into()));
        }
        if let Some(validator) = &self.validator {
            validator.validate(&prop)?;
        }
        self.prop = prop;
        Ok(())
    }

    /// Validates the new proptype if a validator is defined
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails
    pub fn validate_proptype(&self, prop: &Proptype) -> OpmResult<()> {
        self.validator
            .as_ref()
            .map_or_else(|| Ok(()), |validator| validator.validate(prop))
    }
    /// Export this [`Property`] to a file at the given `report_path`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying implementation for the concrete
    /// [`Proptype`] returns an error.
    pub fn export_data(&self, report_path: &Path, id: &str) -> OpmResult<()> {
        match &self.prop {
            Proptype::FluenceData(fluence) => {
                fluence.to_plot(report_path, id, crate::plottable::PltBackEnd::Bitmap)?;
            }
            Proptype::Spectrum(spectrum) => {
                spectrum.to_plot(report_path, id, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::RayPositionHistory(ray_hist) => {
                ray_hist.to_plot(report_path, id, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::GhostFocusHistory(ghost_hist) => {
                let mut ghost_hist = ghost_hist.clone();
                ghost_hist.plot_view_direction = Some(vector![1.0, 0.0, 0.0]);
                ghost_hist.to_plot(report_path, id, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::WaveFrontData(wavefront_error_map) => {
                wavefront_error_map.to_plot(
                    report_path,
                    id,
                    crate::plottable::PltBackEnd::Bitmap,
                )?;
            }
            Proptype::HitMap(hit_map) => {
                hit_map.to_plot(report_path, id, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::NodeReport(report) => {
                report.export(report_path)?;
            }
            _ => {}
        }
        Ok(())
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
        };
        assert_eq!(prop.description(), "my description");
    }
    #[test]
    fn set_different_type() {
        let mut prop = Property {
            prop: Proptype::Bool(true),
            description: "".into(),
            validator: None,
        };
        assert!(prop.set_value(Proptype::Bool(false)).is_ok());
        assert!(prop.set_value(Proptype::F64(3.14)).is_err());
    }
}
