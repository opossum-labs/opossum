//! Streaming frame format for `optoscene` scene updates.
//!
//! This crate is intentionally minimal and dependency-light so that it compiles
//! for `wasm32-unknown-unknown` (no `std::fs`, no threads). A frame is a small
//! binary envelope around a JSON [`Header`] and an optional binary payload (a
//! GLB, for the variants that carry geometry):
//!
//! | Offset | Size | Content |
//! |---|---|---|
//! | 0 | 4 | magic `b"OSCN"` |
//! | 4 | 1 | protocol version (`1`) |
//! | 5 | 4 | header length `H` as little-endian `u32` |
//! | 9 | `H` | header as UTF-8 JSON |
//! | 9 + `H` | rest | payload (GLB or empty) |
//!
//! Use [`encode`] to build a frame and [`decode`] to read one back.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

/// The frame magic, identifying an `optoscene` stream frame.
const MAGIC: &[u8; 4] = b"OSCN";

/// The protocol version this crate reads and writes.
const VERSION: u8 = 1;

/// The number of bytes before the header: magic (4) + version (1) + length (4).
const PREAMBLE_LEN: usize = 9;

/// Message header, serialized as JSON with a `"type"` tag.
///
/// Each variant tells the client what to do with the frame's payload; see the
/// crate docs for the client semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Header {
    /// Payload: the complete scene as a GLB. Replace everything.
    FullScene,
    /// Payload: a GLB containing only this layer. Replace that layer's group.
    ReplaceLayer {
        /// The layer name (`"optics"`, `"rays"`, `"aux"`).
        layer: String,
    },
    /// Payload: a GLB containing the new or changed nodes. Upsert by `uid`.
    UpsertNodes {
        /// The uids contained in the payload.
        uids: Vec<String>,
    },
    /// No payload: set the transformation of existing nodes.
    UpdateTransforms {
        /// The nodes whose transformation changed.
        nodes: Vec<TransformEntry>,
    },
    /// No payload: remove the given nodes.
    RemoveNodes {
        /// The uids to remove.
        uids: Vec<String>,
    },
}

/// New transformation of a node, relative to the scene origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformEntry {
    /// The uid of the node to move.
    pub uid: String,
    /// Translation relative to the scene origin, in meters.
    pub translation: [f32; 3],
    /// Rotation as a quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

/// An error produced while encoding or decoding a frame.
#[derive(Debug)]
pub enum FrameError {
    /// The frame does not start with the `b"OSCN"` magic.
    BadMagic,
    /// The frame declares a protocol version this crate does not support.
    UnsupportedVersion(u8),
    /// The frame ends before the declared header or preamble is complete.
    Truncated,
    /// The header does not fit into the `u32` length field.
    HeaderTooLarge,
    /// The header JSON could not be serialized or deserialized.
    Json(serde_json::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => f.write_str("frame does not start with the OSCN magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported protocol version {version}")
            }
            Self::Truncated => f.write_str("frame is truncated"),
            Self::HeaderTooLarge => f.write_str("header does not fit into a u32 length"),
            Self::Json(error) => write!(f, "header JSON error: {error}"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for FrameError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Encodes a header and payload into a frame.
///
/// # Arguments
/// * `header` — the message header, serialized to JSON.
/// * `payload` — the binary payload (a GLB, or empty for payload-less headers).
///
/// # Returns
/// The complete frame bytes.
///
/// # Errors
/// Returns [`FrameError::Json`] if the header cannot be serialized, or
/// [`FrameError::HeaderTooLarge`] if the header JSON exceeds `u32::MAX` bytes.
pub fn encode(header: &Header, payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    let header_json = serde_json::to_vec(header)?;
    let header_len = u32::try_from(header_json.len()).map_err(|_| FrameError::HeaderTooLarge)?;

    let mut frame = Vec::with_capacity(PREAMBLE_LEN + header_json.len() + payload.len());
    frame.extend_from_slice(MAGIC);
    frame.push(VERSION);
    frame.extend_from_slice(&header_len.to_le_bytes());
    frame.extend_from_slice(&header_json);
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Decodes a frame into its header and a borrowed slice of its payload.
///
/// # Arguments
/// * `frame` — the complete frame bytes.
///
/// # Returns
/// The parsed [`Header`] and the payload slice (empty for payload-less headers).
///
/// # Errors
/// Returns [`FrameError::BadMagic`] if the magic is wrong,
/// [`FrameError::UnsupportedVersion`] if the version is not supported,
/// [`FrameError::Truncated`] if the frame ends early, or [`FrameError::Json`] if
/// the header JSON is invalid.
pub fn decode(frame: &[u8]) -> Result<(Header, &[u8]), FrameError> {
    let magic = frame.get(0..4).ok_or(FrameError::Truncated)?;
    if magic != MAGIC {
        return Err(FrameError::BadMagic);
    }
    let version = *frame.get(4).ok_or(FrameError::Truncated)?;
    if version != VERSION {
        return Err(FrameError::UnsupportedVersion(version));
    }

    let length_bytes: [u8; 4] = frame
        .get(5..PREAMBLE_LEN)
        .ok_or(FrameError::Truncated)?
        .try_into()
        .map_err(|_| FrameError::Truncated)?;
    let header_len = u32::from_le_bytes(length_bytes) as usize;
    let header_end = PREAMBLE_LEN
        .checked_add(header_len)
        .ok_or(FrameError::Truncated)?;

    let header_bytes = frame
        .get(PREAMBLE_LEN..header_end)
        .ok_or(FrameError::Truncated)?;
    let header = serde_json::from_slice(header_bytes)?;
    let payload = frame.get(header_end..).unwrap_or(&[]);
    Ok((header, payload))
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, FrameError, Header, TransformEntry, MAGIC, PREAMBLE_LEN, VERSION};

    /// Every header variant, for the roundtrip test.
    fn sample_headers() -> Vec<Header> {
        vec![
            Header::FullScene,
            Header::ReplaceLayer {
                layer: "rays".to_string(),
            },
            Header::UpsertNodes {
                uids: vec!["a".to_string(), "b".to_string()],
            },
            Header::UpdateTransforms {
                nodes: vec![TransformEntry {
                    uid: "lens".to_string(),
                    translation: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                }],
            },
            Header::RemoveNodes {
                uids: vec!["gone".to_string()],
            },
        ]
    }

    #[test]
    fn roundtrip_every_variant_with_and_without_payload() {
        for header in sample_headers() {
            for payload in [b"".as_slice(), b"glTF-payload".as_slice()] {
                let frame = encode(&header, payload).unwrap();
                let (decoded, decoded_payload) = decode(&frame).unwrap();
                assert_eq!(decoded, header);
                assert_eq!(decoded_payload, payload);
            }
        }
    }

    #[test]
    fn type_tag_uses_snake_case() {
        let frame = encode(&Header::FullScene, &[]).unwrap();
        let json = &frame[PREAMBLE_LEN..];
        assert_eq!(json, br#"{"type":"full_scene"}"#.as_slice());
    }

    #[test]
    fn bad_magic_is_reported() {
        let mut frame = encode(&Header::FullScene, &[]).unwrap();
        frame[0] = b'X';
        assert!(matches!(decode(&frame), Err(FrameError::BadMagic)));
    }

    #[test]
    fn unsupported_version_is_reported() {
        let mut frame = encode(&Header::FullScene, &[]).unwrap();
        frame[4] = VERSION + 1;
        assert!(matches!(
            decode(&frame),
            Err(FrameError::UnsupportedVersion(v)) if v == VERSION + 1
        ));
    }

    #[test]
    fn truncated_frames_are_reported() {
        // Shorter than the preamble.
        assert!(matches!(decode(MAGIC), Err(FrameError::Truncated)));

        // Declares a header longer than the bytes that follow.
        let frame = encode(&Header::FullScene, &[]).unwrap();
        let cut = &frame[..frame.len() - 1];
        assert!(matches!(decode(cut), Err(FrameError::Truncated)));
    }

    #[test]
    fn invalid_header_json_is_reported() {
        // Valid preamble, but the header bytes are not valid JSON for a Header.
        let body = b"not json";
        let header_len = u32::try_from(body.len()).unwrap();
        let mut frame = Vec::new();
        frame.extend_from_slice(MAGIC);
        frame.push(VERSION);
        frame.extend_from_slice(&header_len.to_le_bytes());
        frame.extend_from_slice(body);
        assert!(matches!(decode(&frame), Err(FrameError::Json(_))));
    }
}
