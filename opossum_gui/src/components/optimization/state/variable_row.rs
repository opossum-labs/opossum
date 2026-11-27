use crate::components::node_editor::inputs::{
    format_si_with_base_unit,
    input_components::{NumericUnitMode, UnitHandling},
    round_to_decimals,
};
use opossum_core::opm_document::optimization::{
    BoxBounds, CouplingType, ParameterAddress, ParameterUnit,
};
use uuid::Uuid;

const VARIABLE_DISPLAY_DECIMALS: usize = 3;
/// Formats a parameter value in SI base units for display in the UI table.
#[must_use]
pub fn format_parameter_value(
    address: &ParameterAddress,
    value: f64,
    unit: Option<&ParameterUnit>,
) -> (String, Option<String>) {
    match address {
        ParameterAddress::SurfaceCurvature { .. } => {
            if value.abs() < 1e-9 {
                ("Plan (R = ∞)".to_string(), Some("c = 0.0 m⁻¹".to_string()))
            } else {
                let r_m = 1.0 / value;
                let unit_cfg = unit
                    .map_or_else(|| UnitHandling::new("m", true), UnitHandling::from)
                    .with_max_decimals(VARIABLE_DISPLAY_DECIMALS);
                let formatted_r = format_si_with_base_unit(r_m, &unit_cfg, false);
                (
                    format!("R = {formatted_r}"),
                    Some(format!("c = {value:.3} m⁻¹")),
                )
            }
        }
        _ => unit.map_or_else(
            || {
                let rounded = round_to_decimals(value, VARIABLE_DISPLAY_DECIMALS);
                (format!("{rounded}"), None)
            },
            |u| {
                let unit_cfg = UnitHandling::from(u).with_max_decimals(VARIABLE_DISPLAY_DECIMALS);
                let formatted = format_si_with_base_unit(value, &unit_cfg, false);
                (formatted, None)
            },
        ),
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
    /// Stored in solver SI base units (`1/m` for `SurfaceCurvature`, `m`, `rad`, `J`, etc.).
    pub current_value: f64,
    pub is_enabled: bool,
    /// Lower bound in SI base units (`m` radius for `SurfaceCurvature`, SI base unit otherwise).
    pub min_bound: Option<f64>,
    /// Upper bound in SI base units (`m` radius for `SurfaceCurvature`, SI base unit otherwise).
    pub max_bound: Option<f64>,
    pub coupling: Option<CouplingType>,
    /// Physical unit metadata resolved from the backend schema.
    pub unit: Option<ParameterUnit>,
}

impl UiVariableRow {
    /// Returns `true` if this parameter represents a spherical surface curvature.
    #[must_use]
    pub const fn is_curvature(&self) -> bool {
        matches!(self.address, ParameterAddress::SurfaceCurvature { .. })
    }

    /// Computes the current radius of curvature in meters ($R = 1 / c$).
    /// Returns `None` for completely flat surfaces ($c \approx 0$).
    #[must_use]
    pub fn current_radius_m(&self) -> Option<f64> {
        if self.is_curvature() && self.current_value.abs() >= 1e-9 {
            Some(1.0 / self.current_value)
        } else {
            None
        }
    }

    /// Returns the [`NumericUnitMode`] for editing bounds of this variable.
    #[must_use]
    pub fn bound_input_mode(&self) -> NumericUnitMode {
        if self.is_curvature() {
            NumericUnitMode::Unit(
                self.unit
                    .as_ref()
                    .map_or_else(|| UnitHandling::new("m", true), UnitHandling::from)
                    .with_max_decimals(VARIABLE_DISPLAY_DECIMALS),
            )
        } else if let Some(u) = &self.unit {
            NumericUnitMode::Unit(
                UnitHandling::from(u).with_max_decimals(VARIABLE_DISPLAY_DECIMALS),
            )
        } else {
            NumericUnitMode::Dimensionless
        }
    }

    /// Formats the current value for display in the UI table.
    #[must_use]
    pub fn display_current(&self) -> (String, Option<String>) {
        format_parameter_value(&self.address, self.current_value, self.unit.as_ref())
    }

    /// Converts UI bounds (in SI base units, i.e. radius `m` for curvature) into solver [`BoxBounds`].
    ///
    /// # Errors
    /// Returns an error string if the resulting bounds are invalid (`min >= max` or non-finite).
    pub fn solver_bounds(&self) -> Result<Option<BoxBounds>, String> {
        let (Some(b_min), Some(b_max)) = (self.min_bound, self.max_bound) else {
            return Ok(None);
        };
        if self.is_curvature() {
            // Case A & B: Both radii have the same sign (e.g. R in [0.04 m, 0.75 m])
            if (b_min > 0.0 && b_max > 0.0) || (b_min < 0.0 && b_max < 0.0) {
                let c_min = 1.0 / b_max;
                let c_max = 1.0 / b_min;
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            } else {
                // Case C: Crossing planar geometry allowed (e.g. min = -0.04 m, max = +0.04 m).
                let c_min = if b_min < 0.0 { 1.0 / b_min } else { -50.0 };
                let c_max = if b_max > 0.0 { 1.0 / b_max } else { 50.0 };
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
        } else {
            BoxBounds::new(b_min, b_max)
                .map(Some)
                .map_err(|e| e.to_string())
        }
    }

    /// Converts solver [`BoxBounds`] in SI base units back into UI bounds in SI base units.
    #[must_use]
    pub fn ui_bounds_from_box_bounds(
        address: &ParameterAddress,
        bounds: BoxBounds,
    ) -> (Option<f64>, Option<f64>) {
        if !bounds.is_finite() {
            return (None, None);
        }
        if matches!(address, ParameterAddress::SurfaceCurvature { .. }) {
            if (bounds.min > 0.0 && bounds.max > 0.0) || (bounds.min < 0.0 && bounds.max < 0.0) {
                (Some(1.0 / bounds.max), Some(1.0 / bounds.min))
            } else {
                let b_min = if bounds.min < 0.0 {
                    1.0 / bounds.min
                } else {
                    -0.02
                };
                let b_max = if bounds.max > 0.0 {
                    1.0 / bounds.max
                } else {
                    0.02
                };
                (Some(b_min), Some(b_max))
            }
        } else {
            (Some(bounds.min), Some(bounds.max))
        }
    }
}
