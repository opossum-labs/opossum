//! Builder for geometric light data generation.
//!
//! This module provides the [`RayDataBuilder`] struct, which acts as a wrapper
//! around a [`RayDataSource`]. It handles the generation of rays and applies
//! global properties such as spatial transformations (isometry) to the entire
//! emitted light field.

use crate::{
    error::OpmResult,
    light::Rays,
    prelude::{Isometry, RayDataSource},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Builder struct for generating geometric [`Rays`].
///
/// This struct combines a specific [`RayDataSource`] (e.g., collimated, point source, or image)
/// with global physical properties that apply to the entire generated light field.
/// This includes its 3D position and orientation in the optical system ([`Isometry`]).
///
/// It describes only the light an analysis sends in; where the setup stands follows from the
/// source ports themselves (see [`SourcePort`](crate::nodes::SourcePort)).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RayDataBuilder {
    source: RayDataSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    isometry: Option<Isometry>,
}

impl Default for RayDataBuilder {
    /// Creates a default [`RayDataBuilder`].
    ///
    /// The default builder uses the default [`RayDataSource`], with no isometry
    /// (placed at the origin without rotation).
    fn default() -> Self {
        Self {
            source: RayDataSource::default(),
            isometry: None,
        }
    }
}

impl RayDataBuilder {
    /// Builds the rays and automatically applies the global source isometry.
    ///
    /// This method first generates the raw rays using the underlying [`RayDataSource`].
    /// If an [`Isometry`] is defined for this builder, it is then applied to transform
    /// all generated rays into their correct global position and orientation.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying [`RayDataSource`] fails to build the rays
    /// (e.g., due to invalid distributions or file paths).
    pub fn build(&self) -> OpmResult<Rays> {
        Ok(self.placed(self.source.clone().build()?))
    }
    /// Places the given rays, sent out by the source of this builder in its own frame, with the
    /// isometry of this builder, if it has one.
    ///
    /// # Arguments
    ///
    /// * `rays` - the rays as the [`RayDataSource`] of this builder builds them.
    ///
    /// # Returns
    ///
    /// The rays where this builder puts its light.
    #[must_use]
    pub fn placed(&self, mut rays: Rays) -> Rays {
        if let Some(iso) = &self.isometry {
            rays = rays.transformed_by_iso(iso);
        }
        rays
    }

    /// Sets the underlying [`RayDataSource`].
    ///
    /// # Arguments
    ///
    /// * `source` - The new ray data source to use for ray generation.
    pub fn set_source(&mut self, source: RayDataSource) {
        self.source = source;
    }
    /// Returns a reference to the underlying [`RayDataSource`].
    #[must_use]
    pub const fn source(&self) -> &RayDataSource {
        &self.source
    }

    /// Sets the global [`Isometry`] (position and orientation) of the generated light field.
    ///
    /// # Arguments
    ///
    /// * `isometry` - The new isometry, or `None` to generate rays at the local origin.
    pub const fn set_isometry(&mut self, isometry: Option<Isometry>) {
        self.isometry = isometry;
    }

    /// Returns the currently defined global [`Isometry`], if any.
    #[must_use]
    pub const fn isometry(&self) -> Option<Isometry> {
        self.isometry
    }
}

impl From<RayDataSource> for RayDataBuilder {
    /// Creates a new [`RayDataBuilder`] from a given [`RayDataSource`].
    ///
    /// The resulting builder will wrap the provided source but will have no
    /// isometry set initially.
    fn from(value: RayDataSource) -> Self {
        let mut rdb = Self::default();
        rdb.set_source(value);
        rdb
    }
}
