pub mod operand_row;
pub mod validation;
pub mod variable_row;

pub use operand_row::UiOperandRow;
pub use validation::{ValidationIssue, ValidationSeverity};
pub use variable_row::{UiVariableRow, format_parameter_value};

use operand_row::{describe_operand, resolve_operand_group_id};
use opossum_core::opm_document::{
    OptimizationConfig,
    optimization::{
        CouplingType, DiscoveredVariable, FreeVariable, ModelOptimizationSchema,
        OptimizationAlgorithm, ParameterAddress, ParameterCoupling, StandardOperand,
    },
};
use uuid::Uuid;
use validation::run_validation;

/// Central reactive state model driving the Optimization UI.
#[derive(Clone, PartialEq, Debug)]
pub struct OptimizationModel {
    /// Cached schema retrieved from the backend.
    pub schema: Option<ModelOptimizationSchema>,
    /// Flattened inventory of all configured and coupled parameters.
    pub variable_rows: Vec<UiVariableRow>,
    /// Configured merit function operands.
    pub operands: Vec<UiOperandRow>,
    /// Solver algorithm and termination settings.
    pub algorithm: OptimizationAlgorithm,
    /// Target cost threshold for early stopping.
    pub target_cost: f64,
    /// Selected scope filter (`None` = entire setup, `Some(uuid)` = specific `NodeGroup`).
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
            target_cost: 1e-12,
            scope_group_id: None,
            validation_issues: Vec::new(),
        }
    }
}

impl OptimizationModel {
    /// Loads a persisted [`OptimizationConfig`] into the reactive UI model.
    pub fn apply_config(&mut self, config: &OptimizationConfig) {
        self.algorithm = config.algorithm.clone();
        self.target_cost = config.target_cost;
        self.variable_rows.clear();
        self.operands.clear();

        for var in &config.free_variables {
            let (min_b, max_b) = var.bounds.map_or((None, None), |b| {
                UiVariableRow::ui_bounds_from_box_bounds(&var.address, b)
            });

            let (default_node_name, default_prop_name) = match &var.address {
                ParameterAddress::NodeProperty { property_name, .. } => {
                    ("Node".to_string(), property_name.clone())
                }
                ParameterAddress::SurfaceCurvature { property_name, .. } => (
                    "Surface".to_string(),
                    format!("{property_name} (curvature)"),
                ),
                ParameterAddress::ConnectionDistance { .. } => {
                    ("Distance".to_string(), "Distance".to_string())
                }
            };

            let mut row = UiVariableRow {
                address: var.address.clone(),
                group_id: None,
                group_name: "Main System".to_string(),
                node_name: default_node_name,
                property_name: default_prop_name,
                current_value: var.initial_value.unwrap_or(0.0),
                is_enabled: true,
                min_bound: min_b,
                max_bound: max_b,
                coupling: None,
            };

            if let Some(schema) = &self.schema
                && let Some(discovered) = schema
                    .controllable_variables
                    .iter()
                    .find(|v| v.address == var.address)
            {
                row.group_id = discovered.group_id;
                row.group_name.clone_from(&discovered.group_name);
                row.node_name.clone_from(&discovered.node_name);
                row.property_name.clone_from(&discovered.display_name);
                row.current_value = discovered.initial_value;
            }

            self.variable_rows.push(row);
        }

        for coupling in &config.couplings {
            let mut row = UiVariableRow {
                address: coupling.target.clone(),
                group_id: None,
                group_name: "Main System".to_string(),
                node_name: String::new(),
                property_name: String::new(),
                current_value: 0.0,
                is_enabled: false,
                min_bound: None,
                max_bound: None,
                coupling: Some(coupling.rule.clone()),
            };

            if let Some(schema) = &self.schema
                && let Some(discovered) = schema
                    .controllable_variables
                    .iter()
                    .find(|v| v.address == coupling.target)
            {
                row.group_id = discovered.group_id;
                row.group_name.clone_from(&discovered.group_name);
                row.node_name.clone_from(&discovered.node_name);
                row.property_name.clone_from(&discovered.display_name);
                row.current_value = discovered.initial_value;
            }

            self.variable_rows.push(row);
        }

        for operand in &config.operands {
            let group_id = resolve_operand_group_id(self.schema.as_ref(), operand);
            self.operands.push(UiOperandRow {
                id: Uuid::new_v4(),
                operand: operand.clone(),
                group_id,
                is_enabled: true,
            });
        }

        self.run_preflight_validation();
    }

    /// Initializes or refreshes the model using an extracted backend schema.
    pub fn populate_from_schema(&mut self, schema: ModelOptimizationSchema) {
        let saved_config = schema.saved_config.clone();
        self.schema = Some(schema);

        if let Some(config) = saved_config {
            self.apply_config(&config);
        } else {
            self.sync_rows_with_schema();
            self.run_preflight_validation();
        }
    }

    /// Refreshes metadata and current values of existing rows from the active schema.
    fn sync_rows_with_schema(&mut self) {
        let Some(schema) = &self.schema else {
            return;
        };
        for row in &mut self.variable_rows {
            if let Some(discovered) = schema
                .controllable_variables
                .iter()
                .find(|v| v.address == row.address)
            {
                row.group_id = discovered.group_id;
                row.group_name.clone_from(&discovered.group_name);
                row.current_value = discovered.initial_value;
                row.node_name.clone_from(&discovered.node_name);
                row.property_name.clone_from(&discovered.display_name);
            }
        }
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

    /// Adds a parameter from the schema as an active variable with sensible initial bounds.
    pub fn add_variable_from_schema(&mut self, var: &DiscoveredVariable) {
        if self.variable_rows.iter().any(|r| r.address == var.address) {
            return;
        }

        let is_curv = matches!(var.address, ParameterAddress::SurfaceCurvature { .. });
        let is_dist = matches!(var.address, ParameterAddress::ConnectionDistance { .. });

        let (min_b, max_b) = if is_curv {
            if var.initial_value.abs() > 1e-6 {
                let r_mm = 1000.0 / var.initial_value;
                if r_mm > 0.0 {
                    (Some(r_mm * 0.5), Some(r_mm * 2.0))
                } else {
                    (Some(r_mm * 2.0), Some(r_mm * 0.5))
                }
            } else {
                (Some(-100.0), Some(100.0))
            }
        } else if is_dist {
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
            group_id: var.group_id,
            group_name: var.group_name.clone(),
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

    /// Removes a variable by parameter address and clears any couplings referencing it.
    pub fn remove_variable(&mut self, address: &ParameterAddress) {
        if let Some(pos) = self
            .variable_rows
            .iter()
            .position(|r| &r.address == address)
        {
            self.variable_rows.remove(pos);

            for row in &mut self.variable_rows {
                if let Some(rule) = &row.coupling {
                    let has_ref = match rule {
                        CouplingType::Identical { source } | CouplingType::Opposite { source } => {
                            source == address
                        }
                        CouplingType::Affine { source, .. } => source == address,
                        CouplingType::TotalLengthLock { other_segments, .. } => {
                            other_segments.contains(address)
                        }
                    };
                    if has_ref {
                        row.coupling = None;
                    }
                }
            }
        }
        self.run_preflight_validation();
    }

    /// Returns `true` if a valid optimization run can be started.
    #[must_use]
    pub fn can_start_optimization(&self) -> bool {
        let has_enabled_vars = self
            .variable_rows
            .iter()
            .any(|r| r.is_enabled && r.coupling.is_none());
        let has_enabled_operands = self.operands.iter().any(|op| op.is_enabled);
        let has_no_fatal_errors = !self
            .validation_issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error);

        has_enabled_vars && has_enabled_operands && has_no_fatal_errors
    }

    /// Toggles the activation state of a variable.
    pub fn toggle_variable(&mut self, address: &ParameterAddress) {
        if let Some(row) = self
            .variable_rows
            .iter_mut()
            .find(|r| &r.address == address)
        {
            row.is_enabled = !row.is_enabled;
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
                row.is_enabled = false;
            }
        }
        self.run_preflight_validation();
    }

    /// Updates the lower bound (in designer units) for a specific parameter.
    pub fn set_min_bound(&mut self, address: &ParameterAddress, min: Option<f64>) {
        if let Some(row) = self
            .variable_rows
            .iter_mut()
            .find(|r| &r.address == address)
        {
            row.min_bound = min;
        }
        self.run_preflight_validation();
    }

    /// Updates the upper bound (in designer units) for a specific parameter.
    pub fn set_max_bound(&mut self, address: &ParameterAddress, max: Option<f64>) {
        if let Some(row) = self
            .variable_rows
            .iter_mut()
            .find(|r| &r.address == address)
        {
            row.max_bound = max;
        }
        self.run_preflight_validation();
    }

    /// Applies default percentage box bounds (e.g. `0.2` for ±20%) to all variables.
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

    /// Shared helper assembling an [`OptimizationConfig`] either leniently (for saving drafts)
    /// or strictly (before launching a solver run).
    fn assemble_config(&self, strict: bool) -> Result<OptimizationConfig, String> {
        let mut free_variables = Vec::new();
        let mut couplings = Vec::new();

        for row in &self.variable_rows {
            if let Some(rule) = &row.coupling {
                couplings.push(ParameterCoupling {
                    target: row.address.clone(),
                    rule: rule.clone(),
                });
            } else if row.is_enabled {
                let mut var =
                    FreeVariable::new(row.address.clone()).with_initial_value(row.current_value);
                match row.solver_bounds() {
                    Ok(Some(bounds)) => var = var.with_bounds(bounds),
                    Err(err) if strict => return Err(err),
                    Ok(None) | Err(_) => {}
                }
                free_variables.push(var);
            }
        }

        let active_operands: Vec<StandardOperand> = self
            .operands
            .iter()
            .filter(|op| op.is_enabled)
            .map(|op| op.operand.clone())
            .collect();

        if strict {
            if free_variables.is_empty() {
                return Err("At least one free variable must be enabled.".to_string());
            }
            if active_operands.is_empty() {
                return Err("At least one merit operand must be enabled.".to_string());
            }
        }

        Ok(OptimizationConfig {
            free_variables,
            couplings,
            operands: active_operands,
            algorithm: self.algorithm.clone(),
            target_cost: self.target_cost,
        })
    }

    /// Converts current UI state into an [`OptimizationConfig`] without strict start validation.
    #[must_use]
    pub fn to_optimization_config(&self) -> OptimizationConfig {
        self.assemble_config(false)
            .unwrap_or_else(|_| OptimizationConfig::default())
    }

    /// Assembles the complete serializable payload expected by `POST /api/optimization/start`.
    ///
    /// # Errors
    /// Returns an error if no free variables or operands are enabled, or if bounds are invalid.
    pub fn build_optimization_config(&self) -> Result<OptimizationConfig, String> {
        self.assemble_config(true)
    }

    /// Re-runs pre-flight validation checks and updates `validation_issues`.
    pub fn run_preflight_validation(&mut self) {
        self.validation_issues = run_validation(&self.variable_rows, self.schema.as_ref());
    }

    /// Removes an operand by unique ID.
    pub fn remove_operand(&mut self, id: Uuid) {
        if let Some(pos) = self.operands.iter().position(|op| op.id == id) {
            self.operands.remove(pos);
        }
        self.run_preflight_validation();
    }

    /// Updates the weight of an operand.
    pub fn update_operand_weight(&mut self, id: Uuid, weight: f64) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id) {
            match &mut row.operand {
                StandardOperand::Metric(m) => m.weight = weight,
                StandardOperand::MaxTotalLength(t) => t.weight = weight,
                StandardOperand::MinEdgeThickness(t) => t.weight = weight,
                StandardOperand::TransmissionPenalty(t) => t.weight = weight,
            }
        }
    }

    /// Adds a new operand to the merit function, automatically resolving group scope.
    pub fn add_operand(&mut self, operand: StandardOperand) {
        let gid = resolve_operand_group_id(self.schema.as_ref(), &operand);
        let is_enabled = self
            .scope_group_id
            .is_none_or(|target_gid| gid == Some(target_gid));

        self.operands.push(UiOperandRow {
            id: Uuid::new_v4(),
            operand,
            group_id: gid,
            is_enabled,
        });
        self.run_preflight_validation();
    }

    /// Updates the target value of an operand in solver SI units (or ray count for transmission).
    pub fn update_operand_target(&mut self, id: Uuid, new_target: f64) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id) {
            match &mut row.operand {
                StandardOperand::Metric(m) => m.target = new_target,
                StandardOperand::MinEdgeThickness(t) => t.min_thickness = new_target,
                StandardOperand::MaxTotalLength(t) => t.max_length = new_target,
                StandardOperand::TransmissionPenalty(t) => {
                    t.expected_rays = new_target.max(1.0).round();
                }
            }
        }
        self.run_preflight_validation();
    }

    /// Updates the operand target value from UI input, converting `mm` to `m` for geometric constraints.
    pub fn update_operand_target_ui(&mut self, id: Uuid, ui_value: f64) {
        let is_geometric = matches!(
            self.operands
                .iter()
                .find(|op| op.id == id)
                .map(|r| &r.operand),
            Some(StandardOperand::MinEdgeThickness(_) | StandardOperand::MaxTotalLength(_))
        );
        let model_val = if is_geometric {
            ui_value * 1e-3
        } else {
            ui_value
        };
        self.update_operand_target(id, model_val);
    }

    /// Returns human-readable display components: `(badge_text, detail_text, operator_symbol)`.
    #[must_use]
    pub fn describe_operand_full(
        &self,
        operand: &StandardOperand,
    ) -> (String, String, &'static str) {
        describe_operand(self.schema.as_ref(), operand)
    }

    /// Returns the active algorithm identifier as a string key.
    #[must_use]
    pub const fn algorithm_kind(&self) -> &'static str {
        match self.algorithm {
            OptimizationAlgorithm::Dls { .. } => "DLS",
            OptimizationAlgorithm::LBFGS { .. } => "LBFGS",
            OptimizationAlgorithm::NelderMead { .. } => "NelderMead",
            OptimizationAlgorithm::SimulatedAnnealing { .. } => "SimulatedAnnealing",
        }
    }

    /// Returns the maximum iteration limit configured on the active algorithm.
    #[must_use]
    pub const fn max_iterations(&self) -> u64 {
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

    /// Returns initial temperature if Simulated Annealing is active.
    #[must_use]
    pub const fn sa_initial_temperature(&self) -> f64 {
        match self.algorithm {
            OptimizationAlgorithm::SimulatedAnnealing {
                initial_temperature,
                ..
            } => initial_temperature,
            _ => 1.0,
        }
    }

    /// Sets the initial temperature for Simulated Annealing.
    pub const fn set_sa_initial_temperature(&mut self, temp: f64) {
        if let OptimizationAlgorithm::SimulatedAnnealing {
            initial_temperature,
            ..
        } = &mut self.algorithm
        {
            *initial_temperature = temp;
        }
    }

    /// Returns the cooling rate alpha if Simulated Annealing is active.
    #[must_use]
    pub const fn sa_cooling_rate(&self) -> f64 {
        match self.algorithm {
            OptimizationAlgorithm::SimulatedAnnealing { cooling_rate, .. } => cooling_rate,
            _ => 0.92,
        }
    }

    /// Sets the cooling rate alpha for Simulated Annealing.
    pub const fn set_sa_cooling_rate(&mut self, rate: f64) {
        if let OptimizationAlgorithm::SimulatedAnnealing { cooling_rate, .. } = &mut self.algorithm
        {
            *cooling_rate = rate.clamp(0.5, 0.999);
        }
    }

    /// Resets current parameter values to the baseline values from the latest schema
    /// while preserving user-defined bounds and couplings.
    pub fn reset_to_initial_baseline(&mut self) {
        self.sync_rows_with_schema();
        self.run_preflight_validation();
    }

    /// Returns all unique groups `(id, name)` present in the schema variables and observables.
    #[must_use]
    pub fn available_groups(&self) -> Vec<(Uuid, String)> {
        let mut groups = Vec::new();
        if let Some(schema) = &self.schema {
            for var in &schema.controllable_variables {
                if let Some(gid) = var.group_id
                    && !groups.iter().any(|(id, _)| id == &gid)
                {
                    groups.push((gid, var.group_name.clone()));
                }
            }
            for obs in &schema.observable_metrics {
                if let Some(gid) = obs.group_id
                    && !groups.iter().any(|(id, _)| id == &gid)
                {
                    groups.push((gid, obs.group_name.clone()));
                }
            }
        }
        groups.sort_by(|a, b| a.1.cmp(&b.1));
        groups
    }

    /// Sets the active optimization scope and enables only relevant variables and operands.
    pub fn set_scope(&mut self, scope: Option<Uuid>) {
        self.scope_group_id = scope;
        if let Some(target_gid) = scope {
            for var in &mut self.variable_rows {
                if var.group_id == Some(target_gid) {
                    if var.coupling.is_none() {
                        var.is_enabled = true;
                    }
                } else {
                    var.is_enabled = false;
                }
            }
            for op in &mut self.operands {
                op.is_enabled = op.group_id == Some(target_gid);
            }
        } else {
            for var in &mut self.variable_rows {
                if var.coupling.is_none() {
                    var.is_enabled = true;
                }
            }
            for op in &mut self.operands {
                op.is_enabled = true;
            }
        }
        self.run_preflight_validation();
    }

    /// Toggles the activation state of an individual operand.
    pub fn toggle_operand(&mut self, id: Uuid) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id) {
            row.is_enabled = !row.is_enabled;
        }
        self.run_preflight_validation();
    }

    /// Cycles the evaluation mode for a `Metric` operand (`=` -> `<=` -> `>=` -> `=`).
    pub fn cycle_operand_mode(&mut self, id: Uuid) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id)
            && let StandardOperand::Metric(m) = &mut row.operand
        {
            m.mode = m.mode().cycle();
        }
        self.run_preflight_validation();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::opm_document::optimization::{BoxBounds, MetricOperand, OperandMode};

    #[test]
    fn test_populate_from_schema_with_saved_config() {
        let node_id = Uuid::new_v4();
        let addr = ParameterAddress::NodeProperty {
            node_id,
            property_name: "thickness".to_string(),
        };
        let var = FreeVariable::new(addr.clone())
            .with_initial_value(0.02)
            .with_bounds(BoxBounds::new(0.01, 0.05).unwrap());
        let operand = StandardOperand::Metric(MetricOperand::new(
            addr.clone(),
            0.04,
            OperandMode::UpperLimit,
            1.0,
            1e-3,
        ));
        let config = OptimizationConfig::new()
            .with_variable(var)
            .with_operand(operand)
            .with_algorithm(OptimizationAlgorithm::dls())
            .with_target_cost(1e-6);

        let mut schema = ModelOptimizationSchema {
            saved_config: Some(config),
            ..Default::default()
        };
        schema.controllable_variables.push(DiscoveredVariable {
            address: addr.clone(),
            group_id: None,
            group_name: "Group 1".to_string(),
            node_name: "Lens 1".to_string(),
            display_name: "Thickness".to_string(),
            initial_value: 0.02,
            is_read_only: false,
        });

        let mut model = OptimizationModel::default();
        model.populate_from_schema(schema);

        assert_eq!(model.variable_rows.len(), 1);
        let row = &model.variable_rows[0];
        assert_eq!(row.address, addr);
        assert_eq!(row.node_name, "Lens 1");
        assert_eq!(row.property_name, "Thickness");
        assert_eq!(row.min_bound, Some(0.01));
        assert_eq!(row.max_bound, Some(0.05));
        assert!(row.is_enabled);
        assert_eq!(model.operands.len(), 1);
        assert!(model.operands[0].is_enabled);
        assert!(matches!(model.algorithm, OptimizationAlgorithm::Dls { .. }));
        assert_eq!(model.target_cost, 1e-6);
    }
}
