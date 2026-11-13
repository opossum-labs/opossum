use opossum_core::opm_document::optimization::{BoxBounds, CouplingType, ParameterAddress};
use uuid::Uuid;

/// Formats a parameter value into a primary display string and an optional secondary detail string
/// (e.g. radius in mm plus underlying curvature in 1/m).
#[must_use]
pub fn format_parameter_value(address: &ParameterAddress, value: f64) -> (String, Option<String>) {
    match address {
        ParameterAddress::SurfaceCurvature { .. } => {
            if value.abs() < 1e-9 {
                ("Plan (R = ∞)".to_string(), Some("c = 0.0 m⁻¹".to_string()))
            } else {
                let r_mm = 1000.0 / value;
                (
                    format!("R = {r_mm:.1} mm"),
                    Some(format!("c = {value:.3} m⁻¹")),
                )
            }
        }
        ParameterAddress::ConnectionDistance { .. } => (format!("{:.2} mm", value * 1000.0), None),
        ParameterAddress::NodeProperty { .. } => (format!("{value:.3}"), None),
    }
}

/// UI-specific representation of a tunable variable row in the parameters table.
#[derive(Clone, PartialEq, Debug)]
pub struct UiVariableRow {
    pub address: ParameterAddress,
    pub group_id: Option<Uuid>,
    pub group_name: String,
    pub node_name: String,
    pub property_name: String,
    /// Stored in solver SI units:
    /// - `SurfaceCurvature`: Curvature `c` in `[1/m]`
    /// - `ConnectionDistance` / `NodeProperty`: Value in SI base units (`m`, `rad`, etc.)
    pub current_value: f64,
    pub is_enabled: bool,
    pub min_bound: Option<f64>,
    pub max_bound: Option<f64>,
    pub coupling: Option<CouplingType>,
}

impl UiVariableRow {
    /// Returns `true` if this parameter represents a spherical surface curvature.
    #[must_use]
    pub const fn is_curvature(&self) -> bool {
        matches!(self.address, ParameterAddress::SurfaceCurvature { .. })
    }

    /// Computes the current radius of curvature in millimeters ($R = 1000 / c$).
    /// Returns `None` for completely flat surfaces ($c \approx 0$).
    #[must_use]
    pub fn current_radius_mm(&self) -> Option<f64> {
        if self.is_curvature() && self.current_value.abs() >= 1e-9 {
            Some(1000.0 / self.current_value)
        } else {
            None
        }
    }

    /// Formats the current value for display in the UI table.
    #[must_use]
    pub fn display_current(&self) -> (String, Option<String>) {
        format_parameter_value(&self.address, self.current_value)
    }

    /// Converts designer bounds (e.g., radii in mm) into solver [`BoxBounds`] in SI units.
    ///
    /// # Errors
    /// Returns an error string if the resulting bounds are invalid (e.g. `min >= max` or non-finite).
    pub fn solver_bounds(&self) -> Result<Option<BoxBounds>, String> {
        let (Some(b_min), Some(b_max)) = (self.min_bound, self.max_bound) else {
            return Ok(None);
        };

        if self.is_curvature() {
            // Case A: Both bounds positive (e.g. R in [40 mm, 750 mm])
            if b_min > 0.0 && b_max > 0.0 {
                let c_min = 1000.0 / b_max;
                let c_max = 1000.0 / b_min;
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            // Case B: Both bounds negative (e.g. R in [-750 mm, -40 mm])
            else if b_min < 0.0 && b_max < 0.0 {
                let c_min = 1000.0 / b_max;
                let c_max = 1000.0 / b_min;
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            // Case C: Crossing planar geometry allowed (e.g. min = -40 mm, max = +40 mm).
            // `min_bound` represents the steepest concave limit, `max_bound` the steepest convex limit.
            else {
                let c_min = if b_min < 0.0 { 1000.0 / b_min } else { -50.0 };
                let c_max = if b_max > 0.0 { 1000.0 / b_max } else { 50.0 };
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
        } else if matches!(self.address, ParameterAddress::ConnectionDistance { .. }) {
            BoxBounds::new(b_min * 1e-3, b_max * 1e-3)
                .map(Some)
                .map_err(|e| e.to_string())
        } else {
            BoxBounds::new(b_min, b_max)
                .map(Some)
                .map_err(|e| e.to_string())
        }
    }

    /// Converts solver [`BoxBounds`] in SI units back into designer UI bounds.
    #[must_use]
    pub fn ui_bounds_from_box_bounds(
        address: &ParameterAddress,
        bounds: BoxBounds,
    ) -> (Option<f64>, Option<f64>) {
        if matches!(address, ParameterAddress::SurfaceCurvature { .. }) {
            if (bounds.min > 0.0 && bounds.max > 0.0) || (bounds.min < 0.0 && bounds.max < 0.0) {
                (Some(1000.0 / bounds.max), Some(1000.0 / bounds.min))
            } else {
                let b_min = if bounds.min < 0.0 {
                    1000.0 / bounds.min
                } else {
                    -50.0
                };
                let b_max = if bounds.max > 0.0 {
                    1000.0 / bounds.max
                } else {
                    50.0
                };
                (Some(b_min), Some(b_max))
            }
        } else if matches!(address, ParameterAddress::ConnectionDistance { .. }) {
            (Some(bounds.min * 1000.0), Some(bounds.max * 1000.0))
        } else {
            (Some(bounds.min), Some(bounds.max))
        }
    }
}
