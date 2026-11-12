use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use opossum_optimization::{
    BoxBounds, CouplingType, DiscoveredVariable, FreeVariable, MetricSource,
    ModelOptimizationSchema, OptimizationAlgorithm, OptimizationConfig, ParameterAddress,
    ParameterCoupling, StandardOperand,
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
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            // Fall B: Beide negativ (z. B. R in [-750 mm, -40 mm])
            else if b_min < 0.0 && b_max < 0.0 {
                let c_min = 1000.0 / b_max; // z.B. 1000 / -40  = -25.0 m^-1
                let c_max = 1000.0 / b_min; // z.B. 1000 / -750 = -1.33 m^-1
                BoxBounds::new(c_min, c_max)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            // Fall C: Umklappen durch Plan erlaubt (z. B. min = -40 mm, max = +40 mm)
            // min_bound ist die steilste konkave Grenze, max_bound die steilste konvexe Grenze
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

    /// Converts solver BoxBounds in SI units back into designer UI bounds.
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

/// UI-specific representation of a merit function operand.
#[derive(Clone, PartialEq, Debug)]
pub struct UiOperandRow {
    pub id: Uuid,
    pub operand: StandardOperand,
    pub group_id: Option<Uuid>,
    pub is_enabled: bool,
}

/// Pre-flight validation severity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationSeverity {
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
    pub operands: Vec<UiOperandRow>,
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
            target_cost: 1e-12, // Fixed target cost close to zero
            scope_group_id: None,
            validation_issues: Vec::new(),
        }
    }
}

impl OptimizationModel {
    /// Loads an optimization configuration into the reactive UI model.
    pub fn apply_config(&mut self, config: &OptimizationConfig) {
        self.algorithm = config.algorithm.clone();
        self.target_cost = config.target_cost;
        self.variable_rows.clear();
        self.operands.clear();

        for var in &config.free_variables {
            let (min_b, max_b) = var.bounds.map_or((None, None), |b| {
                UiVariableRow::ui_bounds_from_box_bounds(&var.address, b)
            });

            // Fallback names derived directly from the parameter address
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

            // Enrich with human-readable node and property names from active schema if present
            if let Some(schema) = &self.schema
                && let Some(discovered) = schema
                    .controllable_variables
                    .iter()
                    .find(|v| v.address == var.address)
            {
                row.group_id = discovered.group_id;
                row.group_name = discovered.group_name.clone();
                row.node_name = discovered.node_name.clone();
                row.property_name = discovered.display_name.clone();
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
                row.group_name = discovered.group_name.clone();
                row.node_name = discovered.node_name.clone();
                row.property_name = discovered.display_name.clone();
                row.current_value = discovered.initial_value;
            }

            self.variable_rows.push(row);
        }

        for operand in &config.operands {
            let group_id = self.resolve_operand_group_id(operand);

            self.operands.push(UiOperandRow {
                id: Uuid::new_v4(),
                operand: operand.clone(),
                group_id,
                is_enabled: true,
            });
        }

        self.run_preflight_validation();
    }

    /// Initializes or refreshes the model using an extracted schema.
    /// Populates or updates the active rows using newly discovered schema information.
    pub fn populate_from_schema(&mut self, schema: ModelOptimizationSchema) {
        let saved_config = schema.saved_config.clone();
        self.schema = Some(schema);

        if let Some(config) = saved_config {
            self.apply_config(&config);
        } else {
            for row in &mut self.variable_rows {
                if let Some(discovered) = self.schema.as_ref().and_then(|s| {
                    s.controllable_variables
                        .iter()
                        .find(|v| v.address == row.address)
                }) {
                    row.group_id = discovered.group_id;
                    row.group_name = discovered.group_name.clone();
                    row.current_value = discovered.initial_value;
                    row.node_name = discovered.node_name.clone();
                    row.property_name = discovered.display_name.clone();
                }
            }
            self.run_preflight_validation();
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

    /// Adds a parameter from the schema as an active variable with reasonable initial bounds.
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

    /// Resolves the human-readable analyzer name for a given analyzer ID.
    #[must_use]
    pub fn analyzer_name(&self, analyzer_id: Uuid) -> String {
        self.schema
            .as_ref()
            .and_then(|s| s.analyzers.iter().find(|a| a.id == analyzer_id))
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Analyzer".to_string())
    }

    /// Removes a variable by parameter address.
    pub fn remove_variable(&mut self, address: &ParameterAddress) {
        if let Some(pos) = self
            .variable_rows
            .iter()
            .position(|r| &r.address == address)
        {
            self.variable_rows.remove(pos);

            // Also clear any couplings that referenced this variable as source
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

    /// Checks if a valid optimization run can be started.
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
    /// Updates the lower bound (in designer units, mm) for a specific parameter.
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

    /// Updates the upper bound (in designer units, mm) for a specific parameter.
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
    /// Converts current UI state into an `OptimizationConfig` without strict start validation.
    pub fn to_optimization_config(&self) -> OptimizationConfig {
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

                if let Ok(Some(bounds)) = row.solver_bounds() {
                    var = var.with_bounds(bounds);
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

        OptimizationConfig {
            free_variables,
            couplings,
            operands: active_operands,
            algorithm: self.algorithm.clone(),
            target_cost: self.target_cost,
        }
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
                let mut var =
                    FreeVariable::new(row.address.clone()).with_initial_value(row.current_value);

                if let Some(bounds) = row.solver_bounds()? {
                    var = var.with_bounds(bounds);
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

        if free_variables.is_empty() {
            return Err("At least one free variable must be enabled.".to_string());
        }

        if active_operands.is_empty() {
            return Err("At least one merit operand must be enabled.".to_string());
        }

        Ok(OptimizationConfig {
            free_variables,
            couplings,
            operands: active_operands,
            algorithm: self.algorithm.clone(),
            target_cost: self.target_cost,
        })
    }

    /// Validates variable bounds and checks couplings for cycles or conflicts.
    pub fn run_preflight_validation(&mut self) {
        let mut issues = Vec::new();

        for row in &self.variable_rows {
            if row.is_enabled
                && let (Some(min), Some(max)) = (row.min_bound, row.max_bound)
                && min >= max
            {
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

        // Check if selected variable still exists in schema
        if let Some(schema) = &self.schema {
            for row in &self.variable_rows {
                if !schema
                    .controllable_variables
                    .iter()
                    .any(|v| v.address == row.address)
                {
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

        // Coupling integrity and cycle detection
        let mut coupling_deps: HashMap<&ParameterAddress, Vec<&ParameterAddress>> = HashMap::new();

        for row in &self.variable_rows {
            if let Some(rule) = &row.coupling {
                let deps: Vec<&ParameterAddress> = match rule {
                    CouplingType::Identical { source } | CouplingType::Opposite { source } => {
                        vec![source]
                    }
                    CouplingType::Affine { source, .. } => vec![source],
                    CouplingType::TotalLengthLock { other_segments, .. } => {
                        other_segments.iter().collect()
                    }
                };

                // Check self-reference
                if deps.contains(&&row.address) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Coupling on '{} ({})' cannot reference itself.",
                            row.node_name, row.property_name
                        ),
                        target_address: Some(row.address.clone()),
                    });
                }

                // Check unresolvable dependencies
                for &dep in &deps {
                    let dep_row = self.variable_rows.iter().find(|r| &r.address == dep);
                    let is_resolvable = match dep_row {
                        Some(r) => (r.is_enabled && r.coupling.is_none()) || r.coupling.is_some(),
                        None => false,
                    };
                    if !is_resolvable {
                        let dep_name = dep_row
                            .map(|r| format!("{} ({})", r.node_name, r.property_name))
                            .unwrap_or_else(|| "Unknown variable".to_string());
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Coupling on '{} ({})' depends on '{dep_name}', which is neither an active variable nor a coupled parameter.",
                                row.node_name, row.property_name
                            ),
                            target_address: Some(row.address.clone()),
                        });
                    }
                }

                coupling_deps.insert(&row.address, deps);
            }
        }

        // DFS Cycle Detection (White = 0, Gray = 1, Black = 2)
        let mut visit_state: HashMap<&ParameterAddress, u8> = HashMap::new();
        let mut reported_cycles: Vec<HashSet<&ParameterAddress>> = Vec::new();

        fn dfs_find_cycle<'a>(
            node: &'a ParameterAddress,
            graph: &HashMap<&'a ParameterAddress, Vec<&'a ParameterAddress>>,
            state: &mut HashMap<&'a ParameterAddress, u8>,
            path: &mut Vec<&'a ParameterAddress>,
        ) -> Option<Vec<&'a ParameterAddress>> {
            state.insert(node, 1);
            path.push(node);

            if let Some(neighbors) = graph.get(node) {
                for &next in neighbors {
                    match state.get(next).copied().unwrap_or(0) {
                        1 => {
                            if let Some(pos) = path.iter().position(|&p| p == next) {
                                let mut cycle = path[pos..].to_vec();
                                cycle.push(next);
                                return Some(cycle);
                            }
                            return Some(vec![node, next]);
                        }
                        0 => {
                            if let Some(cycle) = dfs_find_cycle(next, graph, state, path) {
                                return Some(cycle);
                            }
                        }
                        _ => {}
                    }
                }
            }

            path.pop();
            state.insert(node, 2);
            None
        }

        for &target in coupling_deps.keys() {
            if visit_state.get(target).copied().unwrap_or(0) == 0 {
                let mut path = Vec::new();
                if let Some(cycle) =
                    dfs_find_cycle(target, &coupling_deps, &mut visit_state, &mut path)
                {
                    let cycle_set: HashSet<&ParameterAddress> = cycle.iter().copied().collect();
                    if !reported_cycles
                        .iter()
                        .any(|existing| existing == &cycle_set)
                    {
                        reported_cycles.push(cycle_set);

                        let cycle_display: Vec<String> = cycle
                            .iter()
                            .map(|addr| {
                                self.variable_rows
                                    .iter()
                                    .find(|r| &r.address == *addr)
                                    .map(|r| format!("{}: {}", r.node_name, r.property_name))
                                    .unwrap_or_else(|| "Unknown".to_string())
                            })
                            .collect();

                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Cyclic parameter coupling detected: {}",
                                cycle_display.join(" → ")
                            ),
                            target_address: Some((*target).clone()),
                        });
                    }
                }
            }
        }

        self.validation_issues = issues;
    }
    /// Removes an operand by unique id.
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
        let gid = self.resolve_operand_group_id(&operand);
        let is_enabled = match self.scope_group_id {
            Some(target_gid) => gid == Some(target_gid),
            None => true,
        };
        self.operands.push(UiOperandRow {
            id: Uuid::new_v4(),
            operand,
            group_id: gid,
            is_enabled,
        });
        self.run_preflight_validation();
    }

    /// Updates the target goal of an operand (if applicable).
    pub fn update_operand_target(&mut self, id: Uuid, new_target: f64) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id) {
            match &mut row.operand {
                StandardOperand::Metric(m) => m.target = new_target,
                StandardOperand::MinEdgeThickness(t) => t.min_thickness = new_target,
                StandardOperand::MaxTotalLength(t) => t.max_length = new_target,
                _ => {}
            }
        }
        self.run_preflight_validation();
    }

    /// Returns human-readable display components: (badge_text, detail_text, operator_symbol)
    #[must_use]
    pub fn describe_operand_full(
        &self,
        operand: &StandardOperand,
    ) -> (String, String, &'static str) {
        match operand {
            StandardOperand::Metric(m) => {
                let symbol = m.mode().symbol();
                match &m.source {
                    MetricSource::Observable(obs) => {
                        let aname = self.analyzer_name(obs.analyzer_id);
                        let nname = self
                            .schema
                            .as_ref()
                            .and_then(|s| {
                                s.observable_metrics
                                    .iter()
                                    .find(|entry| entry.address.node_id == obs.node_id)
                                    .map(|entry| entry.node_name.clone())
                            })
                            .unwrap_or_else(|| "Detector".to_string());
                        (aname, format!("{nname} → {}", obs.metric_name), symbol)
                    }
                    MetricSource::Parameter(p) => ("Parameter".to_string(), p.to_string(), symbol),
                }
            }
            StandardOperand::MinEdgeThickness(t) => {
                let node_name = self
                    .schema
                    .as_ref()
                    .and_then(|s| {
                        s.edge_thickness_nodes
                            .iter()
                            .find(|n| n.node_id == t.node_id)
                            .map(|n| n.node_name.clone())
                    })
                    .unwrap_or_else(|| t.node_id.to_string());
                (
                    "Geometry".to_string(),
                    format!("{node_name} → Min Edge"),
                    "≥",
                )
            }
            StandardOperand::MaxTotalLength(_) => (
                "Geometry".to_string(),
                "Total Track Length".to_string(),
                "≤",
            ),
            StandardOperand::TransmissionPenalty(t) => {
                let aname = self.analyzer_name(t.address.analyzer_id);
                let nname = self
                    .schema
                    .as_ref()
                    .and_then(|s| {
                        s.observable_metrics
                            .iter()
                            .find(|entry| {
                                entry.address == t.address
                                    || entry.address.node_id == t.address.node_id
                            })
                            .map(|entry| entry.node_name.clone())
                    })
                    .unwrap_or_else(|| "Detector".to_string());
                (aname, format!("{nname} → Transmission"), "≥")
            }
        }
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

    /// Returns initial temperature if Simulated Annealing is active.
    pub fn sa_initial_temperature(&self) -> f64 {
        match self.algorithm {
            OptimizationAlgorithm::SimulatedAnnealing {
                initial_temperature,
                ..
            } => initial_temperature,
            _ => 1.0,
        }
    }

    /// Sets the initial temperature for Simulated Annealing.
    pub fn set_sa_initial_temperature(&mut self, temp: f64) {
        if let OptimizationAlgorithm::SimulatedAnnealing {
            initial_temperature,
            ..
        } = &mut self.algorithm
        {
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
        if let OptimizationAlgorithm::SimulatedAnnealing { cooling_rate, .. } = &mut self.algorithm
        {
            *cooling_rate = rate.clamp(0.5, 0.999);
        }
    }
    /// Updates the operand target value from UI input, automatically converting mm to m for geometric constraints.
    pub fn update_operand_target_ui(&mut self, id: Uuid, ui_value: f64) {
        let is_geometric = matches!(
            self.operands
                .iter()
                .find(|op| op.id == id)
                .map(|r| &r.operand),
            Some(StandardOperand::MinEdgeThickness(_)) | Some(StandardOperand::MaxTotalLength(_))
        );
        let model_val = if is_geometric {
            ui_value * 1e-3
        } else {
            ui_value
        };
        self.update_operand_target(id, model_val);
    }
    /// Resets current parameter values to the baseline values from the latest schema
    /// while preserving user-defined bounds and couplings.
    pub fn reset_to_initial_baseline(&mut self) {
        if let Some(schema) = &self.schema {
            for row in &mut self.variable_rows {
                if let Some(discovered) = schema
                    .controllable_variables
                    .iter()
                    .find(|v| v.address == row.address)
                {
                    row.current_value = discovered.initial_value;
                }
            }
        }
        self.run_preflight_validation();
    }
    /// Returns all unique groups (id, name) present in the schema variables and observables.
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

        match scope {
            Some(target_gid) => {
                // 1. Filter variables: enable matching group, disable all others
                for var in &mut self.variable_rows {
                    if var.group_id == Some(target_gid) {
                        if var.coupling.is_none() {
                            var.is_enabled = true;
                        }
                    } else {
                        var.is_enabled = false;
                    }
                }

                // 2. Filter operands: enable matching group, disable all others
                for op in &mut self.operands {
                    op.is_enabled = op.group_id == Some(target_gid);
                }
            }
            None => {
                // "all" scope: reactivate all independent variables and all operands
                for var in &mut self.variable_rows {
                    if var.coupling.is_none() {
                        var.is_enabled = true;
                    }
                }
                for op in &mut self.operands {
                    op.is_enabled = true;
                }
            }
        }

        self.run_preflight_validation();
    }

    /// Resolves the group ID for an operand based on its referenced node ID in the schema.
    pub fn resolve_operand_group_id(&self, operand: &StandardOperand) -> Option<Uuid> {
        let schema = self.schema.as_ref()?;
        match operand {
            StandardOperand::Metric(m) => match &m.source {
                MetricSource::Observable(obs) => schema
                    .observable_metrics
                    .iter()
                    .find(|o| o.address.node_id == obs.node_id)
                    .and_then(|o| o.group_id),
                MetricSource::Parameter(_) => None,
            },
            StandardOperand::MinEdgeThickness(t) => schema
                .controllable_variables
                .iter()
                .find(|v| match &v.address {
                    ParameterAddress::SurfaceCurvature { node_id, .. } => *node_id == t.node_id,
                    _ => false,
                })
                .and_then(|v| v.group_id),
            StandardOperand::TransmissionPenalty(t) => schema
                .observable_metrics
                .iter()
                .find(|m| m.address.node_id == t.address.node_id)
                .and_then(|m| m.group_id),
            StandardOperand::MaxTotalLength(_) => None,
        }
    }
    /// Toggles the activation state of an individual operand.
    pub fn toggle_operand(&mut self, id: Uuid) {
        if let Some(row) = self.operands.iter_mut().find(|op| op.id == id) {
            row.is_enabled = !row.is_enabled;
        }
        self.run_preflight_validation();
    }
    /// Cycles the evaluation mode for a Metric operand (= -> <= -> >= -> =).
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
    use opossum_optimization::{MetricOperand, OperandMode};

    use super::*;

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

        let mut schema = ModelOptimizationSchema::default();
        schema.saved_config = Some(config);
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
        assert!(model.variable_rows.is_empty());
        assert!(model.operands.is_empty());

        model.populate_from_schema(schema);

        // Variables restored
        assert_eq!(model.variable_rows.len(), 1);
        let row = &model.variable_rows[0];
        assert_eq!(row.address, addr);
        assert_eq!(row.node_name, "Lens 1");
        assert_eq!(row.property_name, "Thickness");
        assert_eq!(row.min_bound, Some(0.01));
        assert_eq!(row.max_bound, Some(0.05));
        assert!(row.is_enabled);

        // Operands restored
        assert_eq!(model.operands.len(), 1);
        assert_eq!(model.operands[0].operand.name(), "Max Thickness");
        assert!(model.operands[0].is_enabled);

        // Algorithm & cost restored
        assert!(matches!(model.algorithm, OptimizationAlgorithm::Dls { .. }));
        assert_eq!(model.target_cost, 1e-6);
    }

    #[test]
    fn test_coupling_cycle_detection() {
        let node_id = Uuid::new_v4();
        let addr_a = ParameterAddress::NodeProperty {
            node_id,
            property_name: "radius_1".to_string(),
        };
        let addr_b = ParameterAddress::NodeProperty {
            node_id,
            property_name: "radius_2".to_string(),
        };

        let mut model = OptimizationModel::default();
        model.variable_rows.push(UiVariableRow {
            address: addr_a.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R1".to_string(),
            current_value: 50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Opposite {
                source: addr_b.clone(),
            }),
        });
        model.variable_rows.push(UiVariableRow {
            address: addr_b.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R2".to_string(),
            current_value: -50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Opposite {
                source: addr_a.clone(),
            }),
        });

        model.run_preflight_validation();

        let has_cycle_issue = model
            .validation_issues
            .iter()
            .any(|i| i.message.contains("Cyclic parameter coupling detected"));
        assert!(has_cycle_issue);
        assert!(!model.can_start_optimization());
    }

    #[test]
    fn test_coupling_self_reference() {
        let node_id = Uuid::new_v4();
        let addr = ParameterAddress::NodeProperty {
            node_id,
            property_name: "radius_1".to_string(),
        };

        let mut model = OptimizationModel::default();
        model.variable_rows.push(UiVariableRow {
            address: addr.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R1".to_string(),
            current_value: 50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Identical {
                source: addr.clone(),
            }),
        });

        model.run_preflight_validation();

        let has_self_ref = model
            .validation_issues
            .iter()
            .any(|i| i.message.contains("cannot reference itself"));
        assert!(has_self_ref);
    }

    #[test]
    fn test_coupling_valid_cascade_no_cycle() {
        let node_id = Uuid::new_v4();
        let addr_a = ParameterAddress::NodeProperty {
            node_id,
            property_name: "r1".to_string(),
        };
        let addr_b = ParameterAddress::NodeProperty {
            node_id,
            property_name: "r2".to_string(),
        };
        let addr_c = ParameterAddress::NodeProperty {
            node_id,
            property_name: "r3".to_string(),
        };

        let mut model = OptimizationModel::default();
        // C is a free variable
        model.variable_rows.push(UiVariableRow {
            address: addr_c.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R3".to_string(),
            current_value: 50.0,
            is_enabled: true,
            min_bound: Some(10.0),
            max_bound: Some(100.0),
            coupling: None,
        });
        // B coupled to C
        model.variable_rows.push(UiVariableRow {
            address: addr_b.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R2".to_string(),
            current_value: -50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Opposite {
                source: addr_c.clone(),
            }),
        });
        // A coupled to B
        model.variable_rows.push(UiVariableRow {
            address: addr_a.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R1".to_string(),
            current_value: 50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Identical {
                source: addr_b.clone(),
            }),
        });

        model.run_preflight_validation();

        let has_cycle_issue = model
            .validation_issues
            .iter()
            .any(|i| i.message.contains("Cyclic") || i.message.contains("cannot reference itself"));
        assert!(!has_cycle_issue);
    }
}
