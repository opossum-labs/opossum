use super::variable_row::UiVariableRow;
use opossum_core::opm_document::optimization::{CouplingType, ParameterAddress};
use opossum_optimization::ModelOptimizationSchema;
use std::collections::{HashMap, HashSet};

/// Pre-flight validation severity level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationSeverity {
    Error,
}

/// Diagnostic message displayed in the Pre-Flight sanity banner.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
    pub target_address: Option<ParameterAddress>,
}

/// Runs all pre-flight checks on the current variables, bounds, schema, and couplings.
#[must_use]
pub fn run_validation(
    variable_rows: &[UiVariableRow],
    schema: Option<&ModelOptimizationSchema>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    validate_bounds_and_schema(variable_rows, schema, &mut issues);
    validate_couplings(variable_rows, &mut issues);
    issues
}

/// Checks that enabled variables have valid bounds (`min < max`) and still exist in the active schema.
fn validate_bounds_and_schema(
    variable_rows: &[UiVariableRow],
    schema: Option<&ModelOptimizationSchema>,
    issues: &mut Vec<ValidationIssue>,
) {
    for row in variable_rows {
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

    if let Some(active_schema) = schema {
        for row in variable_rows {
            if !active_schema
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
}

/// Validates parameter coupling dependencies and detects cyclic coupling graphs.
fn validate_couplings(variable_rows: &[UiVariableRow], issues: &mut Vec<ValidationIssue>) {
    let mut coupling_deps: HashMap<&ParameterAddress, Vec<&ParameterAddress>> = HashMap::new();

    for row in variable_rows {
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

            for &dep in &deps {
                let dep_row = variable_rows.iter().find(|r| &r.address == dep);
                let is_resolvable = dep_row.is_some_and(|r| {
                    (r.is_enabled && r.coupling.is_none()) || r.coupling.is_some()
                });

                if !is_resolvable {
                    let dep_name = dep_row.map_or_else(
                        || "Unknown variable".to_string(),
                        |r| format!("{} ({})", r.node_name, r.property_name),
                    );
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

    // DFS Cycle Detection (0 = Unvisited, 1 = Visiting, 2 = Visited)
    let mut visit_state: HashMap<&ParameterAddress, u8> = HashMap::new();
    let mut reported_cycles: Vec<HashSet<&ParameterAddress>> = Vec::new();

    for &target in coupling_deps.keys() {
        if visit_state.get(target).copied().unwrap_or(0) == 0 {
            let mut path = Vec::new();
            if let Some(cycle) = dfs_find_cycle(target, &coupling_deps, &mut visit_state, &mut path)
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
                            variable_rows
                                .iter()
                                .find(|r| &r.address == *addr)
                                .map_or_else(
                                    || "Unknown".to_string(),
                                    |r| format!("{}: {}", r.node_name, r.property_name),
                                )
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
}

/// Performs depth-first traversal to locate a directed cycle in coupling dependencies.
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

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

        let rows = vec![
            UiVariableRow {
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
            },
            UiVariableRow {
                address: addr_b.clone(),
                group_id: None,
                group_name: "Group".to_string(),
                node_name: "Lens".to_string(),
                property_name: "R2".to_string(),
                current_value: -50.0,
                is_enabled: false,
                min_bound: None,
                max_bound: None,
                coupling: Some(CouplingType::Opposite { source: addr_a }),
            },
        ];

        let issues = run_validation(&rows, None);
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("Cyclic parameter coupling detected"))
        );
    }

    #[test]
    fn test_coupling_self_reference() {
        let node_id = Uuid::new_v4();
        let addr = ParameterAddress::NodeProperty {
            node_id,
            property_name: "radius_1".to_string(),
        };

        let rows = vec![UiVariableRow {
            address: addr.clone(),
            group_id: None,
            group_name: "Group".to_string(),
            node_name: "Lens".to_string(),
            property_name: "R1".to_string(),
            current_value: 50.0,
            is_enabled: false,
            min_bound: None,
            max_bound: None,
            coupling: Some(CouplingType::Identical { source: addr }),
        }];

        let issues = run_validation(&rows, None);
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("cannot reference itself"))
        );
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

        let rows = vec![
            UiVariableRow {
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
            },
            UiVariableRow {
                address: addr_b.clone(),
                group_id: None,
                group_name: "Group".to_string(),
                node_name: "Lens".to_string(),
                property_name: "R2".to_string(),
                current_value: -50.0,
                is_enabled: false,
                min_bound: None,
                max_bound: None,
                coupling: Some(CouplingType::Opposite { source: addr_c }),
            },
            UiVariableRow {
                address: addr_a,
                group_id: None,
                group_name: "Group".to_string(),
                node_name: "Lens".to_string(),
                property_name: "R1".to_string(),
                current_value: 50.0,
                is_enabled: false,
                min_bound: None,
                max_bound: None,
                coupling: Some(CouplingType::Identical { source: addr_b }),
            },
        ];

        let issues = run_validation(&rows, None);
        assert!(
            !issues
                .iter()
                .any(|i| i.message.contains("Cyclic")
                    || i.message.contains("cannot reference itself"))
        );
    }
}
