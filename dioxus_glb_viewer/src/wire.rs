//! Transport layer: converts an op list into a sequence of `serde_json::Value` messages,
//! each sent individually over a single `eval.send()` call.
//!
//! Byte payloads (`WireSource::Bytes`) are split into chunks so no single
//! `evaluate_script` call carries a huge Base64 string.

use crate::op::{Op, WireSource};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::Value;

/// Maximum Base64 length per chunk (~256 KiB decoded → ~341 KiB Base64).
/// Declared as `const` so it can be tuned without a protocol change.
const CHUNK_BYTES: usize = 256 * 1024;

/// Converts an op list into send-ready JSON values.
///
/// For each `Add`/`Reload` with `WireSource::Bytes`, the function prepends
/// `BytesBegin` + N×`BytesChunk` (built from the payload the op carries) before the op
/// itself. All other ops are serialised directly.
///
/// # Arguments
///
/// * `ops` - The op list produced by the diff.
///
/// # Returns
///
/// A flat list of JSON values to be sent in the given order, one at a time.
///
/// # Errors
///
/// Returns `Err` if an op cannot be serialised (practically never, since all fields are
/// serde-compatible).
pub fn messages(ops: Vec<Op>) -> Result<Vec<Value>, serde_json::Error> {
    let mut out = Vec::new();

    for op in ops {
        // Byte ops need BytesBegin + BytesChunk* prepended; all other ops go through directly.
        if let Op::Add {
            source: WireSource::Bytes { key, data },
            ..
        }
        | Op::Reload {
            source: WireSource::Bytes { key, data },
            ..
        } = &op
        {
            prepend_bytes_chunks(&mut out, key, data)?;
        }
        out.push(serde_json::to_value(&op)?);
    }

    Ok(out)
}

/// Appends `BytesBegin` + N×`BytesChunk` for one payload to `out`.
///
/// # Arguments
///
/// * `out` - Message list the stream is appended to.
/// * `key` - Transport key the following `Add`/`Reload` references.
/// * `raw` - The payload to encode.
///
/// # Errors
///
/// Returns `Err` if a transport op cannot be serialised (practically never).
fn prepend_bytes_chunks(
    out: &mut Vec<Value>,
    key: &str,
    raw: &[u8],
) -> Result<(), serde_json::Error> {
    let chunks: Vec<String> = raw.chunks(CHUNK_BYTES).map(|c| B64.encode(c)).collect();
    let n = chunks.len();
    out.push(serde_json::to_value(Op::BytesBegin {
        key: key.to_owned(),
        chunks: n,
    })?);
    for (seq, b64) in chunks.into_iter().enumerate() {
        out.push(serde_json::to_value(Op::BytesChunk {
            key: key.to_owned(),
            seq,
            b64,
        })?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff;
    use crate::model::{GlbObject, GlbSource, Transform};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn url_op(id: &str, url: &str) -> Op {
        Op::Add {
            id: id.into(),
            source: WireSource::Url { url: url.into() },
            transform: Transform::default(),
            visible: true,
            selected: false,
        }
    }

    fn bytes_op(id: &str, data: &[u8]) -> Op {
        Op::Add {
            id: id.into(),
            source: WireSource::Bytes {
                key: id.into(),
                data: Arc::from(data),
            },
            transform: Transform::default(),
            visible: true,
            selected: false,
        }
    }

    fn bytes_obj(id: &str, data: &[u8]) -> GlbObject {
        GlbObject {
            id: id.into(),
            source: GlbSource::Bytes(Arc::from(data)),
            transform: Transform::default(),
            visible: true,
            selected: false,
        }
    }

    #[test]
    fn url_op_single_message() {
        let ops = vec![url_op("a", "http://x/a.glb")];
        let msgs = messages(ops).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["op"], "add");
        assert_eq!(msgs[0]["source"]["kind"], "url");
    }

    #[test]
    fn bytes_op_emits_begin_chunks_then_add() {
        let ops = vec![bytes_op("a", &[0u8; 10])]; // small buffer → one chunk
        let msgs = messages(ops).unwrap();
        // bytes_begin + bytes_chunk + add = 3
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["op"], "bytes_begin");
        assert_eq!(msgs[0]["key"], "a");
        assert_eq!(msgs[0]["chunks"], 1);
        assert_eq!(msgs[1]["op"], "bytes_chunk");
        assert_eq!(msgs[1]["seq"], 0);
        assert_eq!(msgs[2]["op"], "add");
        assert_eq!(msgs[2]["source"]["kind"], "bytes");
    }

    #[test]
    fn bytes_payload_not_serialised_into_op() {
        let msgs = messages(vec![bytes_op("a", &[1, 2, 3])]).unwrap();
        assert_eq!(msgs[2]["op"], "add");
        assert!(
            msgs[2]["source"].get("data").is_none(),
            "payload must only travel as chunks: {}",
            msgs[2]
        );
    }

    #[test]
    fn large_bytes_multiple_chunks() {
        // slightly more than one chunk
        let ops = vec![bytes_op("b", &vec![0xAAu8; CHUNK_BYTES + 1])];
        let msgs = messages(ops).unwrap();
        // bytes_begin + 2×chunk + add = 4
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0]["chunks"], 2);
        assert_eq!(msgs[1]["seq"], 0);
        assert_eq!(msgs[2]["seq"], 1);
    }

    #[test]
    fn exactly_one_chunk_boundary() {
        // exactly CHUNK_BYTES → one chunk
        let ops = vec![bytes_op("c", &vec![0u8; CHUNK_BYTES])];
        let msgs = messages(ops).unwrap();
        // bytes_begin + 1×chunk + add = 3
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["chunks"], 1);
    }

    // Regression: `viewer.rs` used to rebuild the byte payloads in a separate map with
    // last-wins semantics, while `diff` resolves duplicate IDs first-wins — so the
    // "ignored" duplicate's bytes were the ones actually streamed.
    #[test]
    fn duplicate_bytes_ids_stream_first_entry_payload() {
        let mut applied = BTreeMap::new();
        let mut dupes = Vec::new();
        let next = [bytes_obj("a", &[1, 2, 3]), bytes_obj("a", &[9, 9, 9])];
        let msgs = messages(diff(&mut applied, &next, &mut dupes)).unwrap();

        assert_eq!(dupes, vec!["a"]);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["op"], "bytes_begin");
        assert_eq!(msgs[1]["op"], "bytes_chunk");
        assert_eq!(msgs[2]["op"], "add");
        let decoded = B64.decode(msgs[1]["b64"].as_str().unwrap()).unwrap();
        assert_eq!(decoded, [1, 2, 3]);
    }

    #[test]
    fn every_bytes_op_is_preceded_by_its_own_stream() {
        let mut applied = BTreeMap::new();
        let mut dupes = Vec::new();
        let url_obj = GlbObject {
            source: GlbSource::Url("http://x/u.glb".into()),
            ..bytes_obj("u", &[])
        };
        let first = [
            url_obj.clone(),
            bytes_obj("b", &[1, 2, 3]),
            bytes_obj("c", &vec![7u8; CHUNK_BYTES + 1]),
        ];
        // First pass → Adds; second pass with a new Arc for "b" → Reload.
        let mut msgs = messages(diff(&mut applied, &first, &mut dupes)).unwrap();
        let second = [url_obj, bytes_obj("b", &[4, 5]), first[2].clone()];
        msgs.extend(messages(diff(&mut applied, &second, &mut dupes)).unwrap());

        // (key, announced chunk count, chunks received so far)
        let mut pending: Option<(String, u64, u64)> = None;
        let mut bytes_ops = 0;
        for msg in &msgs {
            match msg["op"].as_str().unwrap() {
                "bytes_begin" => {
                    assert!(pending.is_none(), "stream started twice: {msg}");
                    let key = msg["key"].as_str().unwrap().to_owned();
                    pending = Some((key, msg["chunks"].as_u64().unwrap(), 0));
                }
                "bytes_chunk" => {
                    let (key, _, received) = pending.as_mut().expect("chunk without begin");
                    assert_eq!(msg["key"], key.as_str());
                    assert_eq!(msg["seq"], *received);
                    *received += 1;
                }
                "add" | "reload" if msg["source"]["kind"] == "bytes" => {
                    let (key, chunks, received) =
                        pending.take().expect("bytes op without preceding stream");
                    assert_eq!(msg["source"]["key"], key.as_str());
                    assert_eq!(received, chunks, "incomplete stream for {key}");
                    bytes_ops += 1;
                }
                _ => assert!(pending.is_none(), "stream interrupted by {msg}"),
            }
        }
        assert!(pending.is_none());
        assert_eq!(bytes_ops, 3, "Add b, Add c, Reload b");
    }

    #[test]
    fn non_bytes_ops_pass_through() {
        let ops = vec![
            Op::Remove { id: "x".into() },
            Op::SetVisible {
                id: "y".into(),
                visible: false,
            },
            Op::FitView,
            Op::Destroy,
        ];
        let msgs = messages(ops).unwrap();
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0]["op"], "remove");
        assert_eq!(msgs[1]["op"], "set_visible");
        assert_eq!(msgs[2]["op"], "fit_view");
        assert_eq!(msgs[3]["op"], "destroy");
    }
}
