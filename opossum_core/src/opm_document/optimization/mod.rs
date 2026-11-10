//! Optical optimization data definitions and serialization configurations for [`OpmDocument`](super::OpmDocument).
#![allow(missing_docs)]

pub mod address;
pub mod algorithm;
pub mod config;
pub mod coupling;
pub mod operand;
pub mod variable;

pub use address::{MetricSource, ObservableAddress, ParameterAddress};
pub use algorithm::OptimizationAlgorithm;
pub use config::OptimizationConfig;
pub use coupling::{CouplingType, ParameterCoupling};
pub use operand::{
    DEFAULT_GEOMETRIC_SCALE, DEFAULT_SPOT_SCALE, DEFAULT_WAVEFRONT_SCALE, MaxTotalLengthOperand,
    MinEdgeThicknessOperand, StandardOperand, TargetValueOperand, ThresholdDirection,
    ThresholdOperand, TransmissionPenaltyOperand, default_scale_for_metric,
};
pub use variable::{
    BoxBounds, DEFAULT_DELTA_ABS_MIN, DEFAULT_DELTA_REL, FiniteDifferenceMethod, FreeVariable,
};
