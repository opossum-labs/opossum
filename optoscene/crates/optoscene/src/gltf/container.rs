//! glTF container formats: the binary GLB wrapper and the `.gltf` + `.bin` pair.

/// GLB header magic: the ASCII bytes `glTF` read as a little-endian `u32`.
const GLB_MAGIC: u32 = 0x4654_6c67;
/// GLB container version.
const GLB_VERSION: u32 = 2;
/// Chunk type for the JSON chunk (`JSON`).
const CHUNK_TYPE_JSON: u32 = 0x4e4f_534a;
/// Chunk type for the binary chunk (`BIN\0`).
const CHUNK_TYPE_BIN: u32 = 0x004e_4942;

/// Packs a glTF JSON document and its binary buffer into a GLB byte stream.
///
/// The JSON chunk is padded to a 4-byte boundary with spaces (`0x20`); the
/// binary chunk, if present, is padded with zeros (`0x00`). The binary chunk is
/// omitted entirely when `bin` is empty.
///
/// # Arguments
/// * `json` — the UTF-8 glTF JSON document.
/// * `bin` — the binary buffer (`buffers[0]`); may be empty.
///
/// # Returns
/// The GLB bytes.
pub fn glb_bytes(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_padded = pad_to_four(json.len());
    let has_bin = !bin.is_empty();
    let bin_padded = if has_bin { pad_to_four(bin.len()) } else { 0 };

    let mut total = 12 + 8 + json_padded;
    if has_bin {
        total += 8 + bin_padded;
    }

    let mut out = Vec::with_capacity(total);
    // Header.
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&as_u32(total).to_le_bytes());
    // JSON chunk.
    out.extend_from_slice(&as_u32(json_padded).to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
    out.extend_from_slice(json);
    out.resize(out.len() + (json_padded - json.len()), b' ');
    // BIN chunk.
    if has_bin {
        out.extend_from_slice(&as_u32(bin_padded).to_le_bytes());
        out.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
        out.extend_from_slice(bin);
        out.resize(out.len() + (bin_padded - bin.len()), 0);
    }
    out
}

/// Rounds a length up to the next multiple of four.
const fn pad_to_four(len: usize) -> usize {
    len.div_ceil(4) * 4
}

/// Narrows a byte length to the `u32` used by the GLB header/chunk fields.
#[allow(clippy::cast_possible_truncation)]
const fn as_u32(len: usize) -> u32 {
    // GLB lengths are `u32` by specification; optical scenes never reach 4 GiB.
    len as u32
}
