//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::fmt::Display;

use crate::{error::OpmResult, rays::Rays};
use serde::{Deserialize, Serialize};

use super::LightData;

/// Builder for the generation of [`LightData::Geometric`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RayDataBuilder {
    /// Raw [`Rays`] data.
    Raw(Rays),
}

impl RayDataBuilder {
    /// Create [`LightData::Geometric`] from the builder definition.
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(self) -> OpmResult<LightData> {
        match self {
            Self::Raw(rays) => Ok(LightData::Geometric(rays)),
        }
    }
}

impl Display for RayDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(r) => write!(f, "Raw({r})"),
        }
    }
}
