use super::Proptype;
use crate::{
    error::{OpmResult, OpossumError},
    plottable::Plottable,
};
use nalgebra::vector;
use serde::{Deserialize, Serialize};
use std::{mem, path::Path};

/// (optical) Property
///
/// A property consists of the actual value (stored as [`Proptype`]), a description and optionally a list of value conditions
/// (such as `GreaterThan`, `NonEmptyString`, etc.)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(transparent)]
pub struct Property {
    prop: Proptype,
    #[serde(skip)]
    description: String,
}
impl Property {
    #[must_use]
    pub const fn new(prop: Proptype, description: String) -> Self {
        Self { prop, description }
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
        self.prop = prop;
        Ok(())
    }
    /// Export this [`Property`] to a file at the given `report_path`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying implementation for the concrete
    /// [`Proptype`] returns an error.
    pub fn export_data(&self, report_path: &Path, id: &str) -> OpmResult<()> {
        match &self.prop {
            Proptype::SpotDiagram(spot_diagram) => {
                let file_path = report_path.join(Path::new(&format!("{id}.svg")));
                spot_diagram.to_plot(&file_path, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::FluenceData(fluence) => {
                let file_path = report_path.join(Path::new(&format!("{id}.png")));
                fluence.to_plot(&file_path, crate::plottable::PltBackEnd::Bitmap)?;
            }
            Proptype::Spectrometer(spectrometer) => {
                let file_path = report_path.join(Path::new(&format!("{id}.svg")));
                spectrometer.to_plot(&file_path, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::RayPositionHistory(ray_hist) => {
                let file_path = report_path.join(Path::new(&format!("{id}.svg")));
                ray_hist.to_plot(&file_path, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::GhostFocusHistory(ghost_hist) => {
                let file_path = report_path.join(Path::new(&format!("{id}.svg")));
                let mut ghost_hist = ghost_hist.clone();
                ghost_hist.plot_view_direction = Some(vector![1.0, 0.0, 0.0]);
                ghost_hist.to_plot(&file_path, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::WaveFrontData(wf_data) => {
                let file_path = report_path.join(Path::new(&format!("{id}.png")));
                wf_data.wavefront_error_maps[0]
                    .to_plot(&file_path, crate::plottable::PltBackEnd::Bitmap)?;
            }
            Proptype::HitMap(hit_map) => {
                let file_path = report_path.join(Path::new(&format!("{id}.svg")));
                hit_map.to_plot(&file_path, crate::plottable::PltBackEnd::SVG)?;
            }
            Proptype::NodeReport(report) => {
                for prop in report.properties() {
                    prop.1
                        .export_data(report_path, &format!("{id}_{}_{}", report.uuid(), prop.0))?;
                }
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
    fn new() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
        };
        assert_eq!(prop.description, "my description");
        // assert_eq!(prop.prop, Proptype::Bool(true));
    }
    #[test]
    fn description() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
        };
        assert_eq!(prop.description(), "my description");
    }
    #[test]
    fn set_different_type() {
        let mut prop = Property {
            prop: Proptype::Bool(true),
            description: "".into(),
        };
        assert!(prop.set_value(Proptype::Bool(false)).is_ok());
        assert!(prop.set_value(Proptype::F64(3.14)).is_err());
    }
}
