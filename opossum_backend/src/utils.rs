use opossum::nodes::NodeAttr;
use serde_json::Value;

use crate::error::ErrorResponse;

/// Update a given [`NodeAttr`] by a JSON object.
///
/// # Errors
///
/// This function will return an error if
/// - [`NodeAttr`] cannot be serialized or deserialized.
/// - an error by updating the fields occurred.
pub fn update_node_attr(
    node_attr: &NodeAttr,
    updates: &serde_json::Value,
) -> Result<NodeAttr, ErrorResponse> {
    let orig_uuid = node_attr.uuid();
    let mut node_attr_json = serde_json::to_value(node_attr).map_err(|e| {
        ErrorResponse::new(
            400,
            "serialization error",
            &format!("error serializing NodeAttr: {e}"),
        )
    })?;
    update_json(&mut node_attr_json, updates)?;
    let mut updated_node_attr: NodeAttr = serde_json::from_value(node_attr_json).map_err(|e| {
        ErrorResponse::new(
            400,
            "deserialization error",
            &format!("error deserializing NodeAttr: {e}"),
        )
    })?;
    // restore at uuid (they should not be changed)
    updated_node_attr.set_uuid(orig_uuid);
    Ok(updated_node_attr)
}

fn update_json(original: &mut Value, updates: &serde_json::Value) -> Result<(), ErrorResponse> {
    if let (Value::Object(orig), Value::Object(upd)) = (original, updates) {
        for (key, value) in upd {
            if value.is_object() && orig.get(key).is_some_and(Value::is_object) {
                update_json(orig.get_mut(key).unwrap(), value)?;
            } else {
                orig.insert(key.clone(), value.clone());
            }
        }
        Ok(())
    } else {
        Err(ErrorResponse::new(
            400,
            "conversion error",
            "no JSON object found",
        ))
    }
}
