use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use opossum_optimization::{
    BoxBounds, CouplingType, DiscoveredVariable, FreeVariable, ModelOptimizationSchema, OptimizationAlgorithm, OptimizationConfig, ParameterAddress, ParameterCoupling, StandardOperand, TargetValueOperand, ThresholdDirection, ThresholdOperand,
};

/// UI-specific representation of a tunable variable row in the dense table.
#[derive(Clone, PartialEq, Debug)]
pub struct UiVariableRow {
    pub address: ParameterAddress,
    pub group_id: Option<Uuid>,
    pub group_name: String,
    pub node_name: String,
    pub property_name: String,
    /// Note: Stored in solver units:
    /// - For SurfaceCurvature: Curvature c in [1/m]
    /// - For ConnectionDistance / NodeProperty: Value in SI base units (m, rad, etc.)
    pub current_value: f64,
    pub is_enabled: bool,
    pub min_bound: Option<f64>,
    pub max_bound: Option<f64>,
    pub coupling: Option<CouplingType>,
}

impl UiVariableRow {
    /// True if this parameter represents a spherical surface curvature.
    #[must_use]
    pub fn is_curvature(&self) -> bool {
        matches!(self.address, ParameterAddress::SurfaceCurvature { .. })
    }

    /// Computes the current radius of curvature in millimeters ($R = 1000 / c$).
    /// Returns None for completely flat surfaces ($c \approx 0$).
    #[must_use]
    pub fn current_radius_mm(&self) -> Option<f64> {
        if self.is_curvature() {
            if self.current_value.abs() < 1e-9 {
                None
            } else {
                Some(1000.0 / self.current_value)
            }
        } else {
            None
        }
    }

    /// Formats the current value for display in the table.
    #[must_use]
    pub fn display_current(&self) -> (String, Option<String>) {
        if self.is_curvature() {
            if let Some(r_mm) = self.current_radius_mm() {
                (
                    format!("R = {r_mm:.1} mm"),
                    Some(format!("c = {:.3} m⁻¹", self.current_value)),
                )
            } else {
                ("Plan (R = ∞)".to_string(), Some("c = 0.0 m⁻¹".to_string()))
            }
        } else if matches!(self.address, ParameterAddress::ConnectionDistance { .. }) {
            (format!("{:.2} mm", self.current_value * 1000.0), None)
        } else {
            (format!("{:.3}", self.current_value), None)
        }
    }

    /// Converts designer bounds into solver BoxBounds in SI units.
    pub fn solver_bounds(&self) -> Result<Option<BoxBounds>, String> {
        let (Some(b_min), Some(b_max)) = (self.min_bound, self.max_bound) else {
            return Ok(None);
        };

        if self.is_curvature() {
            // Fall A: Vorzeichengleich (z. B. beide positiv: R in [40 mm, 750 mm])
            if b_min > 0.0 && b_max > 0.0 {
                let c_min = 1000.0 / b_max; // z.B. 1000 / 750 = 1.33 m^-1
                let c_max = 1000.0 / b_min; // z.B. 1000 / 40  = 25.0 m^-1
                BoxBounds::new(c_min, c_max).map(Some).map_err(|e| e.to_string())
            }
            // Fall B: Beide negativ (z. B. R in [-750 mm, -40 mm])
            else if b_min < 0.0 && b_max < 0.0 {
                let c_min = 1000.0 / b_max; // z.B. 1000 / -40  = -25.0 m^-1
                let c_max = 1000.0 / b_min; // z.B. 1000 / -750 = -1.33 m^-1
                BoxBounds::new(c_min, c_max).map(Some).map_err(|e| e.to_string())
            }
            // Fall C: Umklappen durch Plan erlaubt (z. B. min = -40 mm, max = +40 mm)
            // min_bound ist die steilste konkave Grenze, max_bound die steilste konvexe Grenze
            else {
                let c_min = if b_min < 0.0 { 1000.0 / b_min } else { -50.0 };
                let c_max = if b_max > 0.0 { 1000.0 / b_max } else { 50.0 };
                BoxBounds::new(c_min, c_max).map(Some).map_err(|e| e.to_string())
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
        // 1. Synchronisiere alle bereits hinzugefügten Variablen mit den aktuellen Werten aus dem Modell
        for row in &mut self.variable_rows {
            if let Some(discovered) = schema
                .controllable_variables
                .iter()
                .find(|v| v.address == row.address)
            {
                row.current_value = discovered.initial_value;
                row.node_name = discovered.node_name.clone();
                row.property_name = discovered.display_name.clone();
            }
        }

        self.schema = Some(schema);
        self.run_preflight_validation();
    }

    /// Updates parameter current values directly from optimization results.
    pub fn update_values(&mut self, values: &[(ParameterAddress, f64)]) {
        for (addr, val) in values {
            if let Some(row) = self.variable_rows.iter_mut().find(|r| &r.address == addr) {
                row.current_value = *val;
            }
        }
        self.run_preflight_validation();
    }

    /// Adds a parameter from the schema as an active variable with reasonable initial bounds.
    pub fn add_variable_from_schema(&mut self, var: &DiscoveredVariable) {
        if self.variable_rows.iter().any(|r| r.address == var.address) {
            return;
        }

        let is_curv = matches!(var.address, ParameterAddress::SurfaceCurvature { .. });
        let is_dist = matches!(var.address, ParameterAddress::ConnectionDistance { .. });

        let (min_b, max_b) = if is_curv {
            // Initial value is curvature c in [1/m]
            if var.initial_value.abs() > 1e-6 {
                let r_mm = 1000.0 / var.initial_value;
                if r_mm > 0.0 {
                    (Some(r_mm * 0.5), Some(r_mm * 2.0))
                } else {
                    (Some(r_mm * 2.0), Some(r_mm * 0.5))
                }
            } else {
                (Some(-100.0), Some(100.0)) // Flat surface initial span
            }
        } else if is_dist {
            // Distance is in meters, UI bounds in mm
            let d_mm = var.initial_value * 1000.0;
            (Some((d_mm * 0.7).max(1.0)), Some(d_mm * 1.5))
        } else {
            let val = var.initial_value;
            if val.abs() > 1e-6 {
                (Some(val * 0.7), Some(val * 1.5))
            } else {
                (Some(-0.1), Some(0.1))
            }
        };

        self.variable_rows.push(UiVariableRow {
            address: var.address.clone(),
            group_id: None,
            group_name: "Selected Parameters".to_string(),
            node_name: var.node_name.clone(),
            property_name: var.display_name.clone(),
            current_value: var.initial_value,
            is_enabled: true,
            min_bound: min_b,
            max_bound: max_b,
            coupling: None,
        });

        self.run_preflight_validation();
    }

    /// Removes a variable by index.
    pub fn remove_variable(&mut self, index: usize) {
        if index < self.variable_rows.len() {
            let removed_addr = self.variable_rows[index].address.clone();
            self.variable_rows.remove(index);

            // Also clear any couplings that referenced this variable as source
            for row in &mut self.variable_rows {
                if let Some(rule) = &row.coupling {
                    let has_ref = match rule {
                        CouplingType::Identical { source } | CouplingType::Opposite { source } => source == &removed_addr,
                        CouplingType::Affine { source, .. } => source == &removed_addr,
                        CouplingType::TotalLengthLock { other_segments, .. } => other_segments.contains(&removed_addr),
                    };
                    if has_ref {
                        row.coupling = None;
                    }
                }
            }
        }
        self.run_preflight_validation();
    }

    /// Checks if a valid optimization run can be started.
    pub fn can_start_optimization(&self) -> bool {
        let has_enabled_vars = self.variable_rows.iter().any(|r| r.is_enabled && r.coupling.is_none());
        let has_operands = !self.operands.is_empty();
        let has_no_fatal_errors = !self.validation_issues.iter().any(|i| i.severity == ValidationSeverity::Error);

        has_enabled_vars && has_operands && has_no_fatal_errors
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
    /// Applies default percentage bounds (e.g. ±20%) to all active variables.
    pub fn apply_default_bounds(&mut self, factor: f64) {
        for row in &mut self.variable_rows {
            if row.is_curvature() {
                if let Some(r_mm) = row.current_radius_mm() {
                    if r_mm > 0.0 {
                        row.min_bound = Some(r_mm * (1.0 - factor));
                        row.max_bound = Some(r_mm * (1.0 + factor));
                    } else {
                        row.min_bound = Some(r_mm * (1.0 + factor));
                        row.max_bound = Some(r_mm * (1.0 - factor));
                    }
                }
            } else if matches!(row.address, ParameterAddress::ConnectionDistance { .. }) {
                let d_mm = row.current_value * 1000.0;
                row.min_bound = Some((d_mm * (1.0 - factor)).max(0.1));
                row.max_bound = Some(d_mm * (1.0 + factor));
            } else {
                let val = row.current_value;
                row.min_bound = Some(val * (1.0 - factor));
                row.max_bound = Some(val * (1.0 + factor));
            }
        }
        self.run_preflight_validation();
    }
    /// Assembles the complete serializable payload expected by `POST /api/optimization/start`.
    pub fn build_optimization_config(&self) -> Result<OptimizationConfig, String> {
        let mut free_variables = Vec::new();
        let mut couplings = Vec::new();

        for row in &self.variable_rows {
            if let Some(rule) = &row.coupling {
                couplings.push(ParameterCoupling {
                    target: row.address.clone(),
                    rule: rule.clone(),
                });
            } else if row.is_enabled {
                let mut var = FreeVariable::new(row.address.clone(), row.property_name.clone())
                    .with_initial_value(row.current_value);

                if let Some(bounds) = row.solver_bounds()? {
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
                    }
                }
            }
        }

        // Check if selected variable still exists in schema
        if let Some(schema) = &self.schema {
            for row in &self.variable_rows {
                if !schema.controllable_variables.iter().any(|v| v.address == row.address) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Variable '{} ({})' no longer exists in the model.",
                            row.node_name, row.property_name
                        ),
                        target_address: Some(row.address.clone()),
                    });
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

    /// Updates the target goal of an operand (if applicable).
    pub fn update_operand_target(&mut self, index: usize, new_target: f64) {
        if let Some(op) = self.operands.get_mut(index) {
            match op {
                StandardOperand::Target(t) => t.target = new_target,
                StandardOperand::Threshold(t) => t.limit = new_target,
                StandardOperand::MinEdgeThickness(t) => t.min_thickness = new_target,
                _ => {}
            }
        }
        self.run_preflight_validation();
    }

    /// Toggles all variables within a specific group on or off.
    pub fn toggle_all_in_group(&mut self, group_name: &str, enable: bool) {
        for row in &mut self.variable_rows {
            if row.group_name == group_name && row.coupling.is_none() {
                row.is_enabled = enable;
            }
        }
        self.run_preflight_validation();
    }
    /// Returns the active algorithm identifier as a string key.
    pub fn algorithm_kind(&self) -> &'static str {
        match self.algorithm {
            OptimizationAlgorithm::Dls { .. } => "DLS",
            OptimizationAlgorithm::LBFGS { .. } => "LBFGS",
            OptimizationAlgorithm::NelderMead { .. } => "NelderMead",
            OptimizationAlgorithm::SimulatedAnnealing { .. } => "SimulatedAnnealing",
        }
    }

    pub fn max_iterations(&self) -> u64 {
        match self.algorithm {
            OptimizationAlgorithm::Dls { max_iterations, .. }
            | OptimizationAlgorithm::NelderMead { max_iterations, .. }
            | OptimizationAlgorithm::LBFGS { max_iterations, .. }
            | OptimizationAlgorithm::SimulatedAnnealing { max_iterations, .. } => max_iterations,
        }
    }

    /// Switches the algorithm type while preserving the currently set maximum iteration limit.
    pub fn set_algorithm_kind(&mut self, kind: &str) {
        let iters = self.max_iterations();
        self.algorithm = match kind {
            "DLS" => OptimizationAlgorithm::dls().with_max_iterations(iters),
            "NelderMead" => OptimizationAlgorithm::nelder_mead().with_max_iterations(iters),
            "SimulatedAnnealing" => {
                OptimizationAlgorithm::simulated_annealing().with_max_iterations(iters)
            }
            _ => OptimizationAlgorithm::lbfgs().with_max_iterations(iters),
        };
    }

    /// Updates the maximum iterations limit on the active algorithm.
    pub fn set_max_iterations(&mut self, iters: u64) {
        self.algorithm = self.algorithm.clone().with_max_iterations(iters);
    }

    /// Updates the target cost threshold for early stopping.
    pub fn set_target_cost(&mut self, cost: f64) {
        self.target_cost = cost;
    }
    /// Returns initial temperature if Simulated Annealing is active.
    pub fn sa_initial_temperature(&self) -> f64 {
        match self.algorithm {
            OptimizationAlgorithm::SimulatedAnnealing { initial_temperature, .. } => initial_temperature,
            _ => 1.0,
        }
    }

    /// Sets the initial temperature for Simulated Annealing.
    pub fn set_sa_initial_temperature(&mut self, temp: f64) {
        if let OptimizationAlgorithm::SimulatedAnnealing { initial_temperature, .. } = &mut self.algorithm {
            *initial_temperature = temp;
        }
    }

    /// Returns the cooling rate alpha if Simulated Annealing is active.
    pub fn sa_cooling_rate(&self) -> f64 {
        match self.algorithm {
            OptimizationAlgorithm::SimulatedAnnealing { cooling_rate, .. } => cooling_rate,
            _ => 0.92,
        }
    }

    /// Sets the cooling rate alpha for Simulated Annealing.
    pub fn set_sa_cooling_rate(&mut self, rate: f64) {
        if let OptimizationAlgorithm::SimulatedAnnealing { cooling_rate, .. } = &mut self.algorithm {
            *cooling_rate = rate.clamp(0.5, 0.999);
        }
    }
}