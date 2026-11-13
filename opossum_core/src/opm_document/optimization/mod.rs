//! Serializable data types for configuring optical system optimization.
//!
//! This module defines the building blocks of an [`OptimizationConfig`]:
//! - **Addressing** ([`ParameterAddress`], [`ObservableAddress`], [`MetricSource`]): uniquely identifies
//!   tunable optical parameters and detector metrics across the model graph.
//! - **Variables & Bounds** ([`FreeVariable`], [`BoxBounds`], [`FiniteDifferenceMethod`]): specifies
//!   independent parameters tuned by the solver, box constraints, and finite-difference step sizes.
//! - **Couplings** ([`ParameterCoupling`], [`CouplingType`]): enforces deterministic relationships
//!   (such as symmetric radii or fixed total system lengths) between parameters.
//! - **Operands** ([`StandardOperand`], [`MetricOperand`], [`MaxTotalLengthOperand`],
//!   [`MinEdgeThicknessOperand`], [`TransmissionPenaltyOperand`]): defines target values, inequality
//!   limits, and geometric penalties for the merit function.
//! - **Algorithms** ([`OptimizationAlgorithm`]): configures local and global numerical solvers.
mod address;
mod algorithm;
mod config;
mod coupling;
mod discovery;
mod operand;
mod progress_event;
mod variable;
pub use address::{MetricSource, ObservableAddress, ParameterAddress};
pub use algorithm::OptimizationAlgorithm;
pub use config::OptimizationConfig;
pub use coupling::{CouplingType, ParameterCoupling};
pub use discovery::{
    DiscoveredAnalyzer, DiscoveredEdgeThicknessNode, DiscoveredObservable, DiscoveredVariable,
    ModelOptimizationSchema,
};
pub use operand::{
    DEFAULT_GEOMETRIC_SCALE, DEFAULT_SPOT_SCALE, DEFAULT_WAVEFRONT_SCALE, MaxTotalLengthOperand,
    MetricOperand, MinEdgeThicknessOperand, OperandMode, StandardOperand,
    TransmissionPenaltyOperand, default_scale_for_metric,
};
pub use progress_event::OptimizationProgressEvent;
pub use variable::{
    BoxBounds, DEFAULT_DELTA_ABS_MIN, DEFAULT_DELTA_REL, FiniteDifferenceMethod, FreeVariable,
};
