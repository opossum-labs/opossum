use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique address identifying a tunable variable in the optical model.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterAddress {
    /// A scalar property located on a node (e.g. radius of curvature, thickness, tilt).
    NodeProperty {
        /// id of the node
        node_id: Uuid,
        /// name of the property
        property_name: String,
    },
    /// A spherical optical surface curvature.
    ///
    /// Solver representation: Curvature $c$ in SI base units [$1/\text{m}$] ($c = 1/R$).
    /// Underlying model property: Radius of curvature $R$ in [$m$].
    /// A curvature of $c = 0.0$ corresponds to a flat surface ($R = \pm\infty$).
    SurfaceCurvature {
        /// id of the node
        node_id: Uuid,
        /// name of the curvature property
        property_name: String,
    },
    /// A distance starting from an output port of a node.
    ConnectionDistance {
        /// if of the source node this connection belongs to
        src_id: Uuid,
        /// port name of the source node this connection bleongs to
        src_port: String,
    },
}

impl Display for ParameterAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeProperty {
                node_id,
                property_name,
            } => {
                write!(f, "{node_id}.{property_name}")
            }
            Self::SurfaceCurvature {
                node_id,
                property_name,
            } => {
                write!(f, "{node_id}.{property_name}(curvature)")
            }
            Self::ConnectionDistance { src_id, src_port } => {
                write!(f, "Distance({src_id}:{src_port})")
            }
        }
    }
}

/// Unique address identifying an observable output metric (operand source).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservableAddress {
    /// Analyzer producing the evaluation.
    pub analyzer_id: Uuid,
    /// Detector node producing the metric.
    pub node_id: Uuid,
    /// Key of the observable metric (e.g. "Energy", "rms beam radius").
    pub metric_name: String,
}

impl Display for ObservableAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}[analyzer: {}]",
            self.node_id, self.metric_name, self.analyzer_id
        )
    }
}

/// Unified source for quantities evaluated by optimization operands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricSource {
    /// A physics metric recorded by a detector during analysis.
    Observable(ObservableAddress),
    /// A geometric or model property (e.g. thickness, radius, spacing).
    Parameter(ParameterAddress),
}

impl Display for MetricSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observable(addr) => write!(f, "Observable({addr})"),
            Self::Parameter(addr) => write!(f, "Param({addr})"),
        }
    }
}

impl From<ObservableAddress> for MetricSource {
    fn from(address: ObservableAddress) -> Self {
        Self::Observable(address)
    }
}

impl From<ParameterAddress> for MetricSource {
    fn from(address: ParameterAddress) -> Self {
        Self::Parameter(address)
    }
}
