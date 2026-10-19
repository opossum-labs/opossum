#![warn(missing_docs)]
//! The solids a component can be made of.
//!
//! A [`Solid`] is either two faces capping a profile extruded along an axis ([`Extruded`]), the
//! intersection of the inner sides of several faces ([`ParametricSolid`]) or several solids sharing
//! interfaces ([`Assembly`]). Each kind lives in a file of its own, together with what it can do.
//!
//! A solid is described rather than built. A [`Placed`] solid is the description built at the
//! placement of its node, which is what can answer geometric questions such as whether a point
//! lies inside it (see [`Body`](crate::geometry::body::Body)).

mod assembly;
mod extruded;
mod parametric;

pub use assembly::{Assembly, FaceId, Interface, Part};
pub use extruded::Extruded;
pub use parametric::ParametricSolid;

use serde::{Deserialize, Serialize};

use crate::{
    geometry::{Face, PlacedSurface},
    utils::geom_transformation::Isometry,
};

/// A component enclosing a volume of material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Solid {
    /// Two faces capping a profile extruded along an axis.
    Extruded(Box<Extruded>),
    /// The intersection of the inner sides of several faces.
    Parametric(ParametricSolid),
    /// Several solids sharing interfaces.
    Assembly(Assembly),
}

impl Solid {
    /// Return the face with the given index, as an [`Interface`] refers to it.
    ///
    /// # Arguments
    ///
    /// * `index` - for an [`Extruded`] solid 0 is the front and 1 the rear face; for a
    ///   [`ParametricSolid`] it is the position in its list of faces.
    ///
    /// # Returns
    ///
    /// The face, the frame it is stated in relative to the solid, and the side of it the solid lies
    /// on; or `None` if there is no such face. An [`Assembly`] has no faces of its own.
    fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        match self {
            Self::Extruded(extruded) => extruded.face(index),
            Self::Parametric(parametric) => parametric.face(index),
            Self::Assembly(_) => None,
        }
    }
}

/// Which side of a face lies inside a [`ParametricSolid`], as
/// [`GeoSurface::is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind) tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// The side the surface reports as behind it.
    Behind,
    /// The side the surface reports as in front of it.
    InFront,
}

/// A solid built at the placement of its node (see [`Extruded::place`]).
///
/// It keeps the description it was built from next to what building adds: the placement of the
/// node and the geometric surfaces light enters and leaves through. Those are the very surfaces a
/// node traces its rays at (see [`PlacedGeometry`]), shared rather than built a second time.
///
/// **Note**: A solid placed this way has exactly one entrance and one exit surface, the assumption
/// the tracing of a node makes as a whole.
#[derive(Debug, Clone)]
pub struct Placed<T> {
    solid: T,
    node_frame: Isometry,
    entrance: PlacedSurface,
    exit: PlacedSurface,
}

impl<T> Placed<T> {
    /// Return the description this solid was built from.
    #[must_use]
    pub const fn solid(&self) -> &T {
        &self.solid
    }
    /// Return the placement of the node this solid was built at.
    #[must_use]
    pub const fn node_frame(&self) -> &Isometry {
        &self.node_frame
    }
    /// Return the surface light enters through, with its anchor relative to the node.
    #[must_use]
    pub const fn entrance(&self) -> &PlacedSurface {
        &self.entrance
    }
    /// Return the surface light leaves through, with its anchor relative to the node.
    #[must_use]
    pub const fn exit(&self) -> &PlacedSurface {
        &self.exit
    }
}
