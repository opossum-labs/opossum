//! The result of a positioning run on a copy of a model.

use crate::{
    error::OpmResult,
    light::Rays,
    nodes::{ConnectionInfo, NodeGroup},
    utils::geom_transformation::Isometry,
};
use uuid::Uuid;

/// The result of one positioning run on a copy of a model's scenery, made by
/// [`OpmDocument::run_positioning`](super::OpmDocument::run_positioning).
///
/// Everything that draws a model needs the same run: where each component sits, the optical axis
/// that put it there, and which connections placed a node by their distance. Holding them together
/// lets one run answer all three.
#[derive(Debug)]
pub struct PositioningRun {
    scenery: NodeGroup,
    optical_axis: Vec<Rays>,
    outcome: OpmResult<()>,
}

impl PositioningRun {
    /// Wraps a copy the positioning run went through, with what the run gave.
    ///
    /// # Arguments
    ///
    /// - `scenery`: the copy, placed as far as the run got, with the ray ends of its axis collected
    /// - `outcome`: what the run gave: an error if it stopped part way
    ///
    /// # Returns
    ///
    /// The result, with the ray ends taken out of the copy as its optical axis.
    pub(super) fn new(mut scenery: NodeGroup, outcome: OpmResult<()>) -> Self {
        let optical_axis = scenery.take_ray_ends();
        Self {
            scenery,
            optical_axis,
            outcome,
        }
    }

    /// Returns the copy with every node at the position the run gave it.
    ///
    /// # Errors
    ///
    /// This function returns the error the run stopped with, if it stopped part way.
    pub fn positioned_scenery(&self) -> OpmResult<&NodeGroup> {
        self.outcome.clone().map(|()| &self.scenery)
    }

    /// Returns the optical axis: the one ray per source the run followed, as it leaves the model
    /// through every open output port, each ray carrying its path in global coordinates.
    ///
    /// Light that ends in a component without any output port is missing.
    ///
    /// # Errors
    ///
    /// This function returns the error the run stopped with, if it stopped part way.
    pub fn optical_axis(&self) -> OpmResult<&[Rays]> {
        self.outcome.clone().map(|()| self.optical_axis.as_slice())
    }

    /// Returns the connections of all groups of the model whose distance placed a node in this
    /// run (see [`NodeGroup::connections_placing_by_distance`]).
    ///
    /// A run that stopped part way still lists the distances that placed a node before it
    /// stopped.
    #[must_use]
    pub fn connections_placing_by_distance(&self) -> Vec<ConnectionInfo> {
        self.scenery.connections_placing_by_distance()
    }

    /// Returns where the run placed each node of the model, or `None` for a node it could not
    /// place (see [`NodeGroup::placements`]).
    ///
    /// A run that stopped part way gives the placements it made before it stopped.
    #[must_use]
    pub fn placements(&self) -> Vec<(Uuid, Option<Isometry>)> {
        self.scenery.placements()
    }
}
