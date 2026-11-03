//! The crate error type.

use crate::model::{MaterialId, MeshId};
use std::fmt;

/// Errors returned by the `optoscene` API.
#[derive(Debug)]
pub enum Error {
    /// A mesh failed validation (see [`crate::Scene::add_mesh`]).
    InvalidMesh(String),
    /// A ray trace failed validation.
    InvalidRayTrace(String),
    /// A referenced material id is not registered in the scene.
    UnknownMaterial(MaterialId),
    /// A referenced mesh id is not registered in the scene.
    UnknownMesh(MeshId),
    /// A node with this uid already exists in the scene.
    DuplicateUid(String),
    /// No node with this uid exists in the scene.
    UnknownUid(String),
    /// An I/O error occurred while writing output.
    Io(std::io::Error),
    /// A JSON serialization error occurred.
    Json(serde_json::Error),
    /// Encoding a scene message into a protocol frame failed.
    #[cfg(feature = "protocol")]
    Frame(optoscene_protocol::FrameError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMesh(msg) => write!(f, "invalid mesh: {msg}"),
            Self::InvalidRayTrace(msg) => write!(f, "invalid ray trace: {msg}"),
            Self::UnknownMaterial(id) => write!(f, "unknown material id {}", id.raw()),
            Self::UnknownMesh(id) => write!(f, "unknown mesh id {}", id.raw()),
            Self::DuplicateUid(uid) => write!(f, "duplicate node uid {uid:?}"),
            Self::UnknownUid(uid) => write!(f, "unknown node uid {uid:?}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Json(err) => write!(f, "json error: {err}"),
            #[cfg(feature = "protocol")]
            Self::Frame(err) => write!(f, "frame error: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            #[cfg(feature = "protocol")]
            Self::Frame(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

#[cfg(feature = "protocol")]
impl From<optoscene_protocol::FrameError> for Error {
    fn from(err: optoscene_protocol::FrameError) -> Self {
        Self::Frame(err)
    }
}
