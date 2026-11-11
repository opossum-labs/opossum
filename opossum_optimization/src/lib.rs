//! # OPOSSUM Optimization
//!
//! Numerical optimization engine for optical systems designed for the OPOSSUM simulation suite.
//!
//! Provides:
//! - Multi-analyzer merit function formulation with target and threshold operands.
//! - Parameter resolution and dependency couplings (identical, opposite, affine, total-length-lock).
//! - Supported numerical solvers via `argmin`:
//!   - Damped Least Squares (Levenberg-Marquardt with Nielsen adaptive damping)
//!   - Nelder-Mead Downhill Simplex (derivative-free direct search)
//!   - L-BFGS with backtracking line search (Armijo condition)
//!   - Adaptive Simulated Annealing (Corana 1/5th acceptance adaptation)
//! - Introspection and schema discovery for interactive GUI bindings.
pub mod address;
pub mod algorithm;
pub mod applier;
pub mod asa;
pub mod config;
pub mod coupling;
pub mod discovery;
pub mod dls;
pub mod merit;
pub mod operand;
pub mod optimizer;
pub mod progress;
pub mod reader;
pub mod resolver;
pub mod schema_extractor;
pub mod summary;
pub mod variable;

pub use address::{MetricSource, ObservableAddress, ParameterAddress};
pub use algorithm::OptimizationAlgorithm;
pub use applier::ParameterApplier;
pub use config::OptimizationConfig;
pub use coupling::{CouplingType, ParameterCoupling};
pub use discovery::{
    DiscoveredEdgeThicknessNode, DiscoveredObservable, DiscoveredVariable, ModelOptimizationSchema,
};
pub use dls::DampedLeastSquares;
pub use merit::{DEFAULT_BOUNDS_PENALTY_WEIGHT, DEFAULT_FATAL_PENALTY, MeritFunction};
pub use operand::{
    EvaluationContext, MaxTotalLengthOperand, MetricOperand, MinEdgeThicknessOperand, Operand,
    OperandMode, StandardOperand, TransmissionPenaltyOperand, default_scale_for_metric,
};
pub use optimizer::OpticOptimizer;
pub use progress::OptimizationProgressEvent;
pub use reader::MetricReader;
pub use resolver::ParameterResolver;
pub use schema_extractor::SchemaExtractor;
pub use summary::OptimizationSummary;
pub use variable::{BoxBounds, FreeVariable};
