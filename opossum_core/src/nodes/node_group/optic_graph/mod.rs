#![warn(missing_docs)]
//! # Optical Graph Topology and Simulation Engine
//!
//! This module provides [`OpticGraph`], the core directed graph structure representing
//! an interconnected optical network of components and optical light flows within a
//! [`NodeGroup`](crate::nodes::NodeGroup).
//!
//! ## Submodules and Architecture
//!
//! * [`analysis`]: General analysis foundations, edge data propagation, topological sorting,
//!   and the [`InvertGraphGuard`] RAII abstraction for safe temporary graph inversion.
//! * [`construction`]: Topologic mutations including adding and removing nodes, creating
//!   and breaking connections, and managing port mappings.
//! * [`delta`]: Transactional delta records ([`GraphDelta`](delta::GraphDelta) and
//!   [`GraphDeletionDelta`](delta::GraphDeletionDelta)) for undo/redo state restoration.
//! * [`ghostfocus`]: Sequential multi-pass ghost focus analysis and stray ray tracking.
//! * [`graph`]: The primary [`OpticGraph`] definition, port maps, and connection metadata.
//! * [`inspection`]: Read-only graph inspection routines, connectivity queries, tree topology
//!   checks, and node lookups.
//! * [`raytrace`]: Forward ray tracing simulations and automated 3D node isometry positioning.
//! * [`serialization`]: Custom serialization and deserialization handling with tolerant
//!   node loading and cross-hierarchy reference proxy resolution.
//! * [`visualization`]: Graphviz DOT formatting utilities for optical system diagrams.

mod analysis;
mod construction;
pub mod delta;
mod ghostfocus;
mod graph;
mod inspection;
mod raytrace;
mod serialization;
mod visualization;

pub(crate) use analysis::InvertGraphGuard;
pub use graph::{ConnectionInfo, OpticGraph};
