use std::collections::{HashMap, HashSet};

use crate::{
    address::ParameterAddress,
    coupling::{CouplingType, ParameterCoupling},
    variable::FreeVariable,
};
use opossum_core::error::{OpmResult, OpossumError};

/// Resolves solver variable vectors and parameter couplings into concrete system values[cite: 1].
pub struct ParameterResolver;

impl ParameterResolver {
    /// Validates configuration integrity:
    /// - No parameter can be both a free variable and a coupling target.
    /// - No target parameter can be claimed by multiple couplings.
    /// - TotalLengthLock cannot reference its own target.
    pub fn validate_configuration(
        free_vars: &[FreeVariable],
        couplings: &[ParameterCoupling],
    ) -> OpmResult<()> {
        let mut free_addresses = HashSet::new();
        for var in free_vars {
            if !free_addresses.insert(&var.address) {
                return Err(OpossumError::Other(format!(
                    "Duplicate free variable address: {:?}",
                    var.address
                )));
            }
        }

        let mut target_addresses = HashSet::new();
        for coupling in couplings {
            if free_addresses.contains(&coupling.target) {
                return Err(OpossumError::Other(format!(
                    "Target parameter {:?} cannot simultaneously be a free variable",
                    coupling.target
                )));
            }
            if !target_addresses.insert(&coupling.target) {
                return Err(OpossumError::Other(format!(
                    "Duplicate coupling target: {:?}",
                    coupling.target
                )));
            }

            // Check self-reference in TotalLengthLock
            if let CouplingType::TotalLengthLock { other_segments, .. } = &coupling.rule {
                if other_segments.contains(&coupling.target) {
                    return Err(OpossumError::Other(format!(
                        "TotalLengthLock target {:?} cannot be part of other_segments",
                        coupling.target
                    )));
                }
            }
        }

        Ok(())
    }

    /// Evaluates the solver parameter vector `x` and all couplings to produce the final parameter map[cite: 1].
    pub fn resolve(
        free_vars: &[FreeVariable],
        couplings: &[ParameterCoupling],
        x: &[f64],
    ) -> OpmResult<HashMap<ParameterAddress, f64>> {
        if free_vars.len() != x.len() {
            return Err(OpossumError::Other(format!(
                "Dimension mismatch: expected {} free variables, got {}",
                free_vars.len(),
                x.len()
            )));
        }

        let mut values = HashMap::with_capacity(free_vars.len() + couplings.len());

        // 1. Populate values from solver vector
        for (var, &val) in free_vars.iter().zip(x.iter()) {
            values.insert(var.address.clone(), val);
        }

        // 2. Resolve couplings (supports multi-pass resolution for cascaded couplings)
        let mut pending = couplings.to_vec();
        let mut progress = true;

        while !pending.is_empty() && progress {
            progress = false;
            let mut remaining = Vec::new();

            for coupling in pending {
                let resolved_val = match &coupling.rule {
                    CouplingType::Identical { source } => values.get(source).copied(),
                    CouplingType::Opposite { source } => values.get(source).map(|&v| -v),
                    CouplingType::Affine {
                        source,
                        factor,
                        offset,
                    } => values.get(source).map(|&v| v * factor + offset),
                    CouplingType::TotalLengthLock {
                        total_length,
                        other_segments,
                    } => {
                        let all_present = other_segments.iter().all(|seg| values.contains_key(seg));
                        if all_present {
                            let sum: f64 = other_segments
                                .iter()
                                .map(|seg| values.get(seg).copied().unwrap_or(0.0))
                                .sum();
                            Some(total_length - sum)
                        } else {
                            None
                        }
                    }
                };

                if let Some(val) = resolved_val {
                    values.insert(coupling.target.clone(), val);
                    progress = true;
                } else {
                    remaining.push(coupling);
                }
            }

            pending = remaining;
        }

        if !pending.is_empty() {
            return Err(OpossumError::Other(format!(
                "Unresolved or cyclic parameter couplings detected: {} rule(s) pending",
                pending.len()
            )));
        }

        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn resolve_couplings_successfully() -> OpmResult<()> {
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();

        let addr_r1 = ParameterAddress::NodeProperty {
            node_id: n1,
            property_name: "radius_1".into(),
        };
        let addr_r2 = ParameterAddress::NodeProperty {
            node_id: n1,
            property_name: "radius_2".into(),
        };
        let addr_d1 = ParameterAddress::ConnectionDistance {
            src_id: n1,
            src_port: "out".into(),
        };
        let addr_d2 = ParameterAddress::ConnectionDistance {
            src_id: n2,
            src_port: "out".into(),
        };

        let free_vars = vec![
            FreeVariable::new(addr_r1.clone(), "r1", 0.050),
            FreeVariable::new(addr_d1.clone(), "d1", 0.020),
        ];

        let couplings = vec![
            // Opposite: r2 = -r1
            ParameterCoupling::opposite(addr_r2.clone(), addr_r1.clone()),
            // Total length: d1 + d2 = 0.100 m => d2 = 0.100 - d1
            ParameterCoupling::total_length_lock(addr_d2.clone(), 0.100, vec![addr_d1.clone()])?,
        ];

        ParameterResolver::validate_configuration(&free_vars, &couplings)?;

        let x = [0.075, 0.030];
        let resolved = ParameterResolver::resolve(&free_vars, &couplings, &x)?;

        assert_eq!(resolved.get(&addr_r1), Some(&0.075));
        assert_eq!(resolved.get(&addr_r2), Some(&-0.075));
        assert_eq!(resolved.get(&addr_d1), Some(&0.030));
        assert!((resolved.get(&addr_d2).unwrap() - 0.070).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn detect_cyclic_or_invalid_couplings() -> OpmResult<()> {
        let id = Uuid::new_v4();
        let addr1 = ParameterAddress::NodeProperty {
            node_id: id,
            property_name: "p1".into(),
        };
        let addr2 = ParameterAddress::NodeProperty {
            node_id: id,
            property_name: "p2".into(),
        };

        let free_vars = vec![FreeVariable::new(addr1.clone(), "p1", 1.0)];

        // Conflict: addr1 cannot be both free and coupling target
        let invalid_couplings = vec![ParameterCoupling::identical(addr1.clone(), addr2.clone())];
        assert!(ParameterResolver::validate_configuration(&free_vars, &invalid_couplings).is_err());

        // Cycle: a -> b and b -> a without free source
        let addr3 = ParameterAddress::NodeProperty {
            node_id: id,
            property_name: "p3".into(),
        };
        let cyclic_couplings = vec![
            ParameterCoupling::identical(addr2.clone(), addr3.clone()),
            ParameterCoupling::identical(addr3.clone(), addr2.clone()),
        ];
        assert!(ParameterResolver::resolve(&free_vars, &cyclic_couplings, &[1.0]).is_err());

        Ok(())
    }
}
