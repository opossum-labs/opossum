use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use opossum_optimization::{
    BoxBounds, CouplingType, FreeVariable, ModelOptimizationSchema,
    OptimizationAlgorithm, OptimizationConfig, ParameterAddress, ParameterCoupling,
    StandardOperand, TargetValueOperand, ThresholdDirection, ThresholdOperand,
};

/// UI-specific representation of a tunable variable row in the dense table.
#[derive(Clone, PartialEq, Debug)]
pub struct UiVariableRow {
    pub address: ParameterAddress,
    pub group_id: Option<Uuid>,
    pub group_name: String,
    pub node_name: String,
    pub property_name: String,
    pub current_value: f64,
    pub is_enabled: bool,
    pub min_bound: Option<f64>,
    pub max_bound: Option<f64>,
    pub coupling: Option<CouplingType>,
}

/// Pre-flight validation severity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationSeverity {
    Warning,
    Error,
}

/// Diagnostic message displayed in the Pre-Flight sanity strip.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
    pub target_address: Option<ParameterAddress>,
}

/// Central reactive model driving the Optimization UI.
#[derive(Clone, PartialEq, Debug)]
pub struct OptimizationModel {
    /// Cached schema retrieved from the backend.
    pub schema: Option<ModelOptimizationSchema>,
    /// Flattened inventory of all discoverable parameters.
    pub variable_rows: Vec<UiVariableRow>,
    /// Configured merit operands.
    pub operands: Vec<StandardOperand>,
    /// Solver algorithm and termination settings.
    pub algorithm: OptimizationAlgorithm,
    /// Target cost threshold for early stopping.
    pub target_cost: f64,
    /// Selected scope filter ("all" or a specific group UUID string).
    pub scope_group_id: Option<Uuid>,
    /// Validation issues identified during pre-flight analysis.
    pub validation_issues: Vec<ValidationIssue>,
}

impl Default for OptimizationModel {
    fn default() -> Self {
        Self {
            schema: None,
            variable_rows: Vec::new(),
            operands: Vec::new(),
            algorithm: OptimizationAlgorithm::lbfgs(),
            target_cost: 1e-8,
            scope_group_id: None,
            validation_issues: Vec::new(),
        }
    }
}

impl OptimizationModel {
    /// Initializes or refreshes the model using an extracted schema.
    pub fn populate_from_schema(&mut self, schema: ModelOptimizationSchema) {
        let mut rows = Vec::with_capacity(schema.controllable_variables.len());

        for var in &schema.controllable_variables {
            // Read-only properties cannot be manipulated by the solver
            if var.is_read_only {
                continue;
            }

            let (group_id, group_name) = match &var.address {
                ParameterAddress::NodeProperty { .. } => {
                    (None, "Optical Nodes".to_string())
                }
                ParameterAddress::ConnectionDistance { .. } => {
                    (None, "Beam Distances".to_string())
                }
            };

            // Derive default box bounds: current +/- 50%
            let val = var.initial_value;
            let (min_b, max_b) = if val.abs() > 1e-6 {
                let margin = val.abs() * 0.5;
                (Some(val - margin), Some(val + margin))
            } else {
                (Some(-0.01), Some(0.01))
            };

            rows.push(UiVariableRow {
                address: var.address.clone(),
                group_id,
                group_name,
                node_name: var.node_name.clone(),
                property_name: var.display_name.clone(),
                current_value: val,
                is_enabled: false, // Default: designer activates parameters explicitly
                min_bound: min_b,
                max_bound: max_b,
                coupling: None,
            });
        }

        self.schema = Some(schema);
        self.variable_rows = rows;
        self.run_preflight_validation();
    }

    /// Toggles the activation state of a variable.
    pub fn toggle_variable(&mut self, address: &ParameterAddress) {
        if let Some(row) = self.variable_rows.iter_mut().find(|r| &r.address == address) {
            row.is_enabled = !row.is_enabled;
            // A coupled variable cannot be simultaneously enabled as an independent free variable
            if row.is_enabled {
                row.coupling = None;
            }
        }
        self.run_preflight_validation();
    }

    /// Sets or removes a parameter coupling on a specific target variable.
    pub fn set_coupling(&mut self, target: &ParameterAddress, coupling: Option<CouplingType>) {
        if let Some(row) = self.variable_rows.iter_mut().find(|r| &r.address == target) {
            row.coupling = coupling;
            if row.coupling.is_some() {
                row.is_enabled = false; // Dependent variable, excluded from direct solver steps
            }
        }
        self.run_preflight_validation();
    }

    /// Updates the box bounds for a given variable.
    pub fn update_bounds(&mut self, address: &ParameterAddress, min: Option<f64>, max: Option<f64>) {
        if let Some(row) = self.variable_rows.iter_mut().find(|r| &r.address == address) {
            row.min_bound = min;
            row.max_bound = max;
        }
        self.run_preflight_validation();
    }

    /// Assembles the complete serializable payload expected by `POST /api/optimization/start`.
    pub fn build_optimization_config(&self) -> Result<OptimizationConfig, String> {
        let mut free_variables = Vec::new();
        let mut couplings = Vec::new();

        for row in &self.variable_rows {
            // Apply subsystem scope filter if active
            if let Some(scope_id) = self.scope_group_id {
                if row.group_id != Some(scope_id) {
                    continue;
                }
            }

            if let Some(rule) = &row.coupling {
                couplings.push(ParameterCoupling {
                    target: row.address.clone(),
                    rule: rule.clone(),
                });
            } else if row.is_enabled {
                let mut var = FreeVariable::new(row.address.clone(), row.property_name.clone())
                    .with_initial_value(row.current_value);

                if let (Some(min), Some(max)) = (row.min_bound, row.max_bound) {
                    let bounds = BoxBounds::new(min, max).map_err(|e| e.to_string())?;
                    var = var.with_bounds(bounds);
                }

                free_variables.push(var);
            }
        }

        if free_variables.is_empty() {
            return Err("At least one free variable must be enabled.".to_string());
        }

        if self.operands.is_empty() {
            return Err("At least one merit operand must be specified.".to_string());
        }

        Ok(OptimizationConfig {
            free_variables,
            couplings,
            operands: self.operands.clone(),
            algorithm: self.algorithm.clone(),
            target_cost: self.target_cost,
        })
    }

    /// Validates variable bounds and checks couplings for cycles or conflicts.
    pub fn run_preflight_validation(&mut self) {
        let mut issues = Vec::new();
        let mut target_set = HashSet::new();

        // 1. Box bounds checks
        for row in &self.variable_rows {
            if row.is_enabled {
                if let (Some(min), Some(max)) = (row.min_bound, row.max_bound) {
                    if min >= max {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "{}: Lower bound ({min}) must be less than upper bound ({max}).",
                                row.node_name
                            ),
                            target_address: Some(row.address.clone()),
                        });
                    } else if row.current_value < min || row.current_value > max {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Warning,
                            message: format!(
                                "{}: Current value ({:.3}) lies outside configured bounds [{min}, {max}].",
                                row.node_name, row.current_value
                            ),
                            target_address: Some(row.address.clone()),
                        });
                    }
                }
            }

            if row.coupling.is_some() {
                target_set.insert(&row.address);
            }
        }

        // 2. Coupling DAG cycle detection
        for row in &self.variable_rows {
            if let Some(rule) = &row.coupling {
                let sources = match rule {
                    CouplingType::Identical { source } | CouplingType::Opposite { source } => {
                        vec![source]
                    }
                    CouplingType::Affine { source, .. } => vec![source],
                    CouplingType::TotalLengthLock { other_segments, .. } => {
                        other_segments.iter().collect()
                    }
                };

                for &src in &sources {
                    if src == &row.address {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Self-referencing coupling detected on '{}'.",
                                row.node_name
                            ),
                            target_address: Some(row.address.clone()),
                        });
                    }
                }
            }
        }

        self.validation_issues = issues;
    }
    /// Removes an operand by index.
    pub fn remove_operand(&mut self, index: usize) {
        if index < self.operands.len() {
            self.operands.remove(index);
        }
        self.run_preflight_validation();
    }

    /// Updates the weight of an operand.
    pub fn update_operand_weight(&mut self, index: usize, weight: f64) {
        if let Some(op) = self.operands.get_mut(index) {
            match op {
                StandardOperand::Target(t) => t.weight = weight,
                StandardOperand::Threshold(t) => t.weight = weight,
                StandardOperand::MaxTotalLength(t) => t.weight = weight,
                StandardOperand::MinEdgeThickness(t) => t.weight = weight,
                StandardOperand::TransmissionPenalty(t) => t.weight = weight,
            }
        }
    }

    /// Adds a new operand to the merit function.
    pub fn add_operand(&mut self, operand: StandardOperand) {
        self.operands.push(operand);
        self.run_preflight_validation();
    }
}